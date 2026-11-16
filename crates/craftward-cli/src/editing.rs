//! Commands that read or change a project file.

use std::fmt::Write as _;
use std::process::ExitCode;
use std::time::SystemTime;

use craftward_core::{
    EditError, IssueChanges, LinkChange, LinkListUpdate, NewIssue, NewPhase, NewTask, NewWatch,
    PhaseChanges, ProjectChanges, ProjectUpdate, StringListUpdate, TaskChanges, add_issue,
    add_phase, add_task, add_watch, edit_file, load_from_path, remove_issue, remove_phase,
    remove_task, remove_watch, render_project, set_issue, set_issue_status, set_phase, set_project,
    set_task, set_task_status, utc_date,
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
        source_dir: Update,
        tags: Option<Vec<String>>,
        clear_tags: bool,
        links: Option<Vec<(String, String)>>,
        clear_links: bool,
    },
    AddPhase {
        target: String,
        title: String,
        id: Option<String>,
        goal: Option<String>,
    },
    SetPhase {
        target: String,
        phase: String,
        title: Option<String>,
        goal: Update,
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
    SetTask {
        target: String,
        phase: String,
        task: String,
        title: Option<String>,
        due: Update,
        note: Update,
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
    SetIssue {
        target: String,
        issue: String,
        title: Option<String>,
        severity: Option<String>,
        url: Update,
        note: Update,
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
        Request::SetProject { .. } => apply_set_project(request, project, updated_on),
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
        Request::SetPhase {
            phase, title, goal, ..
        } => set_phase(
            project,
            phase,
            PhaseChanges {
                title: title.as_deref(),
                goal: goal.as_update(),
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
        Request::SetTask {
            phase,
            task,
            title,
            due,
            note,
            ..
        } => set_task(
            project,
            phase,
            task,
            TaskChanges {
                title: title.as_deref(),
                due: due.as_update(),
                note: note.as_update(),
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
        | Request::SetIssue { .. }
        | Request::SetIssueStatus { .. }
        | Request::RemoveIssue { .. }
        | Request::AddWatch { .. }
        | Request::RemoveWatch { .. } => apply_record(request, project, updated_on),
        Request::Show { .. } => unreachable!("show does not change the file"),
    }
}

fn apply_set_project(
    request: &Request,
    project: &craftward_core::Project,
    updated_on: &str,
) -> Result<craftward_core::EditOutcome, EditError> {
    let Request::SetProject {
        name,
        status,
        version,
        description,
        source_dir,
        tags,
        clear_tags,
        links,
        clear_links,
        ..
    } = request
    else {
        unreachable!("apply_set_project expects SetProject");
    };
    let tags_update = if *clear_tags {
        StringListUpdate::Clear
    } else if let Some(tags) = tags {
        StringListUpdate::Set(tags.as_slice())
    } else {
        StringListUpdate::Keep
    };
    let link_changes: Vec<LinkChange<'_>> = links.as_ref().map_or_default(|items| {
        items
            .iter()
            .map(|(label, url)| LinkChange {
                label: label.as_str(),
                url: url.as_str(),
            })
            .collect()
    });
    let links_update = if *clear_links {
        LinkListUpdate::Clear
    } else if links.is_some() {
        LinkListUpdate::Set(&link_changes)
    } else {
        LinkListUpdate::Keep
    };
    set_project(
        project,
        ProjectChanges {
            name: name.as_deref(),
            status: status.as_deref(),
            version: version.as_update(),
            description: description.as_update(),
            source_dir: source_dir.as_update(),
            tags: tags_update,
            links: links_update,
        },
        updated_on,
    )
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
        Request::SetIssue {
            issue,
            title,
            severity,
            url,
            note,
            ..
        } => set_issue(
            project,
            issue,
            IssueChanges {
                title: title.as_deref(),
                severity: severity.as_deref(),
                url: url.as_update(),
                note: note.as_update(),
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
        | Request::SetPhase { .. }
        | Request::RemovePhase { .. }
        | Request::AddTask { .. }
        | Request::SetTask { .. }
        | Request::SetTaskStatus { .. }
        | Request::RemoveTask { .. } => unreachable!("handled by apply"),
    }
}

fn confirmation(request: &Request, created: Option<&str>) -> String {
    match request {
        Request::SetProject { .. } => "updated project".to_owned(),
        Request::AddPhase { .. } => format!("added phase {}", created.unwrap_or("?")),
        Request::SetPhase { phase, .. } => format!("updated phase {phase}"),
        Request::RemovePhase { phase, .. } => format!("removed phase {phase}"),
        Request::AddTask { .. } => format!("added task {}", created.unwrap_or("?")),
        Request::SetTask { task, .. } => format!("updated task {task}"),
        Request::SetTaskStatus { task, status, .. } => format!("set task {task} to {status}"),
        Request::RemoveTask { task, .. } => format!("removed task {task}"),
        Request::AddIssue { .. } => format!("added issue {}", created.unwrap_or("?")),
        Request::SetIssue { issue, .. } => format!("updated issue {issue}"),
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
            | Self::SetPhase { target, .. }
            | Self::RemovePhase { target, .. }
            | Self::AddTask { target, .. }
            | Self::SetTask { target, .. }
            | Self::SetTaskStatus { target, .. }
            | Self::RemoveTask { target, .. }
            | Self::AddIssue { target, .. }
            | Self::SetIssue { target, .. }
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
