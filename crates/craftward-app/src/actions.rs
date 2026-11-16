//! Desktop edits that call the same craftward-core writers as the CLI.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use craftward_core::{
    ContentFingerprint, EditError, EditOutcome, IssueChanges, LinkChange, LinkListUpdate, NewIssue,
    NewPhase, NewTask, PhaseChanges, ProjectChanges, ProjectStatus, StringListUpdate, TaskChanges,
    UndoOffer, UndoSnapshot, add_issue, add_phase, add_project_if_unchanged, add_task,
    content_fingerprint, edit_file_if_unchanged, init_project, remove_issue, remove_phase,
    remove_project_if_unchanged, remove_task, set_issue, set_issue_status, set_phase, set_project,
    set_task, set_task_status, utc_date, workspace_bytes_or_empty,
};

use crate::view::{
    HeaderForm, IssueEditForm, IssueForm, NewProjectForm, PhaseForm, TaskEditForm, TaskForm,
};

/// Result of a desktop save attempt.
#[derive(Debug)]
pub enum SaveResult {
    /// File(s) written successfully.
    Saved {
        /// Toast text.
        message: String,
        /// Snapshots for undo.
        snapshots: Vec<UndoSnapshot>,
    },
    /// Validation or I/O failed; the draft should stay open.
    Failed {
        /// Message shown next to the control that triggered the save.
        message: String,
        /// True when the on-disk file drifted; the UI should offer Reload.
        conflict: bool,
    },
}

impl SaveResult {
    /// Builds an undo offer when this was a successful save.
    ///
    /// An empty snapshot list still shows the toast (message + Dismiss) but
    /// has no Undo action — used when a file deletion is irreversible.
    #[must_use]
    pub fn undo_offer(&self, now_secs: u64) -> Option<UndoOffer> {
        match self {
            Self::Saved { message, snapshots } => {
                Some(UndoOffer::new(message.clone(), snapshots.clone(), now_secs))
            }
            Self::Failed { .. } => None,
        }
    }
}

/// Seconds since the Unix epoch for the undo clock.
#[must_use]
pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

/// Sets a task status through the shared core writer.
#[must_use]
pub fn save_task_status(
    path: &Path,
    fingerprint: &ContentFingerprint,
    phase_id: &str,
    task_id: &str,
    status: &str,
    title: &str,
) -> SaveResult {
    let label = status_label(status);
    apply(
        path,
        fingerprint,
        format!("Status of {title} set to {label}"),
        |project, updated_on| set_task_status(project, phase_id, task_id, status, updated_on),
    )
}

/// Sets an issue status through the shared core writer.
#[must_use]
pub fn save_issue_status(
    path: &Path,
    fingerprint: &ContentFingerprint,
    issue_id: &str,
    status: &str,
    title: &str,
) -> SaveResult {
    let label = status_label(status);
    apply(
        path,
        fingerprint,
        format!("Status of {title} set to {label}"),
        |project, updated_on| set_issue_status(project, issue_id, status, updated_on),
    )
}

/// Adds a task from a validated form.
#[must_use]
pub fn save_new_task(path: &Path, fingerprint: &ContentFingerprint, form: &TaskForm) -> SaveResult {
    if let Some(message) = form.validation_message() {
        return failed(message);
    }
    let title = form.title_for_edit().to_owned();
    apply(
        path,
        fingerprint,
        format!("Added task {title}"),
        |project, updated_on| {
            add_task(
                project,
                NewTask {
                    phase: &form.phase_id,
                    title: form.title_for_edit(),
                    id: None,
                    status: None,
                    due: form.due_for_edit(),
                    note: None,
                },
                updated_on,
            )
        },
    )
}

/// Adds an issue from a validated form.
#[must_use]
pub fn save_new_issue(
    path: &Path,
    fingerprint: &ContentFingerprint,
    form: &IssueForm,
) -> SaveResult {
    if let Some(message) = form.validation_message() {
        return failed(message);
    }
    let title = form.title_for_edit().to_owned();
    apply(
        path,
        fingerprint,
        format!("Added issue {title}"),
        |project, updated_on| {
            add_issue(
                project,
                NewIssue {
                    title: form.title_for_edit(),
                    severity: &form.severity,
                    id: None,
                    url: form.url_for_edit(),
                    note: form.note_for_edit(),
                },
                updated_on,
            )
        },
    )
}

/// Saves overview header fields.
#[must_use]
pub fn save_header(path: &Path, fingerprint: &ContentFingerprint, form: &HeaderForm) -> SaveResult {
    if let Some(message) = form.validation_message() {
        return failed(message);
    }
    let tags = form.tags_for_edit();
    let links = form.links_for_edit();
    let link_changes: Vec<LinkChange<'_>> = links
        .iter()
        .map(|(label, url)| LinkChange {
            label: label.as_str(),
            url: url.as_str(),
        })
        .collect();
    apply(
        path,
        fingerprint,
        "Updated project header".to_owned(),
        |project, updated_on| {
            set_project(
                project,
                ProjectChanges {
                    name: Some(form.name_for_edit()),
                    status: Some(form.status.as_str()),
                    version: form.version_update(),
                    description: form.description_update(),
                    source_dir: form.source_dir_update(),
                    tags: if tags.is_empty() {
                        StringListUpdate::Clear
                    } else {
                        StringListUpdate::Set(tags.as_slice())
                    },
                    links: if link_changes.is_empty() {
                        LinkListUpdate::Clear
                    } else {
                        LinkListUpdate::Set(&link_changes)
                    },
                },
                updated_on,
            )
        },
    )
}

/// Adds a phase.
#[must_use]
pub fn save_new_phase(
    path: &Path,
    fingerprint: &ContentFingerprint,
    form: &PhaseForm,
) -> SaveResult {
    if let Some(message) = form.validation_message() {
        return failed(message);
    }
    let title = form.title_for_edit().to_owned();
    apply(
        path,
        fingerprint,
        format!("Added phase {title}"),
        |project, updated_on| {
            add_phase(
                project,
                NewPhase {
                    title: form.title_for_edit(),
                    id: form.id_for_edit(),
                    goal: form.goal_for_edit(),
                },
                updated_on,
            )
        },
    )
}

/// Updates a phase title/goal.
#[must_use]
pub fn save_phase_edit(
    path: &Path,
    fingerprint: &ContentFingerprint,
    phase_id: &str,
    form: &PhaseForm,
) -> SaveResult {
    if let Some(message) = form.validation_message() {
        return failed(message);
    }
    apply(
        path,
        fingerprint,
        format!("Updated phase {}", form.title_for_edit()),
        |project, updated_on| {
            set_phase(
                project,
                phase_id,
                PhaseChanges {
                    title: Some(form.title_for_edit()),
                    goal: form.goal_update(),
                },
                updated_on,
            )
        },
    )
}

/// Removes a phase. Always forces when tasks remain; undo restores them.
#[must_use]
pub fn save_remove_phase(
    path: &Path,
    fingerprint: &ContentFingerprint,
    phase_id: &str,
    title: &str,
    task_count: usize,
) -> SaveResult {
    let message = match task_count {
        0 => format!("Removed phase {title}"),
        1 => format!("Removed phase {title} and 1 task"),
        n => format!("Removed phase {title} and {n} tasks"),
    };
    apply(path, fingerprint, message, |project, updated_on| {
        remove_phase(project, phase_id, true, updated_on)
    })
}

/// Updates a task's fields.
#[must_use]
pub fn save_task_edit(
    path: &Path,
    fingerprint: &ContentFingerprint,
    phase_id: &str,
    task_id: &str,
    form: &TaskEditForm,
) -> SaveResult {
    if let Some(message) = form.validation_message() {
        return failed(message);
    }
    apply(
        path,
        fingerprint,
        format!("Updated task {}", form.title_for_edit()),
        |project, updated_on| {
            set_task(
                project,
                phase_id,
                task_id,
                TaskChanges {
                    title: Some(form.title_for_edit()),
                    due: form.due_update(),
                    note: form.note_update(),
                },
                updated_on,
            )
        },
    )
}

/// Removes a task.
#[must_use]
pub fn save_remove_task(
    path: &Path,
    fingerprint: &ContentFingerprint,
    phase_id: &str,
    task_id: &str,
    title: &str,
) -> SaveResult {
    apply(
        path,
        fingerprint,
        format!("Removed task {title}"),
        |project, updated_on| remove_task(project, phase_id, task_id, updated_on),
    )
}

/// Updates an issue's fields.
#[must_use]
pub fn save_issue_edit(
    path: &Path,
    fingerprint: &ContentFingerprint,
    issue_id: &str,
    form: &IssueEditForm,
) -> SaveResult {
    if let Some(message) = form.validation_message() {
        return failed(message);
    }
    apply(
        path,
        fingerprint,
        format!("Updated issue {}", form.title_for_edit()),
        |project, updated_on| {
            set_issue(
                project,
                issue_id,
                IssueChanges {
                    title: Some(form.title_for_edit()),
                    severity: Some(form.severity.as_str()),
                    url: form.url_update(),
                    note: form.note_update(),
                },
                updated_on,
            )
        },
    )
}

/// Removes an issue.
#[must_use]
pub fn save_remove_issue(
    path: &Path,
    fingerprint: &ContentFingerprint,
    issue_id: &str,
    title: &str,
) -> SaveResult {
    apply(
        path,
        fingerprint,
        format!("Removed issue {title}"),
        |project, updated_on| remove_issue(project, issue_id, updated_on),
    )
}

/// Creates a project file and adds it to the workspace.
#[must_use]
pub fn save_new_project(
    workspace_file: &Path,
    workspace_fingerprint: &ContentFingerprint,
    form: &NewProjectForm,
) -> SaveResult {
    if let Some(message) = form.validation_message(&[]) {
        return failed(message);
    }
    let directory = PathBuf::from(form.directory.trim());
    if !directory.is_dir() {
        return failed("folder does not exist");
    }
    let project_path = directory.join("craftward.json");
    if project_path.exists() {
        return failed("folder already contains craftward.json");
    }
    let Some(status) = parse_project_status(&form.status) else {
        return failed("invalid status");
    };
    let previous_workspace = match workspace_bytes_or_empty(workspace_file) {
        Ok(bytes) => bytes,
        Err(error) => return failed(error.to_string()),
    };
    if content_fingerprint(&previous_workspace) != *workspace_fingerprint {
        return SaveResult::Failed {
            message: "workspace file changed on disk; reload before saving again".to_owned(),
            conflict: true,
        };
    }
    if let Err(error) = init_project(&directory, form.id_for_edit(), form.name_for_edit(), status) {
        return failed(error.to_string());
    }
    match add_project_if_unchanged(
        workspace_file,
        &project_path,
        workspace_fingerprint.as_bytes(),
    ) {
        Ok((_id, written)) => {
            let Ok(project_bytes) = std::fs::read(&project_path) else {
                let _ = std::fs::remove_file(&project_path);
                return failed("created project file but could not read it back");
            };
            SaveResult::Saved {
                message: format!("Created project {}", form.name_for_edit()),
                snapshots: vec![
                    UndoSnapshot {
                        path: workspace_file.to_path_buf(),
                        previous_bytes: Some(previous_workspace),
                        expected: content_fingerprint(&written),
                    },
                    UndoSnapshot {
                        path: project_path,
                        previous_bytes: None,
                        expected: content_fingerprint(&project_bytes),
                    },
                ],
            }
        }
        Err(error) => {
            let _ = std::fs::remove_file(&project_path);
            SaveResult::Failed {
                message: error.to_string(),
                conflict: error.is_conflict(),
            }
        }
    }
}

/// Adds an existing project file or folder to the workspace.
#[must_use]
pub fn save_add_project(
    workspace_file: &Path,
    workspace_fingerprint: &ContentFingerprint,
    input: &str,
) -> SaveResult {
    let input = PathBuf::from(input.trim());
    if input.as_os_str().is_empty() {
        return failed("path is required");
    }
    let previous = match workspace_bytes_or_empty(workspace_file) {
        Ok(bytes) => bytes,
        Err(error) => return failed(error.to_string()),
    };
    match add_project_if_unchanged(workspace_file, &input, workspace_fingerprint.as_bytes()) {
        Ok((id, written)) => SaveResult::Saved {
            message: format!("Added project {id}"),
            snapshots: vec![UndoSnapshot {
                path: workspace_file.to_path_buf(),
                previous_bytes: Some(previous),
                expected: content_fingerprint(&written),
            }],
        },
        Err(error) => SaveResult::Failed {
            message: error.to_string(),
            conflict: error.is_conflict(),
        },
    }
}

/// Removes a project from the workspace list.
///
/// When `delete_file` is true, also deletes that project's `craftward.json`.
/// File deletion is irreversible; no undo snapshot is offered for that path.
#[must_use]
pub fn save_remove_from_workspace(
    workspace_file: &Path,
    workspace_fingerprint: &ContentFingerprint,
    project_id: &str,
    project_name: &str,
    delete_file: bool,
) -> SaveResult {
    let previous = match workspace_bytes_or_empty(workspace_file) {
        Ok(bytes) => bytes,
        Err(error) => return failed(error.to_string()),
    };
    match remove_project_if_unchanged(
        workspace_file,
        project_id,
        workspace_fingerprint.as_bytes(),
        delete_file,
    ) {
        Ok(written) => {
            let message = if delete_file {
                format!("Removed {project_name} and deleted craftward.json")
            } else {
                format!("Removed {project_name} from the list (craftward.json kept on disk)")
            };
            // Undo restores the workspace entry only. A deleted craftward.json
            // is not restored (the confirmation dialog names that irreversibility).
            let snapshots = if delete_file {
                Vec::new()
            } else {
                vec![UndoSnapshot {
                    path: workspace_file.to_path_buf(),
                    previous_bytes: Some(previous),
                    expected: content_fingerprint(&written),
                }]
            };
            SaveResult::Saved { message, snapshots }
        }
        Err(error) => SaveResult::Failed {
            message: error.to_string(),
            conflict: error.is_conflict(),
        },
    }
}

fn apply(
    path: &Path,
    fingerprint: &ContentFingerprint,
    message: String,
    change: impl FnOnce(&craftward_core::Project, &str) -> Result<EditOutcome, EditError>,
) -> SaveResult {
    let Some(today) = utc_date(SystemTime::now()) else {
        return failed("cannot read the system clock");
    };
    let previous_bytes = fingerprint.as_bytes().to_vec();
    match edit_file_if_unchanged(path, fingerprint, &today, change) {
        Ok(outcome) => SaveResult::Saved {
            message,
            snapshots: vec![UndoSnapshot {
                path: path.to_path_buf(),
                previous_bytes: Some(previous_bytes),
                expected: outcome.fingerprint(),
            }],
        },
        Err(error) => SaveResult::Failed {
            conflict: error.is_conflict(),
            message: error.to_string(),
        },
    }
}

fn failed(message: impl Into<String>) -> SaveResult {
    SaveResult::Failed {
        message: message.into(),
        conflict: false,
    }
}

fn status_label(status: &str) -> &'static str {
    match status {
        "open" => "Open",
        "in_progress" => "In progress",
        "done" => "Done",
        "dropped" => "Dropped",
        "resolved" => "Resolved",
        "wont_fix" => "Won't fix",
        _ => "updated",
    }
}

fn parse_project_status(text: &str) -> Option<ProjectStatus> {
    match text {
        "idea" => Some(ProjectStatus::Idea),
        "in_development" => Some(ProjectStatus::InDevelopment),
        "released" => Some(ProjectStatus::Released),
        "maintenance" => Some(ProjectStatus::Maintenance),
        "paused" => Some(ProjectStatus::Paused),
        "discontinued" => Some(ProjectStatus::Discontinued),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use craftward_core::{
        IssueChanges, LinkChange, LinkListUpdate, NewIssue, NewPhase, NewTask, PhaseChanges,
        ProjectChanges, ProjectUpdate, StringListUpdate, TaskChanges, add_issue, add_phase,
        add_task, content_fingerprint, edit_file, fingerprint_file, remove_phase, set_issue,
        set_phase, set_project, set_task, utc_date,
    };

    use super::{
        SaveResult, save_header, save_issue_edit, save_new_phase, save_phase_edit,
        save_remove_phase, save_task_edit,
    };
    use crate::view::{HeaderForm, IssueEditForm, PhaseForm, TaskEditForm};

    fn today() -> String {
        utc_date(SystemTime::now()).expect("clock")
    }

    fn scratch(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        let path = std::env::temp_dir().join(format!("craftward-app-parity-{name}-{nanos}"));
        fs::create_dir_all(&path).expect("scratch");
        path
    }

    fn minimal(directory: &Path) -> PathBuf {
        let path = directory.join("craftward.json");
        fs::write(
            &path,
            "{\n  \"schema_version\": 1,\n  \"id\": \"demo\",\n  \"name\": \"Demo\",\n  \"status\": \"idea\"\n}\n",
        )
        .expect("write");
        path
    }

    fn twin_paths(path: &Path) -> (PathBuf, PathBuf) {
        let app = path.with_extension("app.json");
        let cli = path.with_extension("cli.json");
        fs::copy(path, &app).expect("copy app");
        fs::copy(path, &cli).expect("copy cli");
        (app, cli)
    }

    fn assert_twins_equal(app: &Path, cli: &Path) {
        assert_eq!(
            fs::read_to_string(app).expect("app"),
            fs::read_to_string(cli).expect("cli")
        );
    }

    #[test]
    fn save_header_matches_cli_set_project_bytes() {
        let root = scratch("header");
        let path = minimal(&root);
        let (app, cli) = twin_paths(&path);
        let fingerprint = fingerprint_file(&app).unwrap();
        let form = HeaderForm {
            name: "Harbor".to_owned(),
            status: "in_development".to_owned(),
            version: "0.2.0".to_owned(),
            description: "Desk".to_owned(),
            source_dir: "src".to_owned(),
            tags: "a, b".to_owned(),
            links: "Docs|https://example.com".to_owned(),
        };
        assert!(matches!(
            save_header(&app, &fingerprint, &form),
            SaveResult::Saved { .. }
        ));
        let day = today();
        edit_file(&cli, &day, |project, updated| {
            let tags = vec!["a".to_owned(), "b".to_owned()];
            let links = [LinkChange {
                label: "Docs",
                url: "https://example.com",
            }];
            set_project(
                project,
                ProjectChanges {
                    name: Some("Harbor"),
                    status: Some("in_development"),
                    version: ProjectUpdate::Set("0.2.0"),
                    description: ProjectUpdate::Set("Desk"),
                    source_dir: ProjectUpdate::Set("src"),
                    tags: StringListUpdate::Set(&tags),
                    links: LinkListUpdate::Set(&links),
                },
                updated,
            )
        })
        .unwrap();
        assert_twins_equal(&app, &cli);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn save_phase_add_edit_remove_match_cli_bytes() {
        let root = scratch("phase");
        let path = minimal(&root);
        let (app, cli) = twin_paths(&path);
        let day = today();
        let fingerprint = fingerprint_file(&app).unwrap();
        let form = PhaseForm {
            id: "build".to_owned(),
            title: "Build".to_owned(),
            goal: "Ship".to_owned(),
        };
        assert!(matches!(
            save_new_phase(&app, &fingerprint, &form),
            SaveResult::Saved { .. }
        ));
        edit_file(&cli, &day, |project, updated| {
            add_phase(
                project,
                NewPhase {
                    title: "Build",
                    id: Some("build"),
                    goal: Some("Ship"),
                },
                updated,
            )
        })
        .unwrap();
        assert_twins_equal(&app, &cli);

        let fingerprint = fingerprint_file(&app).unwrap();
        let edit = PhaseForm {
            id: "build".to_owned(),
            title: "Build hard".to_owned(),
            goal: String::new(),
        };
        assert!(matches!(
            save_phase_edit(&app, &fingerprint, "build", &edit),
            SaveResult::Saved { .. }
        ));
        edit_file(&cli, &day, |project, updated| {
            set_phase(
                project,
                "build",
                PhaseChanges {
                    title: Some("Build hard"),
                    goal: ProjectUpdate::Clear,
                },
                updated,
            )
        })
        .unwrap();
        assert_twins_equal(&app, &cli);

        edit_file(&app, &day, |project, updated| {
            add_task(
                project,
                NewTask {
                    phase: "build",
                    title: "One",
                    id: Some("one"),
                    status: None,
                    due: None,
                    note: None,
                },
                updated,
            )
        })
        .unwrap();
        fs::copy(&app, &cli).unwrap();
        let fingerprint = fingerprint_file(&app).unwrap();
        match save_remove_phase(&app, &fingerprint, "build", "Build hard", 1) {
            SaveResult::Saved { message, .. } => {
                assert!(message.contains("1 task"));
            }
            SaveResult::Failed { message, .. } => panic!("{message}"),
        }
        edit_file(&cli, &day, |project, updated| {
            remove_phase(project, "build", true, updated)
        })
        .unwrap();
        assert_twins_equal(&app, &cli);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn save_task_and_issue_edit_match_cli_bytes() {
        let root = scratch("task-issue");
        let path = minimal(&root);
        let day = today();
        edit_file(&path, &day, |project, updated| {
            add_phase(
                project,
                NewPhase {
                    title: "Build",
                    id: Some("build"),
                    goal: None,
                },
                updated,
            )
        })
        .unwrap();
        edit_file(&path, &day, |project, updated| {
            add_task(
                project,
                NewTask {
                    phase: "build",
                    title: "Ship",
                    id: Some("ship"),
                    status: None,
                    due: None,
                    note: None,
                },
                updated,
            )
        })
        .unwrap();
        edit_file(&path, &day, |project, updated| {
            add_issue(
                project,
                NewIssue {
                    title: "Contrast",
                    severity: "high",
                    id: Some("contrast"),
                    url: None,
                    note: None,
                },
                updated,
            )
        })
        .unwrap();
        let (app, cli) = twin_paths(&path);

        let fingerprint = fingerprint_file(&app).unwrap();
        let task_form = TaskEditForm {
            title: "Ship it".to_owned(),
            due: "2026-12-01".to_owned(),
            note: "go".to_owned(),
        };
        assert!(matches!(
            save_task_edit(&app, &fingerprint, "build", "ship", &task_form),
            SaveResult::Saved { .. }
        ));
        edit_file(&cli, &day, |project, updated| {
            set_task(
                project,
                "build",
                "ship",
                TaskChanges {
                    title: Some("Ship it"),
                    due: ProjectUpdate::Set("2026-12-01"),
                    note: ProjectUpdate::Set("go"),
                },
                updated,
            )
        })
        .unwrap();
        assert_twins_equal(&app, &cli);

        let fingerprint = fingerprint_file(&app).unwrap();
        let issue_form = IssueEditForm {
            title: "Contrast bug".to_owned(),
            severity: "critical".to_owned(),
            url: "https://example.com/i".to_owned(),
            note: "see ticket".to_owned(),
        };
        assert!(matches!(
            save_issue_edit(&app, &fingerprint, "contrast", &issue_form),
            SaveResult::Saved { .. }
        ));
        edit_file(&cli, &day, |project, updated| {
            set_issue(
                project,
                "contrast",
                IssueChanges {
                    title: Some("Contrast bug"),
                    severity: Some("critical"),
                    url: ProjectUpdate::Set("https://example.com/i"),
                    note: ProjectUpdate::Set("see ticket"),
                },
                updated,
            )
        })
        .unwrap();
        assert_twins_equal(&app, &cli);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn invalid_forms_do_not_write() {
        let root = scratch("invalid");
        let path = minimal(&root);
        let before = fs::read(&path).unwrap();
        let fingerprint = content_fingerprint(&before);
        let form = HeaderForm {
            name: String::new(),
            status: "idea".to_owned(),
            version: String::new(),
            description: String::new(),
            source_dir: String::new(),
            tags: String::new(),
            links: String::new(),
        };
        assert!(matches!(
            save_header(&path, &fingerprint, &form),
            SaveResult::Failed {
                conflict: false,
                ..
            }
        ));
        assert_eq!(fs::read(&path).unwrap(), before);
        let _ = fs::remove_dir_all(root);
    }
}
