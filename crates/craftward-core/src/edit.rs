//! Changes to a project file.
//!
//! Each function copies the project, applies one change, and validates the
//! copy. [`edit_file`] loads a file, runs one of those functions, and writes
//! the result only when it is valid.

use std::fmt::Write as _;
use std::path::Path;

use crate::date::{DateError, validate_date};
use crate::error::{LoadError, ValidationError};
use crate::load::{load_from_path, load_from_str};
use crate::model::{
    Issue, IssueSeverity, IssueStatus, Phase, Plan, Project, ProjectStatus, Task, TaskStatus,
    WatchItem, WatchKind,
};
use crate::workspace::{format_project, write_atomic};

const PROJECT_STATUSES: &[&str] = &[
    "idea",
    "in_development",
    "released",
    "maintenance",
    "paused",
    "discontinued",
];
const TASK_STATUSES: &[&str] = &["open", "in_progress", "done", "dropped"];
const ISSUE_STATUSES: &[&str] = &["open", "in_progress", "resolved", "wont_fix"];
const SEVERITIES: &[&str] = &["low", "medium", "high", "critical"];
const WATCH_KINDS: &[&str] = &["deadline", "expiry", "review", "other"];
const DATE_FORMS: &[&str] = &["YYYY-MM-DD"];
const URL_FORMS: &[&str] = &["http://", "https://"];

/// A value that can stay, be replaced, or be removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectUpdate<'a> {
    /// Leave the field as it is.
    Keep,
    /// Store this text.
    Set(&'a str),
    /// Remove the field.
    Clear,
}

/// Fields accepted by [`set_project`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectChanges<'a> {
    /// New project name, when one was given.
    pub name: Option<&'a str>,
    /// New lifecycle status, when one was given.
    pub status: Option<&'a str>,
    /// `current_version`. [`ProjectUpdate::Clear`] removes it.
    pub version: ProjectUpdate<'a>,
    /// Description. [`ProjectUpdate::Clear`] removes it.
    pub description: ProjectUpdate<'a>,
    /// New source directory, when one was given.
    pub source_dir: Option<&'a str>,
}

/// A phase to append.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewPhase<'a> {
    /// Phase title.
    pub title: &'a str,
    /// Explicit id. When absent, the id is formed from the title.
    pub id: Option<&'a str>,
    /// Optional goal.
    pub goal: Option<&'a str>,
}

/// A task to append to one phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewTask<'a> {
    /// Phase id.
    pub phase: &'a str,
    /// Task title.
    pub title: &'a str,
    /// Explicit id. When absent, the id is formed from the title.
    pub id: Option<&'a str>,
    /// Status word. When absent, the task is `open`.
    pub status: Option<&'a str>,
    /// Due day, `YYYY-MM-DD`.
    pub due: Option<&'a str>,
    /// Optional note.
    pub note: Option<&'a str>,
}

/// An issue to append.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewIssue<'a> {
    /// Issue title.
    pub title: &'a str,
    /// Severity word.
    pub severity: &'a str,
    /// Explicit id. When absent, the id is formed from the title.
    pub id: Option<&'a str>,
    /// Tracker address.
    pub url: Option<&'a str>,
    /// Optional note.
    pub note: Option<&'a str>,
}

/// A watch item to append.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewWatch<'a> {
    /// Kind word.
    pub kind: &'a str,
    /// Reminder title.
    pub title: &'a str,
    /// Due day, `YYYY-MM-DD`.
    pub due: &'a str,
    /// Explicit id. When absent, the id is formed from the title.
    pub id: Option<&'a str>,
    /// Optional note.
    pub note: Option<&'a str>,
}

/// The project produced by a successful change, and the id created by an add.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditOutcome {
    /// The project that was written.
    pub project: Project,
    /// The id assigned by an add command.
    pub created_id: Option<String>,
    serialized: String,
}

/// Why a project change was rejected or could not be stored.
#[derive(Debug)]
pub enum EditError {
    /// No item with this id exists in that list.
    NotFound {
        /// `phase`, `task`, `issue`, or `watch`.
        kind: &'static str,
        /// The id that was requested.
        id: String,
        /// Ids that do exist in that list, in file order.
        available: Vec<String>,
    },
    /// A status, date, severity, kind, or URL is not one of the allowed values.
    Invalid {
        /// Field name.
        field: &'static str,
        /// The rejected text.
        value: String,
        /// Allowed words or forms.
        allowed: &'static [&'static str],
    },
    /// A title produced no id. The caller should pass `--id`.
    NeedsId,
    /// The phase still has tasks and `--force` was not set.
    PhaseNotEmpty {
        /// Phase id.
        id: String,
    },
    /// The changed project failed validation. The file was not written.
    InvalidFile(Vec<ValidationError>),
    /// The file could not be read or is already invalid.
    Load(LoadError),
    /// The validated text could not be written.
    Write(std::io::Error),
}

impl std::fmt::Display for EditError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound {
                kind,
                id,
                available,
            } => {
                let label = match *kind {
                    "phase" => "phases",
                    "task" => "tasks",
                    "issue" => "issues",
                    "watch" => "watch items",
                    _ => kind,
                };
                let list = if available.is_empty() {
                    "(none)".to_owned()
                } else {
                    available.join(", ")
                };
                write!(formatter, "unknown {kind}: {id}\n{label}: {list}")
            }
            Self::Invalid {
                field,
                value,
                allowed,
            } => write!(
                formatter,
                "invalid {field}: {value}\nallowed: {}",
                allowed.join(", ")
            ),
            Self::NeedsId => formatter.write_str("cannot form an id from the title; pass --id"),
            Self::PhaseNotEmpty { id } => write!(
                formatter,
                "phase {id} still has tasks; pass --force to remove it"
            ),
            Self::InvalidFile(errors) | Self::Load(LoadError::Validation(errors)) => {
                write!(formatter, "invalid project file")?;
                for error in errors {
                    write!(formatter, "\n  {error}")?;
                }
                Ok(())
            }
            Self::Load(error) => write!(formatter, "{error}"),
            Self::Write(error) => write!(formatter, "failed to write project file: {error}"),
        }
    }
}

impl std::error::Error for EditError {}

impl EditError {
    /// Whether this rejection is a bad value on the command line.
    ///
    /// Those exit 2. Every other edit error exits 1.
    #[must_use]
    pub const fn is_usage(&self) -> bool {
        matches!(self, Self::Invalid { .. })
    }
}

/// Loads `path`, applies `change`, and replaces the file only when the result
/// is valid.
///
/// `updated_on` is the UTC day stored in `updated`, as `YYYY-MM-DD`.
///
/// # Errors
///
/// Returns the change's error without writing when the result is invalid.
/// Returns [`EditError::Load`] or [`EditError::Write`] when the file cannot
/// be read or replaced.
pub fn edit_file(
    path: &Path,
    updated_on: &str,
    change: impl FnOnce(&Project, &str) -> Result<EditOutcome, EditError>,
) -> Result<EditOutcome, EditError> {
    let project = load_from_path(path).map_err(EditError::Load)?;
    let outcome = change(&project, updated_on)?;
    write_atomic(path, outcome.serialized.as_bytes()).map_err(EditError::Write)?;
    Ok(outcome)
}

/// Replaces the given header fields and sets `updated`.
///
/// # Errors
///
/// Returns [`EditError::Invalid`] for an unknown status, or
/// [`EditError::InvalidFile`] when the result breaks a project rule.
pub fn set_project(
    project: &Project,
    changes: ProjectChanges<'_>,
    updated_on: &str,
) -> Result<EditOutcome, EditError> {
    let mut next = project.clone();
    if let Some(name) = changes.name {
        name.clone_into(&mut next.name);
    }
    if let Some(status) = changes.status {
        next.status = parse_project_status(status)?;
    }
    match changes.version {
        ProjectUpdate::Keep => {}
        ProjectUpdate::Set(text) => next.current_version = Some(text.to_owned()),
        ProjectUpdate::Clear => next.current_version = None,
    }
    match changes.description {
        ProjectUpdate::Keep => {}
        ProjectUpdate::Set(text) => next.description = Some(text.to_owned()),
        ProjectUpdate::Clear => next.description = None,
    }
    if let Some(source_dir) = changes.source_dir {
        next.source_dir = Some(source_dir.to_owned());
    }
    commit(next, updated_on, None)
}

/// Appends a phase. The plan is created when the project has none.
///
/// # Errors
///
/// Returns [`EditError::NeedsId`] when the title yields no id, or
/// [`EditError::InvalidFile`] when the id is already used or otherwise illegal.
pub fn add_phase(
    project: &Project,
    phase: NewPhase<'_>,
    updated_on: &str,
) -> Result<EditOutcome, EditError> {
    let mut next = project.clone();
    let id = choose_id(phase.title, phase.id, |candidate| {
        phase_ids(&next).contains(&candidate)
    })?;
    let phases = &mut next
        .plan
        .get_or_insert_with(|| Plan { phases: Vec::new() })
        .phases;
    phases.push(Phase {
        id: id.clone(),
        title: phase.title.to_owned(),
        goal: phase.goal.map(str::to_owned),
        tasks: Vec::new(),
    });
    commit(next, updated_on, Some(id))
}

/// Removes a phase. A phase that still has tasks is removed only when `force`
/// is set.
///
/// # Errors
///
/// Returns [`EditError::NotFound`] when the phase id is unknown, or
/// [`EditError::PhaseNotEmpty`] when tasks remain and `force` is false.
pub fn remove_phase(
    project: &Project,
    phase_id: &str,
    force: bool,
    updated_on: &str,
) -> Result<EditOutcome, EditError> {
    let mut next = project.clone();
    let index = phase_index(&next, phase_id)?;
    let Some(phases) = next.plan.as_mut().map(|plan| &mut plan.phases) else {
        return Err(missing("phase", phase_id, Vec::new()));
    };
    if !phases[index].tasks.is_empty() && !force {
        return Err(EditError::PhaseNotEmpty {
            id: phase_id.to_owned(),
        });
    }
    phases.remove(index);
    commit(next, updated_on, None)
}

/// Appends a task to a phase. The status defaults to `open`.
///
/// # Errors
///
/// Returns [`EditError::NotFound`] for an unknown phase, [`EditError::Invalid`]
/// for a bad status or date, and [`EditError::NeedsId`] when the title yields
/// no id.
pub fn add_task(
    project: &Project,
    task: NewTask<'_>,
    updated_on: &str,
) -> Result<EditOutcome, EditError> {
    let status = match task.status {
        Some(text) => parse_task_status(text)?,
        None => TaskStatus::Open,
    };
    let due = optional_date(task.due)?;
    let mut next = project.clone();
    let index = phase_index(&next, task.phase)?;
    let id = choose_id(task.title, task.id, |candidate| {
        next.plan.as_ref().is_some_and(|plan| {
            plan.phases[index]
                .tasks
                .iter()
                .any(|existing| existing.id == candidate)
        })
    })?;
    let Some(plan) = next.plan.as_mut() else {
        return Err(missing("phase", task.phase, Vec::new()));
    };
    plan.phases[index].tasks.push(Task {
        id: id.clone(),
        title: task.title.to_owned(),
        status,
        due,
        note: task.note.map(str::to_owned),
    });
    commit(next, updated_on, Some(id))
}

/// Sets one task's status.
///
/// # Errors
///
/// Returns [`EditError::NotFound`] when the phase or task is unknown, or
/// [`EditError::Invalid`] when `status` is not a task status.
pub fn set_task_status(
    project: &Project,
    phase_id: &str,
    task_id: &str,
    status: &str,
    updated_on: &str,
) -> Result<EditOutcome, EditError> {
    let status = parse_task_status(status)?;
    let mut next = project.clone();
    let phase = phase_index(&next, phase_id)?;
    let Some(plan) = next.plan.as_mut() else {
        return Err(missing("phase", phase_id, Vec::new()));
    };
    let tasks = &mut plan.phases[phase].tasks;
    let task = task_index(tasks, task_id)?;
    tasks[task].status = status;
    commit(next, updated_on, None)
}

/// Removes one task from a phase.
///
/// # Errors
///
/// Returns [`EditError::NotFound`] when the phase or task is unknown.
pub fn remove_task(
    project: &Project,
    phase_id: &str,
    task_id: &str,
    updated_on: &str,
) -> Result<EditOutcome, EditError> {
    let mut next = project.clone();
    let phase = phase_index(&next, phase_id)?;
    let Some(plan) = next.plan.as_mut() else {
        return Err(missing("phase", phase_id, Vec::new()));
    };
    let tasks = &mut plan.phases[phase].tasks;
    let task = task_index(tasks, task_id)?;
    tasks.remove(task);
    commit(next, updated_on, None)
}

/// Appends an issue. Its status is `open`.
///
/// # Errors
///
/// Returns [`EditError::Invalid`] for a bad severity or URL,
/// [`EditError::NeedsId`] when the title yields no id, or
/// [`EditError::InvalidFile`] when the id is already used.
pub fn add_issue(
    project: &Project,
    issue: NewIssue<'_>,
    updated_on: &str,
) -> Result<EditOutcome, EditError> {
    let severity = parse_severity(issue.severity)?;
    let url = optional_url(issue.url)?;
    let mut next = project.clone();
    let id = choose_id(issue.title, issue.id, |candidate| {
        issue_ids(&next).contains(&candidate)
    })?;
    next.issues.get_or_insert_with(Vec::new).push(Issue {
        id: id.clone(),
        title: issue.title.to_owned(),
        severity,
        status: IssueStatus::Open,
        url,
        note: issue.note.map(str::to_owned),
    });
    commit(next, updated_on, Some(id))
}

/// Sets one issue's status.
///
/// # Errors
///
/// Returns [`EditError::NotFound`] when the issue is unknown, or
/// [`EditError::Invalid`] when `status` is not an issue status.
pub fn set_issue_status(
    project: &Project,
    issue_id: &str,
    status: &str,
    updated_on: &str,
) -> Result<EditOutcome, EditError> {
    let status = parse_issue_status(status)?;
    let mut next = project.clone();
    let issues = next.issues.as_mut();
    let Some(issues) = issues else {
        return Err(missing("issue", issue_id, Vec::new()));
    };
    let index = item_index(
        issues.iter().map(|issue| issue.id.as_str()),
        issue_id,
        "issue",
    )?;
    issues[index].status = status;
    commit(next, updated_on, None)
}

/// Removes one issue.
///
/// # Errors
///
/// Returns [`EditError::NotFound`] when the issue is unknown.
pub fn remove_issue(
    project: &Project,
    issue_id: &str,
    updated_on: &str,
) -> Result<EditOutcome, EditError> {
    let mut next = project.clone();
    let issues = next.issues.as_mut();
    let Some(issues) = issues else {
        return Err(missing("issue", issue_id, Vec::new()));
    };
    let index = item_index(
        issues.iter().map(|issue| issue.id.as_str()),
        issue_id,
        "issue",
    )?;
    issues.remove(index);
    commit(next, updated_on, None)
}

/// Appends a watch item.
///
/// # Errors
///
/// Returns [`EditError::Invalid`] for a bad kind or date, or
/// [`EditError::NeedsId`] when the title yields no id.
pub fn add_watch(
    project: &Project,
    watch: NewWatch<'_>,
    updated_on: &str,
) -> Result<EditOutcome, EditError> {
    let kind = parse_watch_kind(watch.kind)?;
    let due = require_date(watch.due)?;
    let mut next = project.clone();
    let id = choose_id(watch.title, watch.id, |candidate| {
        watch_ids(&next).contains(&candidate)
    })?;
    next.watch_items
        .get_or_insert_with(Vec::new)
        .push(WatchItem {
            id: id.clone(),
            kind,
            title: watch.title.to_owned(),
            due,
            note: watch.note.map(str::to_owned),
        });
    commit(next, updated_on, Some(id))
}

/// Removes one watch item.
///
/// # Errors
///
/// Returns [`EditError::NotFound`] when the watch id is unknown.
pub fn remove_watch(
    project: &Project,
    watch_id: &str,
    updated_on: &str,
) -> Result<EditOutcome, EditError> {
    let mut next = project.clone();
    let items = next.watch_items.as_mut();
    let Some(items) = items else {
        return Err(missing("watch", watch_id, Vec::new()));
    };
    let index = item_index(items.iter().map(|item| item.id.as_str()), watch_id, "watch")?;
    items.remove(index);
    commit(next, updated_on, None)
}

/// Renders a project so its ids can be copied into later commands.
#[must_use]
pub fn render_project(project: &Project) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "id: {}", project.id);
    let _ = writeln!(out, "name: {}", project.name);
    let _ = writeln!(out, "status: {}", project.status.as_str());
    if let Some(version) = &project.current_version {
        let _ = writeln!(out, "version: {version}");
    }
    if let Some(description) = &project.description {
        let _ = writeln!(out, "description: {description}");
    }
    if let Some(source_dir) = &project.source_dir {
        let _ = writeln!(out, "source_dir: {source_dir}");
    }
    if let Some(updated) = &project.updated {
        let _ = writeln!(out, "updated: {updated}");
    }
    if let Some(tags) = &project.tags {
        let _ = writeln!(out, "tags: {}", tags.join(", "));
    }
    out.push('\n');
    if let Some(plan) = &project.plan {
        for phase in &plan.phases {
            let _ = writeln!(out, "phase {}", phase.id);
            let _ = writeln!(out, "  title: {}", phase.title);
            if let Some(goal) = &phase.goal {
                let _ = writeln!(out, "  goal: {goal}");
            }
            for task in &phase.tasks {
                let _ = writeln!(
                    out,
                    "  task {} [{}] {}",
                    task.id,
                    task_status_word(task.status),
                    task.title
                );
                if let Some(due) = &task.due {
                    let _ = writeln!(out, "    due: {due}");
                }
                if let Some(note) = &task.note {
                    let _ = writeln!(out, "    note: {note}");
                }
            }
            out.push('\n');
        }
    }
    let _ = writeln!(out, "issues");
    let open_issues = project
        .issues
        .iter()
        .flatten()
        .filter(|issue| matches!(issue.status, IssueStatus::Open | IssueStatus::InProgress));
    let mut any_issue = false;
    for issue in open_issues {
        any_issue = true;
        let _ = writeln!(
            out,
            "  issue {} [{}] {} {}",
            issue.id,
            issue_status_word(issue.status),
            severity_word(issue.severity),
            issue.title
        );
        if let Some(url) = &issue.url {
            let _ = writeln!(out, "    url: {url}");
        }
        if let Some(note) = &issue.note {
            let _ = writeln!(out, "    note: {note}");
        }
    }
    if !any_issue {
        let _ = writeln!(out, "  none");
    }
    out.push('\n');
    let _ = writeln!(out, "watch_items");
    let mut any_watch = false;
    for item in project.watch_items.iter().flatten() {
        any_watch = true;
        let _ = writeln!(
            out,
            "  watch {} [{}] {}",
            item.id,
            watch_kind_word(item.kind),
            item.title
        );
        let _ = writeln!(out, "    due: {}", item.due);
        if let Some(note) = &item.note {
            let _ = writeln!(out, "    note: {note}");
        }
    }
    if !any_watch {
        let _ = writeln!(out, "  none");
    }
    out
}

/// Forms a short id from a title.
///
/// Letters with a known Latin base are folded. `ä ö ü ß` become `ae oe ue ss`.
/// Other letters are dropped. Every remaining character outside `a-z` and
/// `0-9` becomes a hyphen. The result is at most 64 characters and does not
/// start or end with a hyphen. [`None`] means nothing usable was left.
#[must_use]
pub fn id_from_title(title: &str) -> Option<String> {
    let mut folded = String::new();
    for character in title.chars() {
        match fold_char(character) {
            Fold::Text(text) => folded.push_str(text),
            Fold::Char(ascii) => folded.push(ascii),
            Fold::Skip => {}
            Fold::Hyphen => folded.push('-'),
        }
    }
    let mut collapsed = String::new();
    let mut pending_hyphen = false;
    for character in folded.chars() {
        if character == '-' {
            pending_hyphen = !collapsed.is_empty();
        } else {
            if pending_hyphen {
                collapsed.push('-');
                pending_hyphen = false;
            }
            collapsed.push(character);
        }
    }
    let limited = limit_slug(&collapsed, 64);
    if limited.is_empty() {
        None
    } else {
        Some(limited)
    }
}

fn commit(
    mut project: Project,
    updated_on: &str,
    created_id: Option<String>,
) -> Result<EditOutcome, EditError> {
    project.updated = Some(updated_on.to_owned());
    let serialized = format_project(&project);
    match load_from_str(&serialized) {
        Ok(_) => Ok(EditOutcome {
            project,
            created_id,
            serialized,
        }),
        Err(LoadError::Validation(errors)) => Err(EditError::InvalidFile(errors)),
        Err(error) => Err(EditError::InvalidFile(vec![ValidationError {
            path: "$".to_owned(),
            message: error.to_string(),
        }])),
    }
}

fn choose_id(
    title: &str,
    explicit: Option<&str>,
    taken: impl Fn(&str) -> bool,
) -> Result<String, EditError> {
    if let Some(id) = explicit {
        return Ok(id.to_owned());
    }
    let base = id_from_title(title).ok_or(EditError::NeedsId)?;
    unique_id(&base, taken)
}

fn unique_id(base: &str, taken: impl Fn(&str) -> bool) -> Result<String, EditError> {
    if !taken(base) {
        return Ok(base.to_owned());
    }
    for number in 2..10_000 {
        let suffix = format!("-{number}");
        let Some(room) = 64usize.checked_sub(suffix.len()) else {
            break;
        };
        let stem = limit_slug(base, room);
        if stem.is_empty() {
            continue;
        }
        let candidate = format!("{stem}{suffix}");
        if !taken(&candidate) {
            return Ok(candidate);
        }
    }
    Err(EditError::NeedsId)
}

fn limit_slug(text: &str, max: usize) -> String {
    let end = text.len().min(max);
    let mut cut = text[..end].to_owned();
    while cut.ends_with('-') {
        cut.pop();
    }
    cut
}

fn phase_ids(project: &Project) -> Vec<&str> {
    project
        .plan
        .as_ref()
        .map_or_default(|plan| plan.phases.iter().map(|phase| phase.id.as_str()).collect())
}

fn issue_ids(project: &Project) -> Vec<&str> {
    project
        .issues
        .as_ref()
        .map_or_default(|issues| issues.iter().map(|issue| issue.id.as_str()).collect())
}

fn watch_ids(project: &Project) -> Vec<&str> {
    project
        .watch_items
        .as_ref()
        .map_or_default(|items| items.iter().map(|item| item.id.as_str()).collect())
}

fn phase_index(project: &Project, id: &str) -> Result<usize, EditError> {
    let ids = phase_ids(project);
    ids.iter()
        .position(|existing| *existing == id)
        .ok_or_else(|| missing("phase", id, ids.into_iter().map(str::to_owned).collect()))
}

fn task_index(tasks: &[Task], id: &str) -> Result<usize, EditError> {
    item_index(tasks.iter().map(|task| task.id.as_str()), id, "task")
}

fn item_index<'a>(
    ids: impl Iterator<Item = &'a str>,
    id: &str,
    kind: &'static str,
) -> Result<usize, EditError> {
    let ids: Vec<&str> = ids.collect();
    ids.iter()
        .position(|existing| *existing == id)
        .ok_or_else(|| missing(kind, id, ids.into_iter().map(str::to_owned).collect()))
}

fn missing(kind: &'static str, id: &str, available: Vec<String>) -> EditError {
    EditError::NotFound {
        kind,
        id: id.to_owned(),
        available,
    }
}

fn optional_date(value: Option<&str>) -> Result<Option<String>, EditError> {
    value.map(require_date).transpose()
}

fn require_date(value: &str) -> Result<String, EditError> {
    match validate_date(value) {
        Ok(()) => Ok(value.to_owned()),
        Err(DateError::Format | DateError::Calendar) => Err(EditError::Invalid {
            field: "date",
            value: value.to_owned(),
            allowed: DATE_FORMS,
        }),
    }
}

fn optional_url(value: Option<&str>) -> Result<Option<String>, EditError> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.starts_with("http://") || value.starts_with("https://") {
        Ok(Some(value.to_owned()))
    } else {
        Err(EditError::Invalid {
            field: "url",
            value: value.to_owned(),
            allowed: URL_FORMS,
        })
    }
}

fn parse_project_status(text: &str) -> Result<ProjectStatus, EditError> {
    ProjectStatus::parse(text).ok_or_else(|| EditError::Invalid {
        field: "status",
        value: text.to_owned(),
        allowed: PROJECT_STATUSES,
    })
}

fn parse_task_status(text: &str) -> Result<TaskStatus, EditError> {
    let status = match text {
        "open" => TaskStatus::Open,
        "in_progress" => TaskStatus::InProgress,
        "done" => TaskStatus::Done,
        "dropped" => TaskStatus::Dropped,
        _ => {
            return Err(EditError::Invalid {
                field: "status",
                value: text.to_owned(),
                allowed: TASK_STATUSES,
            });
        }
    };
    Ok(status)
}

fn parse_issue_status(text: &str) -> Result<IssueStatus, EditError> {
    let status = match text {
        "open" => IssueStatus::Open,
        "in_progress" => IssueStatus::InProgress,
        "resolved" => IssueStatus::Resolved,
        "wont_fix" => IssueStatus::WontFix,
        _ => {
            return Err(EditError::Invalid {
                field: "status",
                value: text.to_owned(),
                allowed: ISSUE_STATUSES,
            });
        }
    };
    Ok(status)
}

fn parse_severity(text: &str) -> Result<IssueSeverity, EditError> {
    let severity = match text {
        "low" => IssueSeverity::Low,
        "medium" => IssueSeverity::Medium,
        "high" => IssueSeverity::High,
        "critical" => IssueSeverity::Critical,
        _ => {
            return Err(EditError::Invalid {
                field: "severity",
                value: text.to_owned(),
                allowed: SEVERITIES,
            });
        }
    };
    Ok(severity)
}

fn parse_watch_kind(text: &str) -> Result<WatchKind, EditError> {
    let kind = match text {
        "deadline" => WatchKind::Deadline,
        "expiry" => WatchKind::Expiry,
        "review" => WatchKind::Review,
        "other" => WatchKind::Other,
        _ => {
            return Err(EditError::Invalid {
                field: "kind",
                value: text.to_owned(),
                allowed: WATCH_KINDS,
            });
        }
    };
    Ok(kind)
}

fn task_status_word(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Open => "open",
        TaskStatus::InProgress => "in_progress",
        TaskStatus::Done => "done",
        TaskStatus::Dropped => "dropped",
    }
}

fn issue_status_word(status: IssueStatus) -> &'static str {
    match status {
        IssueStatus::Open => "open",
        IssueStatus::InProgress => "in_progress",
        IssueStatus::Resolved => "resolved",
        IssueStatus::WontFix => "wont_fix",
    }
}

fn severity_word(severity: IssueSeverity) -> &'static str {
    match severity {
        IssueSeverity::Low => "low",
        IssueSeverity::Medium => "medium",
        IssueSeverity::High => "high",
        IssueSeverity::Critical => "critical",
    }
}

fn watch_kind_word(kind: WatchKind) -> &'static str {
    match kind {
        WatchKind::Deadline => "deadline",
        WatchKind::Expiry => "expiry",
        WatchKind::Review => "review",
        WatchKind::Other => "other",
    }
}

enum Fold {
    Text(&'static str),
    Char(char),
    Skip,
    Hyphen,
}

fn fold_char(character: char) -> Fold {
    match character {
        'ä' | 'Ä' | 'æ' | 'Æ' => Fold::Text("ae"),
        'ö' | 'Ö' | 'œ' | 'Œ' => Fold::Text("oe"),
        'ü' | 'Ü' => Fold::Text("ue"),
        'ß' | 'ẞ' => Fold::Text("ss"),
        'þ' | 'Þ' => Fold::Text("th"),
        'ð' | 'Ð' => Fold::Text("d"),
        'ø' | 'Ø' => Fold::Text("o"),
        'ł' | 'Ł' => Fold::Text("l"),
        other => {
            if let Some(base) = latin_base(other) {
                Fold::Char(base)
            } else if other.is_ascii_digit() {
                Fold::Char(other)
            } else if other.is_ascii_alphabetic() {
                Fold::Char(other.to_ascii_lowercase())
            } else if other.is_alphabetic() {
                Fold::Skip
            } else {
                Fold::Hyphen
            }
        }
    }
}

fn latin_base(character: char) -> Option<char> {
    Some(match character {
        'À' | 'Á' | 'Â' | 'Ã' | 'Å' | 'Ā' | 'Ă' | 'Ą' | 'à' | 'á' | 'â' | 'ã' | 'å' | 'ā' | 'ă'
        | 'ą' => 'a',
        'Ç' | 'Ć' | 'Ĉ' | 'Ċ' | 'Č' | 'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => 'c',
        'Ď' | 'Đ' | 'ď' | 'đ' => 'd',
        'È' | 'É' | 'Ê' | 'Ë' | 'Ē' | 'Ĕ' | 'Ė' | 'Ę' | 'Ě' | 'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ'
        | 'ė' | 'ę' | 'ě' => 'e',
        'Ĝ' | 'Ğ' | 'Ġ' | 'Ģ' | 'ĝ' | 'ğ' | 'ġ' | 'ģ' => 'g',
        'Ĥ' | 'Ħ' | 'ĥ' | 'ħ' => 'h',
        'Ì' | 'Í' | 'Î' | 'Ï' | 'Ĩ' | 'Ī' | 'Ĭ' | 'Į' | 'İ' | 'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī'
        | 'ĭ' | 'į' | 'ı' => 'i',
        'Ĵ' | 'ĵ' => 'j',
        'Ķ' | 'ķ' => 'k',
        'Ĺ' | 'Ļ' | 'Ľ' | 'ĺ' | 'ļ' | 'ľ' => 'l',
        'Ñ' | 'Ń' | 'Ņ' | 'Ň' | 'ñ' | 'ń' | 'ņ' | 'ň' => 'n',
        'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ō' | 'Ŏ' | 'Ő' | 'ò' | 'ó' | 'ô' | 'õ' | 'ō' | 'ŏ' | 'ő' => {
            'o'
        }
        'Ŕ' | 'Ŗ' | 'Ř' | 'ŕ' | 'ŗ' | 'ř' => 'r',
        'Ś' | 'Ŝ' | 'Ş' | 'Š' | 'ś' | 'ŝ' | 'ş' | 'š' => 's',
        'Ţ' | 'Ť' | 'Ŧ' | 'ţ' | 'ť' | 'ŧ' => 't',
        'Ù' | 'Ú' | 'Û' | 'Ũ' | 'Ū' | 'Ŭ' | 'Ů' | 'Ű' | 'Ų' | 'ù' | 'ú' | 'û' | 'ũ' | 'ū' | 'ŭ'
        | 'ů' | 'ű' | 'ų' => 'u',
        'Ŵ' | 'ŵ' => 'w',
        'Ý' | 'Ÿ' | 'Ŷ' | 'ý' | 'ÿ' | 'ŷ' => 'y',
        'Ź' | 'Ż' | 'Ž' | 'ź' | 'ż' | 'ž' => 'z',
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::id_from_title;

    #[test]
    fn titles_become_short_ids() {
        assert_eq!(id_from_title("Prüfung").as_deref(), Some("pruefung"));
        assert_eq!(id_from_title("Straße").as_deref(), Some("strasse"));
        assert_eq!(id_from_title("Größe").as_deref(), Some("groesse"));
        assert_eq!(id_from_title("Café!").as_deref(), Some("cafe"));
        assert_eq!(
            id_from_title("Hello, World").as_deref(),
            Some("hello-world")
        );
        assert_eq!(id_from_title("Α-φως").as_deref(), None);
        assert_eq!(id_from_title("!!!").as_deref(), None);
        assert_eq!(id_from_title("a---b").as_deref(), Some("a-b"));
        assert_eq!(id_from_title("-Hello-").as_deref(), Some("hello"));
        let long = "a".repeat(80);
        let id = id_from_title(&long).unwrap();
        assert_eq!(id.len(), 64);
        assert!(!id.ends_with('-'));
        assert!(!id.starts_with('-'));
        let cut = format!("{}-b", "a".repeat(63));
        assert_eq!(id_from_title(&cut).unwrap(), "a".repeat(63));
    }
}
