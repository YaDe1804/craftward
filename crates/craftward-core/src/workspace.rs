//! The workspace registry and the project overview.

use std::fs;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

use serde_json::{Map, Number, Value};

use crate::error::{LoadError, ValidationError};
use crate::fs_atomic::write_atomic;
use crate::load::{load_from_path, load_from_str};
use crate::model::{IssueStatus, Project, ProjectStatus, TaskStatus};

const WORKSPACE_FIELDS: &[&str] = &["schema_version", "projects"];
const ENTRY_FIELDS: &[&str] = &["path"];
const PROJECT_FILE_NAME: &str = "craftward.json";

/// One `workspace.json` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workspace {
    /// Format generation. Readers of this version accept only the integer `1`.
    pub schema_version: u32,
    /// Project files, in the order they were added.
    pub projects: Vec<WorkspaceEntry>,
}

/// A path to one `craftward.json`, stored in the workspace file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceEntry {
    /// Path of the project file. A relative path is relative to the workspace
    /// file's directory and uses `/`. An absolute path is a normal path for
    /// this operating system, without the `\\?\` prefix.
    pub path: String,
}

impl Workspace {
    /// An empty schema-version-1 workspace.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            schema_version: 1,
            projects: Vec::new(),
        }
    }
}

/// Counts shown for one valid project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectSnapshot {
    /// Project id.
    pub id: String,
    /// Project name.
    pub name: String,
    /// Lifecycle status.
    pub status: ProjectStatus,
    /// `current_version`, when the file sets one.
    pub current_version: Option<String>,
    /// Tasks whose status is `open` or `in_progress`.
    pub open_tasks: usize,
    /// Issues whose status is `open` or `in_progress`.
    pub open_issues: usize,
    /// Earliest `tasks.due` or `watch_items.due` still outstanding.
    ///
    /// Tasks with status `done` or `dropped` are left out. Every watch item is
    /// included. The value is `YYYY-MM-DD`.
    pub next_due: Option<String>,
}

/// One row of the project overview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRow {
    /// Project file after resolving a relative workspace path.
    pub path: PathBuf,
    /// Metrics, or why this file could not be summarized.
    pub detail: Result<ProjectSnapshot, String>,
}

/// Failure while reading or updating a workspace.
#[derive(Debug)]
pub enum WorkspaceError {
    /// The workspace file could not be read.
    Read(io::Error),
    /// The workspace file could not be written.
    Write(io::Error),
    /// The workspace text is not valid JSON.
    Syntax {
        /// 1-based line number.
        line: usize,
        /// 1-based column number.
        column: usize,
        /// Parser message.
        message: String,
    },
    /// `schema_version` is newer than this build understands.
    UnsupportedSchema {
        /// The integer from the file.
        version: u64,
    },
    /// The workspace file breaks one or more schema-version-1 rules.
    Validation(Vec<ValidationError>),
    /// `init` found an existing project file.
    AlreadyExists(PathBuf),
    /// `add` was given a path that is not a file or directory.
    MissingProject(PathBuf),
    /// `add` refused a project file that does not validate.
    InvalidProject {
        /// The project file.
        path: PathBuf,
        /// Why the file was rejected.
        source: LoadError,
    },
    /// The workspace already lists this project file.
    DuplicatePath(PathBuf),
    /// The workspace already lists this project id.
    DuplicateId(String),
    /// `remove` found no readable project with this id.
    UnknownId(String),
}

impl std::fmt::Display for WorkspaceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read(error) => write!(formatter, "failed to read workspace: {error}"),
            Self::Write(error) => write!(formatter, "failed to write workspace: {error}"),
            Self::Syntax {
                line,
                column,
                message,
            } => write!(
                formatter,
                "syntax error at line {line}, column {column}: {message}"
            ),
            Self::UnsupportedSchema { .. } => {
                formatter.write_str("file was written by a newer Craftward version")
            }
            Self::Validation(errors) => {
                write!(
                    formatter,
                    "invalid workspace file ({} errors)",
                    errors.len()
                )
            }
            Self::AlreadyExists(path) => {
                write!(formatter, "file already exists: {}", path.display())
            }
            Self::MissingProject(path) => {
                write!(formatter, "project file not found: {}", path.display())
            }
            Self::InvalidProject { path, source } => {
                write!(
                    formatter,
                    "invalid project file {}: {source}",
                    path.display()
                )
            }
            Self::DuplicatePath(path) => {
                write!(
                    formatter,
                    "project file is already listed: {}",
                    path.display()
                )
            }
            Self::DuplicateId(id) => write!(formatter, "project id is already listed: {id}"),
            Self::UnknownId(id) => write!(formatter, "unknown project id: {id}"),
        }
    }
}

impl std::error::Error for WorkspaceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read(error) | Self::Write(error) => Some(error),
            Self::InvalidProject { source, .. } => Some(source),
            Self::Syntax { .. }
            | Self::UnsupportedSchema { .. }
            | Self::Validation(_)
            | Self::AlreadyExists(_)
            | Self::MissingProject(_)
            | Self::DuplicatePath(_)
            | Self::DuplicateId(_)
            | Self::UnknownId(_) => None,
        }
    }
}

/// Reads a workspace file.
///
/// A missing file is [`WorkspaceError::Read`] with [`ErrorKind::NotFound`].
/// Use [`load_or_empty`] when a missing registry means no projects.
///
/// # Errors
///
/// Returns a read, syntax, schema, or validation error.
pub fn load_workspace(path: &Path) -> Result<Workspace, WorkspaceError> {
    let text = fs::read_to_string(path).map_err(WorkspaceError::Read)?;
    parse_workspace(&text)
}

/// Reads a workspace file, treating a missing file as an empty registry.
///
/// # Errors
///
/// Returns the same errors as [`load_workspace`], except a missing file.
pub fn load_or_empty(path: &Path) -> Result<Workspace, WorkspaceError> {
    match load_workspace(path) {
        Ok(workspace) => Ok(workspace),
        Err(WorkspaceError::Read(error)) if error.kind() == ErrorKind::NotFound => {
            Ok(Workspace::empty())
        }
        Err(error) => Err(error),
    }
}

/// Writes a workspace file as pretty JSON with LF endings.
///
/// The write is atomic: a temporary file in the same directory is synced and
/// then renamed over the destination. Parent directories are created. A failed
/// write removes the temporary file.
///
/// # Errors
///
/// Returns [`WorkspaceError::Write`] when the directory cannot be created or
/// the rename fails.
pub fn save_workspace(path: &Path, workspace: &Workspace) -> Result<(), WorkspaceError> {
    let text = format_workspace(workspace);
    write_atomic(path, text.as_bytes()).map_err(WorkspaceError::Write)
}

/// Creates `directory/craftward.json` with only the required fields.
///
/// An existing file is left unchanged. The project is not added to a workspace.
///
/// # Errors
///
/// Returns [`WorkspaceError::AlreadyExists`] when the file is already there,
/// [`WorkspaceError::Validation`] when `id` or `name` breaks the project rules,
/// and [`WorkspaceError::Write`] when the file cannot be created.
pub fn init_project(
    directory: &Path,
    id: &str,
    name: &str,
    status: ProjectStatus,
) -> Result<PathBuf, WorkspaceError> {
    let path = directory.join(PROJECT_FILE_NAME);
    if path.exists() {
        return Err(WorkspaceError::AlreadyExists(path));
    }
    let project = Project {
        schema_version: 1,
        id: id.to_owned(),
        name: name.to_owned(),
        status,
        source_dir: None,
        description: None,
        current_version: None,
        tags: None,
        updated: None,
        links: None,
        stack: None,
        plan: None,
        issues: None,
        watch_items: None,
    };
    let text = format_project(&project);
    if let Err(error) = load_from_str(&text) {
        return Err(validation_from_load(error));
    }
    write_atomic(&path, text.as_bytes()).map_err(WorkspaceError::Write)?;
    Ok(path)
}

/// Adds a project file, or a directory containing `craftward.json`, to the workspace.
///
/// # Errors
///
/// Returns an error when the file is missing or invalid, or when its path or
/// project id is already listed. The workspace file is not created in those cases.
pub fn add_project(workspace_file: &Path, input: &Path) -> Result<String, WorkspaceError> {
    let project_file = project_file(input)?;
    let project =
        load_from_path(&project_file).map_err(|source| WorkspaceError::InvalidProject {
            path: project_file.clone(),
            source,
        })?;
    let mut workspace = load_or_empty(workspace_file)?;
    let new_full = canonical_file(&project_file);
    for entry in &workspace.projects {
        let existing = resolve_entry_path(workspace_file, &entry.path);
        if same_file(&existing, &new_full) {
            return Err(WorkspaceError::DuplicatePath(project_file));
        }
        if let Ok(listed) = load_from_path(&existing)
            && listed.id == project.id
        {
            return Err(WorkspaceError::DuplicateId(project.id));
        }
    }
    if let Some(parent) = workspace_file.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(WorkspaceError::Write)?;
    }
    let stored = path_to_store(workspace_file, &project_file);
    let id = project.id.clone();
    workspace.projects.push(WorkspaceEntry { path: stored });
    save_workspace(workspace_file, &workspace)?;
    Ok(id)
}

/// Returns the project file whose id is `id`.
///
/// Entries that cannot be read are skipped, because their id is unknown.
///
/// # Errors
///
/// Returns [`WorkspaceError::UnknownId`] when no readable entry has that id,
/// including when the workspace file does not exist.
pub fn project_file_by_id(workspace_file: &Path, id: &str) -> Result<PathBuf, WorkspaceError> {
    let workspace = match load_workspace(workspace_file) {
        Ok(workspace) => workspace,
        Err(WorkspaceError::Read(error)) if error.kind() == ErrorKind::NotFound => {
            return Err(WorkspaceError::UnknownId(id.to_owned()));
        }
        Err(error) => return Err(error),
    };
    for entry in &workspace.projects {
        let path = resolve_entry_path(workspace_file, &entry.path);
        if load_from_path(&path).is_ok_and(|project| project.id == id) {
            return Ok(path);
        }
    }
    Err(WorkspaceError::UnknownId(id.to_owned()))
}

/// Removes the workspace entry whose project id is `id`.
///
/// The project file is not deleted. Entries that cannot be read are skipped,
/// because their id is unknown.
///
/// # Errors
///
/// Returns [`WorkspaceError::UnknownId`] when no readable entry has that id,
/// including when the workspace file does not exist.
pub fn remove_project(workspace_file: &Path, id: &str) -> Result<(), WorkspaceError> {
    let mut workspace = match load_workspace(workspace_file) {
        Ok(workspace) => workspace,
        Err(WorkspaceError::Read(error)) if error.kind() == ErrorKind::NotFound => {
            return Err(WorkspaceError::UnknownId(id.to_owned()));
        }
        Err(error) => return Err(error),
    };
    let Some(index) = workspace.projects.iter().position(|entry| {
        let path = resolve_entry_path(workspace_file, &entry.path);
        load_from_path(&path).is_ok_and(|project| project.id == id)
    }) else {
        return Err(WorkspaceError::UnknownId(id.to_owned()));
    };
    workspace.projects.remove(index);
    save_workspace(workspace_file, &workspace)
}

/// Builds the overview for every entry.
///
/// A missing workspace file is an empty list. A project file that is missing or
/// invalid becomes a row whose detail is an error string. The other rows are
/// still returned.
///
/// # Errors
///
/// Returns an error when the workspace file itself is unreadable or invalid.
pub fn list_projects(workspace_file: &Path) -> Result<Vec<ProjectRow>, WorkspaceError> {
    let workspace = load_or_empty(workspace_file)?;
    Ok(workspace
        .projects
        .iter()
        .map(|entry| project_row(workspace_file, entry))
        .collect())
}

/// Counts open work and the next due date for a project that has already been loaded.
#[must_use]
pub fn snapshot(project: &Project) -> ProjectSnapshot {
    let mut open_tasks = 0;
    let mut dates = Vec::new();
    if let Some(plan) = &project.plan {
        for phase in &plan.phases {
            for task in &phase.tasks {
                if is_open_task(task.status) {
                    open_tasks += 1;
                    if let Some(due) = &task.due {
                        dates.push(due.clone());
                    }
                }
            }
        }
    }
    if let Some(items) = &project.watch_items {
        for item in items {
            dates.push(item.due.clone());
        }
    }
    let open_issues = project.issues.as_ref().map_or(0, |issues| {
        issues
            .iter()
            .filter(|issue| is_open_issue(issue.status))
            .count()
    });
    ProjectSnapshot {
        id: project.id.clone(),
        name: project.name.clone(),
        status: project.status,
        current_version: project.current_version.clone(),
        open_tasks,
        open_issues,
        next_due: dates.into_iter().min(),
    }
}

/// Resolves a stored path against the workspace file's directory.
///
/// The stored text may use `/`. Rebuilding the path from its components writes
/// the separator of this operating system between them. Joining the stored
/// text in one piece would keep those `/` on Windows, so a relative entry
/// `app/craftward.json` would be shown as `C:\workspace\app/craftward.json`.
#[must_use]
pub fn resolve_entry_path(workspace_file: &Path, stored: &str) -> PathBuf {
    let path = crate::paths::platform_path(Path::new(stored));
    if path.is_absolute() {
        path
    } else {
        let parent = workspace_file.parent().unwrap_or_else(|| Path::new(""));
        crate::paths::platform_path(&parent.join(path))
    }
}

fn project_row(workspace_file: &Path, entry: &WorkspaceEntry) -> ProjectRow {
    let path = resolve_entry_path(workspace_file, &entry.path);
    let detail = match load_from_path(&path) {
        Ok(project) => Ok(snapshot(&project)),
        Err(error) => Err(format!("{error}")),
    };
    ProjectRow { path, detail }
}

fn is_open_task(status: TaskStatus) -> bool {
    matches!(status, TaskStatus::Open | TaskStatus::InProgress)
}

fn is_open_issue(status: IssueStatus) -> bool {
    matches!(status, IssueStatus::Open | IssueStatus::InProgress)
}

/// Resolves a project file or a directory that contains `craftward.json`.
///
/// # Errors
///
/// Returns [`WorkspaceError::MissingProject`] when the file is not there, and
/// [`WorkspaceError::Read`] when the path cannot be inspected.
pub fn project_file(input: &Path) -> Result<PathBuf, WorkspaceError> {
    let metadata = fs::metadata(input).map_err(|error| {
        if error.kind() == ErrorKind::NotFound {
            WorkspaceError::MissingProject(input.to_path_buf())
        } else {
            WorkspaceError::Read(error)
        }
    })?;
    if metadata.is_dir() {
        let nested = input.join(PROJECT_FILE_NAME);
        if !nested.is_file() {
            return Err(WorkspaceError::MissingProject(nested));
        }
        Ok(nested)
    } else {
        Ok(input.to_path_buf())
    }
}

fn same_file(existing: &Path, new_full: &Path) -> bool {
    canonical_file(existing) == *new_full
}

fn canonical_file(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn path_to_store(workspace_file: &Path, project_file: &Path) -> String {
    // `std::path::absolute` makes a normal absolute path and does not add the
    // `\\?\` prefix. `canonicalize` is reserved for comparing the same file.
    let project = normal_absolute(project_file);
    let base = normal_absolute(&workspace_directory(workspace_file));
    if let Ok(relative) = project.strip_prefix(&base) {
        let relative = if relative.as_os_str().is_empty() {
            Path::new(PROJECT_FILE_NAME)
        } else {
            relative
        };
        return forward_slashes(relative);
    }
    project.to_string_lossy().into_owned()
}

fn workspace_directory(workspace_file: &Path) -> PathBuf {
    match workspace_file.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => PathBuf::from("."),
    }
}

fn normal_absolute(path: &Path) -> PathBuf {
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    without_verbatim(&absolute)
}

fn without_verbatim(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    let Some(stripped) = text.strip_prefix(r"\\?\") else {
        return path.to_path_buf();
    };
    if let Some(unc) = stripped.strip_prefix(r"UNC\") {
        PathBuf::from(format!(r"\\{unc}"))
    } else {
        PathBuf::from(stripped)
    }
}

fn forward_slashes(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    if text.is_empty() {
        PROJECT_FILE_NAME.to_owned()
    } else {
        text
    }
}

fn parse_workspace(text: &str) -> Result<Workspace, WorkspaceError> {
    let value = serde_json::from_str(text).map_err(|error| WorkspaceError::Syntax {
        line: error.line(),
        column: error.column(),
        message: error.to_string(),
    })?;
    if let Some(version) = newer_schema_version(&value) {
        return Err(WorkspaceError::UnsupportedSchema { version });
    }
    let mut errors = Vec::new();
    validate_workspace(&value, &mut errors);
    if !errors.is_empty() {
        return Err(WorkspaceError::Validation(errors));
    }
    workspace_from_value(&value)
}

fn newer_schema_version(value: &Value) -> Option<u64> {
    let version = value.as_object()?.get("schema_version")?.as_number()?;
    let version = u64_number(version)?;
    (version > 1).then_some(version)
}

fn u64_number(number: &Number) -> Option<u64> {
    if number.is_u64() {
        number.as_u64()
    } else {
        None
    }
}

fn validate_workspace(value: &Value, errors: &mut Vec<ValidationError>) {
    let Some(object) = expect_object(value, "$", errors) else {
        return;
    };
    reject_unknown(object, "$", WORKSPACE_FIELDS, errors);
    match object.get("schema_version") {
        None => push(errors, "schema_version", "is required"),
        Some(version) => {
            let is_one = version
                .as_number()
                .and_then(u64_number)
                .is_some_and(|number| number == 1);
            if !is_one {
                push(errors, "schema_version", "must be 1");
            }
        }
    }
    let Some(projects) = object.get("projects") else {
        push(errors, "projects", "is required");
        return;
    };
    let Some(items) = expect_array(projects, "projects", errors) else {
        return;
    };
    let mut seen = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let path = format!("projects[{index}]");
        let Some(entry) = expect_object(item, &path, errors) else {
            continue;
        };
        reject_unknown(entry, &path, ENTRY_FIELDS, errors);
        let Some(stored) = required_path(entry, &path, errors) else {
            continue;
        };
        let key = stored.replace('\\', "/");
        if seen.iter().any(|existing: &String| existing == &key) {
            push(errors, format!("{path}.path"), "duplicate path");
        } else {
            seen.push(key);
        }
    }
}

fn required_path<'a>(
    object: &'a Map<String, Value>,
    parent: &str,
    errors: &mut Vec<ValidationError>,
) -> Option<&'a str> {
    let path = format!("{parent}.path");
    let Some(value) = object.get("path") else {
        push(errors, path, "is required");
        return None;
    };
    let text = expect_string(value, &path, errors)?;
    if text.trim().is_empty() {
        push(errors, path, "must not be empty or whitespace");
        return None;
    }
    Some(text)
}

fn workspace_from_value(value: &Value) -> Result<Workspace, WorkspaceError> {
    let object = value.as_object().ok_or_else(|| {
        WorkspaceError::Validation(vec![ValidationError {
            path: "$".to_owned(),
            message: "must be an object".to_owned(),
        }])
    })?;
    let mut projects = Vec::new();
    for item in object
        .get("projects")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let path = item
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        projects.push(WorkspaceEntry { path });
    }
    Ok(Workspace {
        schema_version: 1,
        projects,
    })
}

fn format_workspace(workspace: &Workspace) -> String {
    let entries: Vec<String> = workspace
        .projects
        .iter()
        .map(|entry| {
            let path = serde_json::to_string(&entry.path).unwrap_or_else(|_| "\"\"".to_owned());
            format!("    {{\n      \"path\": {path}\n    }}")
        })
        .collect();
    let body = if entries.is_empty() {
        String::new()
    } else {
        format!("\n{}\n  ", entries.join(",\n"))
    };
    format!(
        "{{\n  \"schema_version\": {},\n  \"projects\": [{body}]\n}}\n",
        workspace.schema_version
    )
}

pub(crate) fn format_project(project: &Project) -> String {
    let mut text = serde_json::to_string_pretty(project).unwrap_or_else(|_| "{}".to_owned());
    text.push('\n');
    text
}

fn validation_from_load(error: LoadError) -> WorkspaceError {
    match error {
        LoadError::Validation(errors) => WorkspaceError::Validation(errors),
        other => WorkspaceError::Validation(vec![ValidationError {
            path: "$".to_owned(),
            message: other.to_string(),
        }]),
    }
}

fn reject_unknown(
    object: &Map<String, Value>,
    parent: &str,
    allowed: &[&str],
    errors: &mut Vec<ValidationError>,
) {
    for key in object.keys() {
        if !allowed.contains(&key.as_str()) {
            push(errors, join(parent, key), "unknown field");
        }
    }
}

fn expect_object<'a>(
    value: &'a Value,
    path: &str,
    errors: &mut Vec<ValidationError>,
) -> Option<&'a Map<String, Value>> {
    if let Value::Object(object) = value {
        Some(object)
    } else {
        push(errors, path, "must be an object");
        None
    }
}

fn expect_array<'a>(
    value: &'a Value,
    path: &str,
    errors: &mut Vec<ValidationError>,
) -> Option<&'a Vec<Value>> {
    if let Value::Array(items) = value {
        Some(items)
    } else {
        push(errors, path, "must be an array");
        None
    }
}

fn expect_string<'a>(
    value: &'a Value,
    path: &str,
    errors: &mut Vec<ValidationError>,
) -> Option<&'a str> {
    if let Value::String(text) = value {
        Some(text)
    } else {
        push(errors, path, "must be a string");
        None
    }
}

fn join(parent: &str, field: &str) -> String {
    if parent == "$" {
        field.to_owned()
    } else {
        format!("{parent}.{field}")
    }
}

fn push(errors: &mut Vec<ValidationError>, path: impl Into<String>, message: impl Into<String>) {
    errors.push(ValidationError {
        path: path.into(),
        message: message.into(),
    });
}
