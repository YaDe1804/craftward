//! Command-line interface for Craftward.

mod cli;
mod deadlines;
mod editing;
mod findings;
mod radar;
mod stack;

use std::env;
use std::fmt::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::SystemTime;

use clap::Parser;
use craftward_core::{
    Language, LoadError, ProcessHost, ProjectRow, ProjectStatus, WorkspaceError, add_project,
    init_project, is_portable_absolute, list_projects, load_from_path, locate_workspace,
    missing_paths, platform_path, project_file, project_file_by_id, remove_project, utc_date,
    write_roadmap,
};

use crate::cli::{
    Cli, Command, FindingsCommand, IssueCommand, PhaseCommand, ProjectCommand, StackCommand,
    StackLockfileCommand, TaskCommand, WatchCommand, WorkspaceCommand,
};

/// Process exit status for a usage error.
const USAGE_ERROR: u8 = 2;

/// Process exit status when a file or request fails.
const DATA_ERROR: u8 = 1;

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            let _ = error.print();
            return ExitCode::from(u8::try_from(error.exit_code()).unwrap_or(USAGE_ERROR));
        }
    };
    match run(cli) {
        Ok(code) => code,
        Err(Failure::Data(message)) => {
            eprintln!("error: {message}");
            ExitCode::from(DATA_ERROR)
        }
        Err(Failure::Usage(message)) => {
            eprintln!("error: {message}");
            ExitCode::from(USAGE_ERROR)
        }
    }
}

/// Why a command stopped. Usage errors exit 2; everything else exits 1.
pub(crate) enum Failure {
    Usage(String),
    Data(String),
}

impl From<String> for Failure {
    fn from(message: String) -> Self {
        Self::Data(message)
    }
}

#[allow(clippy::too_many_lines)]
fn run(cli: Cli) -> Result<ExitCode, Failure> {
    let Some(command) = cli.command else {
        if cli.workspace.is_some() {
            return Err(Failure::Usage("--workspace needs a command".to_owned()));
        }
        println!("{} {}", env!("CARGO_BIN_NAME"), craftward_core::version());
        return Ok(ExitCode::SUCCESS);
    };
    let workspace = cli.workspace.as_deref();
    match command {
        Command::Init {
            directory,
            id,
            name,
            status,
        } => run_init(directory.as_deref(), &id, &name, status.into()),
        Command::Add { path } => {
            let workspace = workspace_file(workspace)?;
            let input = absolute_from_cwd(&platform_path(&path))?;
            match add_project(&workspace, &input) {
                Ok(id) => {
                    println!("added {id}");
                    Ok(ExitCode::SUCCESS)
                }
                Err(error) => Err(Failure::Data(explain_workspace(&error))),
            }
        }
        Command::Remove { id } => {
            let workspace = workspace_file(workspace)?;
            match remove_project(&workspace, &id) {
                Ok(()) => {
                    println!("removed {id}");
                    Ok(ExitCode::SUCCESS)
                }
                Err(error) => Err(Failure::Data(error.to_string())),
            }
        }
        Command::List => {
            let workspace = workspace_file(workspace)?;
            match list_projects(&workspace) {
                Ok(rows) => Ok(print_list(&rows)),
                Err(error) => Err(Failure::Data(explain_workspace(&error))),
            }
        }
        Command::Validate { paths } => {
            if paths.is_empty() {
                let workspace = workspace_file(workspace)?;
                Ok(validate_workspace(&workspace)?)
            } else {
                let paths = paths
                    .iter()
                    .map(|path| platform_path(path))
                    .collect::<Vec<_>>();
                Ok(validate_files(&paths))
            }
        }
        Command::Workspace {
            command: WorkspaceCommand::Path,
        } => {
            let location =
                locate_workspace(&ProcessHost, workspace).map_err(|error| error.to_string())?;
            let path = absolute_from_cwd(&location.path)?;
            println!("{} ({})", path.display(), location.origin.label());
            Ok(ExitCode::SUCCESS)
        }
        Command::Roadmap {
            target,
            directory,
            language,
        } => run_roadmap(&target, directory.as_deref(), language, workspace),
        Command::Show { project } => {
            editing::run(editing::Request::Show { target: project }, workspace)
        }
        Command::Project {
            command:
                ProjectCommand::Set {
                    project,
                    name,
                    status,
                    version,
                    description,
                    source_dir,
                    tags,
                    clear_tags,
                    links,
                    clear_links,
                },
        } => {
            if name.is_none()
                && status.is_none()
                && version.is_none()
                && description.is_none()
                && source_dir.is_none()
                && tags.is_none()
                && !clear_tags
                && links.is_empty()
                && !clear_links
            {
                return Err(Failure::Usage(
                    "project set requires at least one option".to_owned(),
                ));
            }
            let parsed_links = if links.is_empty() {
                None
            } else {
                Some(parse_link_args(&links)?)
            };
            let parsed_tags = tags.map(|text| parse_tags(&text));
            editing::run(
                editing::Request::SetProject {
                    target: project,
                    name,
                    status: status.map(|value| value.as_str().to_owned()),
                    version: clearable(version),
                    description: clearable(description),
                    source_dir: clearable(source_dir),
                    tags: parsed_tags,
                    clear_tags,
                    links: parsed_links,
                    clear_links,
                },
                workspace,
            )
        }
        Command::Phase { command } => editing::run(phase_request(command)?, workspace),
        Command::Task { command } => editing::run(task_request(command)?, workspace),
        Command::Issue { command } => editing::run(issue_request(command)?, workspace),
        Command::Watch { command } => editing::run(watch_request(command), workspace),
        Command::Radar {
            projects,
            refresh,
            offline,
            json,
            show_all,
            strict,
            eol_warning_days,
            deadline_warning_days,
        } => {
            let mode = if refresh {
                radar::ScanMode::Refresh
            } else if offline {
                radar::ScanMode::Offline
            } else {
                radar::ScanMode::Normal
            };
            radar::run(
                &radar::Request {
                    projects,
                    mode,
                    json,
                    show_all,
                    cache_dir: cli.cache_dir.clone(),
                    data_dir: cli.data_dir.clone(),
                    strict,
                    eol_warning_days,
                    deadline_warning_days,
                },
                workspace,
            )
        }
        Command::Findings {
            command,
            projects,
            state,
            kind,
            json,
        } => {
            let request = match command {
                None => findings::Request::List(findings::ListRequest {
                    projects,
                    state: state.into(),
                    kind: kind.map(|value| value.as_str().to_owned()),
                    json,
                    data_dir: cli.data_dir.clone(),
                }),
                Some(FindingsCommand::Show { id, json }) => {
                    findings::Request::Show(findings::IdRequest {
                        id,
                        json,
                        data_dir: cli.data_dir.clone(),
                    })
                }
                Some(FindingsCommand::Ignore { id, reason, until }) => {
                    findings::Request::Ignore(findings::IgnoreRequest {
                        id,
                        reason,
                        until,
                        data_dir: cli.data_dir.clone(),
                    })
                }
                Some(FindingsCommand::Reopen { id }) => {
                    findings::Request::Reopen(findings::IdRequest {
                        id,
                        json: false,
                        data_dir: cli.data_dir.clone(),
                    })
                }
            };
            findings::run(&request, workspace)
        }
        Command::Stack { command } => stack::run(&stack_request(command), workspace),
        Command::Deadlines => deadlines::run(&deadlines::Request {
            data_dir: cli.data_dir.clone(),
        }),
    }
}

fn clearable(value: Option<String>) -> editing::Update {
    match value {
        None => editing::Update::Keep,
        Some(text) if text.is_empty() => editing::Update::Clear,
        Some(text) => editing::Update::Set(text),
    }
}

fn clearable_stack(value: Option<String>) -> stack::Update {
    match value {
        None => stack::Update::Keep,
        Some(text) if text.is_empty() => stack::Update::Clear,
        Some(text) => stack::Update::Set(text),
    }
}

fn phase_request(command: PhaseCommand) -> Result<editing::Request, Failure> {
    match command {
        PhaseCommand::Add {
            project,
            title,
            id,
            goal,
        } => Ok(editing::Request::AddPhase {
            target: project,
            title,
            id,
            goal,
        }),
        PhaseCommand::Set {
            project,
            phase,
            title,
            goal,
        } => {
            if title.is_none() && goal.is_none() {
                return Err(Failure::Usage(
                    "phase set requires at least one of --title or --goal".to_owned(),
                ));
            }
            Ok(editing::Request::SetPhase {
                target: project,
                phase,
                title,
                goal: clearable(goal),
            })
        }
        PhaseCommand::Remove {
            project,
            phase,
            force,
        } => Ok(editing::Request::RemovePhase {
            target: project,
            phase,
            force,
        }),
    }
}

fn task_request(command: TaskCommand) -> Result<editing::Request, Failure> {
    match command {
        TaskCommand::Add {
            project,
            phase,
            title,
            id,
            status,
            due,
            note,
        } => Ok(editing::Request::AddTask {
            target: project,
            phase,
            title,
            id,
            status: status.map(|value| value.as_str().to_owned()),
            due,
            note,
        }),
        TaskCommand::Set {
            project,
            phase,
            task,
            title,
            due,
            note,
        } => {
            if title.is_none() && due.is_none() && note.is_none() {
                return Err(Failure::Usage(
                    "task set requires at least one of --title, --due, or --note".to_owned(),
                ));
            }
            Ok(editing::Request::SetTask {
                target: project,
                phase,
                task,
                title,
                due: clearable(due),
                note: clearable(note),
            })
        }
        TaskCommand::Status {
            project,
            phase,
            task,
            status,
        } => Ok(editing::Request::SetTaskStatus {
            target: project,
            phase,
            task,
            status: status.as_str().to_owned(),
        }),
        TaskCommand::Done {
            project,
            phase,
            task,
        } => Ok(editing::Request::SetTaskStatus {
            target: project,
            phase,
            task,
            status: "done".to_owned(),
        }),
        TaskCommand::Remove {
            project,
            phase,
            task,
        } => Ok(editing::Request::RemoveTask {
            target: project,
            phase,
            task,
        }),
    }
}

fn issue_request(command: IssueCommand) -> Result<editing::Request, Failure> {
    match command {
        IssueCommand::Add {
            project,
            title,
            severity,
            id,
            url,
            note,
        } => Ok(editing::Request::AddIssue {
            target: project,
            title,
            severity: severity.as_str().to_owned(),
            id,
            url,
            note,
        }),
        IssueCommand::Set {
            project,
            issue,
            title,
            severity,
            url,
            note,
        } => {
            if title.is_none() && severity.is_none() && url.is_none() && note.is_none() {
                return Err(Failure::Usage(
                    "issue set requires at least one of --title, --severity, --url, or --note"
                        .to_owned(),
                ));
            }
            Ok(editing::Request::SetIssue {
                target: project,
                issue,
                title,
                severity: severity.map(|value| value.as_str().to_owned()),
                url: clearable(url),
                note: clearable(note),
            })
        }
        IssueCommand::Status {
            project,
            issue,
            status,
        } => Ok(editing::Request::SetIssueStatus {
            target: project,
            issue,
            status: status.as_str().to_owned(),
        }),
        IssueCommand::Remove { project, issue } => Ok(editing::Request::RemoveIssue {
            target: project,
            issue,
        }),
    }
}

fn parse_tags(text: &str) -> Vec<String> {
    text.split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .collect()
}

fn parse_link_args(args: &[String]) -> Result<Vec<(String, String)>, Failure> {
    let mut links = Vec::with_capacity(args.len());
    for arg in args {
        let Some((label, url)) = arg.split_once('=') else {
            return Err(Failure::Usage(format!(
                "invalid --link {arg}: expected LABEL=URL"
            )));
        };
        let label = label.trim();
        let url = url.trim();
        if label.is_empty() || url.is_empty() {
            return Err(Failure::Usage(format!(
                "invalid --link {arg}: expected LABEL=URL"
            )));
        }
        links.push((label.to_owned(), url.to_owned()));
    }
    Ok(links)
}

fn watch_request(command: WatchCommand) -> editing::Request {
    match command {
        WatchCommand::Add {
            project,
            kind,
            title,
            due,
            id,
            note,
        } => editing::Request::AddWatch {
            target: project,
            kind: kind.as_str().to_owned(),
            title,
            due,
            id,
            note,
        },
        WatchCommand::Remove { project, watch } => editing::Request::RemoveWatch {
            target: project,
            watch,
        },
    }
}

fn stack_request(command: StackCommand) -> stack::Request {
    match command {
        StackCommand::Add {
            project,
            kind,
            name,
            version,
            note,
            eol_product,
        } => stack::Request::Add(stack::AddRequest {
            project,
            kind: kind.as_str().to_owned(),
            name,
            version,
            note,
            eol_product,
        }),
        StackCommand::Set {
            project,
            name,
            version,
            note,
            eol_product,
        } => stack::Request::Set(stack::SetRequest {
            project,
            name,
            version: clearable_stack(version),
            note: clearable_stack(note),
            eol_product: clearable_stack(eol_product),
        }),
        StackCommand::Remove { project, name } => {
            stack::Request::Remove(stack::NameRequest { project, name })
        }
        StackCommand::Lockfile {
            command: StackLockfileCommand::Add { project, path },
        } => stack::Request::AddLockfile(stack::PathRequest { project, path }),
        StackCommand::Lockfile {
            command: StackLockfileCommand::Remove { project, path },
        } => stack::Request::RemoveLockfile(stack::PathRequest { project, path }),
        StackCommand::Android {
            project,
            target_sdk,
            min_sdk,
            form_factor,
            store,
            detect_from,
            clear_detect_from,
        } => stack::Request::Android(stack::AndroidRequest {
            project,
            target_sdk: clearable_stack(target_sdk),
            min_sdk: clearable_stack(min_sdk),
            form_factor: form_factor.map(|value| value.as_str().to_owned()),
            store: store.map(|value| value.as_str().to_owned()),
            detect_from,
            clear_detect_from,
        }),
    }
}

fn run_init(
    directory: Option<&Path>,
    id: &str,
    name: &str,
    status: ProjectStatus,
) -> Result<ExitCode, Failure> {
    let directory = match directory {
        Some(path) => absolute_from_cwd(path)?,
        None => env::current_dir()
            .map_err(|error| format!("cannot read the current directory: {error}"))?,
    };
    match init_project(&directory, id, name, status) {
        Ok(path) => {
            println!("created {}", path.display());
            Ok(ExitCode::SUCCESS)
        }
        Err(WorkspaceError::Validation(errors)) => {
            let mut message = String::from("invalid project file");
            for error in errors {
                let _ = write!(message, "\n  {}: {}", error.path, error.message);
            }
            Err(Failure::Data(message))
        }
        Err(error) => Err(Failure::Data(error.to_string())),
    }
}

fn run_roadmap(
    target: &str,
    directory: Option<&Path>,
    language: Language,
    workspace: Option<&Path>,
) -> Result<ExitCode, Failure> {
    let project = resolve_project(target, workspace)?;
    let directory = match directory {
        Some(path) => absolute_from_cwd(path)?,
        None => env::current_dir()
            .map_err(|error| format!("cannot read the current directory: {error}"))?,
    };
    let generated_on =
        utc_date(SystemTime::now()).ok_or_else(|| "cannot read the system clock".to_owned())?;
    match write_roadmap(&project, &directory, language, &generated_on) {
        Ok(path) => {
            println!("{}", path.display());
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => Err(Failure::Data(error.to_string())),
    }
}

pub(crate) fn resolve_project(target: &str, workspace: Option<&Path>) -> Result<PathBuf, Failure> {
    let path = platform_path(Path::new(target));
    let exists = path.exists();
    let registered = match registered_project_id(target, workspace) {
        Ok(found) => found,
        Err(_) if is_path_target(target, &path) => false,
        Err(error) => return Err(Failure::Data(error)),
    };
    if exists && registered {
        let kind = if path.is_dir() { "folder" } else { "path" };
        let separator = std::path::MAIN_SEPARATOR;
        return Err(Failure::Usage(format!(
            "`{target}` is both a workspace project id and an existing {kind}. \
             Pass `.{separator}{target}` to use the {kind}, or pass the path of \
             the project file to use the registered project."
        )));
    }
    if is_path_target(target, &path) {
        let absolute = absolute_from_cwd(&path)?;
        return project_file(&absolute).map_err(|error| Failure::Data(error.to_string()));
    }
    let workspace = workspace_file(workspace)?;
    project_file_by_id(&workspace, target).map_err(|error| Failure::Data(error.to_string()))
}

fn registered_project_id(target: &str, workspace: Option<&Path>) -> Result<bool, String> {
    let file = workspace_file(workspace)?;
    match project_file_by_id(&file, target) {
        Ok(_) => Ok(true),
        Err(WorkspaceError::UnknownId(_)) => Ok(false),
        Err(error) => Err(error.to_string()),
    }
}

fn is_path_target(raw: &str, path: &Path) -> bool {
    path.exists()
        || Path::new(raw)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
        || raw.contains('/')
        || raw.contains('\\')
        || is_portable_absolute(raw)
}

pub(crate) fn workspace_file(explicit: Option<&Path>) -> Result<PathBuf, String> {
    let location = locate_workspace(&ProcessHost, explicit).map_err(|error| error.to_string())?;
    absolute_from_cwd(&location.path)
}

fn absolute_from_cwd(path: &Path) -> Result<PathBuf, String> {
    let path = platform_path(path);
    if path.is_absolute() {
        Ok(path)
    } else {
        let current = env::current_dir()
            .map_err(|error| format!("cannot read the current directory: {error}"))?;
        Ok(platform_path(&current.join(path)))
    }
}

fn validate_workspace(path: &Path) -> Result<ExitCode, String> {
    let rows = list_projects(path).map_err(|error| explain_workspace(&error))?;
    if rows.is_empty() {
        println!("no projects");
        return Ok(ExitCode::SUCCESS);
    }
    let mut failed = false;
    for row in rows {
        if !report_project(&row.path) {
            failed = true;
        }
    }
    Ok(exit_from_failure(failed))
}

fn validate_files(paths: &[PathBuf]) -> ExitCode {
    let mut failed = false;
    for path in paths {
        if !report_project(path) {
            failed = true;
        }
    }
    exit_from_failure(failed)
}

fn report_project(path: &Path) -> bool {
    let rendered = path.to_string_lossy();
    match load_from_path(path) {
        Ok(project) => {
            println!("{rendered}: ok");
            for missing in missing_paths(path, &project) {
                eprintln!("warning: {missing}");
            }
            true
        }
        Err(error) => {
            println!("{rendered}:");
            print_load_error(&error);
            false
        }
    }
}

fn print_load_error(error: &LoadError) {
    match error {
        LoadError::Read(source) => println!("  failed to read file: {source}"),
        LoadError::Syntax {
            line,
            column,
            message,
        } => println!("  syntax error at line {line}, column {column}: {message}"),
        LoadError::UnsupportedSchema { .. } => {
            println!("  schema_version: file was written by a newer Craftward version");
        }
        LoadError::Validation(errors) => {
            for error in errors {
                println!("  {}: {}", error.path, error.message);
            }
        }
    }
}

fn print_list(rows: &[ProjectRow]) -> ExitCode {
    if rows.is_empty() {
        println!("no projects");
        return ExitCode::SUCCESS;
    }
    let mut failed = false;
    let mut lines = Vec::new();
    for row in rows {
        lines.push(match &row.detail {
            Ok(snapshot) => ListLine {
                id: snapshot.id.clone(),
                name: snapshot.name.clone(),
                status: snapshot.status.as_str().to_owned(),
                version: snapshot
                    .current_version
                    .clone()
                    .unwrap_or_else(|| "-".to_owned()),
                open_tasks: snapshot.open_tasks.to_string(),
                open_issues: snapshot.open_issues.to_string(),
                next_due: snapshot.next_due.clone().unwrap_or_else(|| "-".to_owned()),
                note: String::new(),
            },
            Err(message) => {
                failed = true;
                ListLine {
                    id: "-".to_owned(),
                    name: "-".to_owned(),
                    status: "-".to_owned(),
                    version: "-".to_owned(),
                    open_tasks: "-".to_owned(),
                    open_issues: "-".to_owned(),
                    next_due: "-".to_owned(),
                    note: format!("{}: {message}", row.path.display()),
                }
            }
        });
    }
    print_table(&lines);
    exit_from_failure(failed)
}

struct ListLine {
    id: String,
    name: String,
    status: String,
    version: String,
    open_tasks: String,
    open_issues: String,
    next_due: String,
    note: String,
}

fn print_table(lines: &[ListLine]) {
    let headers = [
        "ID",
        "NAME",
        "STATUS",
        "VERSION",
        "OPEN_TASKS",
        "OPEN_ISSUES",
        "NEXT_DUE",
        "NOTE",
    ];
    let mut widths = headers.map(str::len);
    for line in lines {
        let cells = line.cells();
        for (width, cell) in widths.iter_mut().zip(cells) {
            *width = (*width).max(cell.len());
        }
    }
    print_row(&headers, &widths);
    for line in lines {
        let cells = line.cells();
        print_row(&cells, &widths);
    }
}

impl ListLine {
    fn cells(&self) -> [&str; 8] {
        [
            &self.id,
            &self.name,
            &self.status,
            &self.version,
            &self.open_tasks,
            &self.open_issues,
            &self.next_due,
            &self.note,
        ]
    }
}

fn print_row(cells: &[&str], widths: &[usize]) {
    let mut first = true;
    for (cell, width) in cells.iter().zip(widths) {
        if first {
            first = false;
        } else {
            print!("  ");
        }
        print!("{cell:<width$}");
    }
    println!();
}

fn exit_from_failure(failed: bool) -> ExitCode {
    if failed {
        ExitCode::from(DATA_ERROR)
    } else {
        ExitCode::SUCCESS
    }
}

fn explain_workspace(error: &WorkspaceError) -> String {
    match error {
        WorkspaceError::InvalidProject { path, source } => {
            let mut message = format!("invalid project file {}", path.display());
            match source {
                LoadError::Validation(errors) => {
                    for error in errors {
                        let _ = write!(message, "\n  {}: {}", error.path, error.message);
                    }
                }
                other => {
                    let _ = write!(message, "\n  {other}");
                }
            }
            message
        }
        WorkspaceError::Validation(errors) => {
            let mut message = String::from("invalid workspace file");
            for error in errors {
                let _ = write!(message, "\n  {}: {}", error.path, error.message);
            }
            message
        }
        other => other.to_string(),
    }
}
