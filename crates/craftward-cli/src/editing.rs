//! Commands that read or change a project file.

use std::fmt::Write as _;
use std::process::ExitCode;
use std::time::SystemTime;

use craftward_core::{
    EditError, NewIssue, NewPhase, NewTask, NewWatch, ProjectChanges, ProjectUpdate, add_issue,
    add_phase, add_task, add_watch, edit_file, load_from_path, remove_issue, remove_phase,
    remove_task, remove_watch, render_project, set_issue_status, set_project, set_task_status,
    utc_date,
};

use crate::{Failure, resolve_project};

/// One parsed edit or show command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Request {
    Show {
        target: String,
    },
    SetProject {
        target: String,
        name: Option<String>,
        status: Option<String>,
        version: Update,
        description: Update,
        source_dir: Option<String>,
    },
    AddPhase {
        target: String,
        title: String,
        id: Option<String>,
        goal: Option<String>,
    },
    RemovePhase {
        target: String,
        phase: String,
        force: bool,
    },
    AddTask {
        target: String,
        phase: String,
        title: String,
        id: Option<String>,
        status: Option<String>,
        due: Option<String>,
        note: Option<String>,
    },
    SetTaskStatus {
        target: String,
        phase: String,
        task: String,
        status: String,
    },
    RemoveTask {
        target: String,
        phase: String,
        task: String,
    },
    AddIssue {
        target: String,
        title: String,
        severity: String,
        id: Option<String>,
        url: Option<String>,
        note: Option<String>,
    },
    SetIssueStatus {
        target: String,
        issue: String,
        status: String,
    },
    RemoveIssue {
        target: String,
        issue: String,
    },
    AddWatch {
        target: String,
        kind: String,
        title: String,
        due: String,
        id: Option<String>,
        note: Option<String>,
    },
    RemoveWatch {
        target: String,
        watch: String,
    },
}

/// A header field that stays, is replaced, or is removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Update {
    Keep,
    Set(String),
    Clear,
}

pub(crate) fn run(
    request: Request,
    workspace: Option<&std::path::Path>,
) -> Result<ExitCode, Failure> {
    match request {
        Request::Show { target } => {
            let path = resolve_project(&target, workspace)?;
            let project = load_from_path(&path).map_err(|error| Failure::Data(show_load(error)))?;
            print!("{}", render_project(&project));
            Ok(ExitCode::SUCCESS)
        }
        other => {
            let today = utc_date(SystemTime::now())
                .ok_or_else(|| Failure::Data("cannot read the system clock".to_owned()))?;
            run_change(&other, workspace, &today)
        }
    }
}

fn run_change(
    request: &Request,
    workspace: Option<&std::path::Path>,
    today: &str,
) -> Result<ExitCode, Failure> {
    let path = resolve_project(request.target(), workspace)?;
    let outcome = edit_file(&path, today, |project, updated_on| {
        apply(request, project, updated_on)
    });
    match outcome {
        Ok(done) => {
            println!("{}", confirmation(request, done.created_id.as_deref()));
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => Err(map_edit(&error)),
    }
}

fn apply(
    request: &Request,
    project: &craftward_core::Project,
    updated_on: &str,
) -> Result<craftward_core::EditOutcome, EditError> {
    match request {
        Request::SetProject {
            name,
            status,
            version,
            description,
            source_dir,
            ..
        } => set_project(
            project,
            ProjectChanges {
                name: name.as_deref(),
                status: status.as_deref(),
                version: version.as_update(),
                description: description.as_update(),
                source_dir: source_dir.as_deref(),
            },
            updated_on,
        ),
        Request::AddPhase {
            title, id, goal, ..
        } => add_phase(
            project,
            NewPhase {
                title,
                id: id.as_deref(),
                goal: goal.as_deref(),
            },
            updated_on,
        ),
        Request::RemovePhase { phase, force, .. } => {
            remove_phase(project, phase, *force, updated_on)
        }
        Request::AddTask {
            phase,
            title,
            id,
            status,
            due,
            note,
            ..
        } => add_task(
            project,
            NewTask {
                phase,
                title,
                id: id.as_deref(),
                status: status.as_deref(),
                due: due.as_deref(),
                note: note.as_deref(),
            },
            updated_on,
        ),
        Request::SetTaskStatus {
            phase,
            task,
            status,
            ..
        } => set_task_status(project, phase, task, status, updated_on),
        Request::RemoveTask { phase, task, .. } => remove_task(project, phase, task, updated_on),
        Request::AddIssue { .. }
        | Request::SetIssueStatus { .. }
        | Request::RemoveIssue { .. }
        | Request::AddWatch { .. }
        | Request::RemoveWatch { .. } => apply_record(request, project, updated_on),
        Request::Show { .. } => unreachable!("show does not change the file"),
    }
}

fn apply_record(
    request: &Request,
    project: &craftward_core::Project,
    updated_on: &str,
) -> Result<craftward_core::EditOutcome, EditError> {
    match request {
        Request::AddIssue {
            title,
            severity,
            id,
            url,
            note,
            ..
        } => add_issue(
            project,
            NewIssue {
                title,
                severity,
                id: id.as_deref(),
                url: url.as_deref(),
                note: note.as_deref(),
            },
            updated_on,
        ),
        Request::SetIssueStatus { issue, status, .. } => {
            set_issue_status(project, issue, status, updated_on)
        }
        Request::RemoveIssue { issue, .. } => remove_issue(project, issue, updated_on),
        Request::AddWatch {
            kind,
            title,
            due,
            id,
            note,
            ..
        } => add_watch(
            project,
            NewWatch {
                kind,
                title,
                due,
                id: id.as_deref(),
                note: note.as_deref(),
            },
            updated_on,
        ),
        Request::RemoveWatch { watch, .. } => remove_watch(project, watch, updated_on),
        Request::Show { .. }
        | Request::SetProject { .. }
        | Request::AddPhase { .. }
        | Request::RemovePhase { .. }
        | Request::AddTask { .. }
        | Request::SetTaskStatus { .. }
        | Request::RemoveTask { .. } => unreachable!("handled by apply"),
    }
}

fn confirmation(request: &Request, created: Option<&str>) -> String {
    match request {
        Request::SetProject { .. } => "updated project".to_owned(),
        Request::AddPhase { .. } => format!("added phase {}", created.unwrap_or("?")),
        Request::RemovePhase { phase, .. } => format!("removed phase {phase}"),
        Request::AddTask { .. } => format!("added task {}", created.unwrap_or("?")),
        Request::SetTaskStatus { task, status, .. } => format!("set task {task} to {status}"),
        Request::RemoveTask { task, .. } => format!("removed task {task}"),
        Request::AddIssue { .. } => format!("added issue {}", created.unwrap_or("?")),
        Request::SetIssueStatus { issue, status, .. } => {
            format!("set issue {issue} to {status}")
        }
        Request::RemoveIssue { issue, .. } => format!("removed issue {issue}"),
        Request::AddWatch { .. } => format!("added watch {}", created.unwrap_or("?")),
        Request::RemoveWatch { watch, .. } => format!("removed watch {watch}"),
        Request::Show { .. } => String::new(),
    }
}

fn map_edit(error: &EditError) -> Failure {
    if error.is_usage() {
        Failure::Usage(error.to_string())
    } else {
        Failure::Data(error.to_string())
    }
}

fn show_load(error: craftward_core::LoadError) -> String {
    match error {
        craftward_core::LoadError::Validation(errors) => {
            let mut message = String::from("invalid project file");
            for item in errors {
                let _ = write!(message, "\n  {item}");
            }
            message
        }
        other => other.to_string(),
    }
}

impl Request {
    fn target(&self) -> &str {
        match self {
            Self::Show { target }
            | Self::SetProject { target, .. }
            | Self::AddPhase { target, .. }
            | Self::RemovePhase { target, .. }
            | Self::AddTask { target, .. }
            | Self::SetTaskStatus { target, .. }
            | Self::RemoveTask { target, .. }
            | Self::AddIssue { target, .. }
            | Self::SetIssueStatus { target, .. }
            | Self::RemoveIssue { target, .. }
            | Self::AddWatch { target, .. }
            | Self::RemoveWatch { target, .. } => target,
        }
    }
}

impl Update {
    fn as_update(&self) -> ProjectUpdate<'_> {
        match self {
            Self::Keep => ProjectUpdate::Keep,
            Self::Set(text) => ProjectUpdate::Set(text),
            Self::Clear => ProjectUpdate::Clear,
        }
    }
}
