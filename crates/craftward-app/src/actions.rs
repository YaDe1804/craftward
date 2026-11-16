//! Desktop edits that call the same craftward-core writers as the CLI.

use std::path::Path;
use std::time::SystemTime;

use craftward_core::{
    ContentFingerprint, EditError, EditOutcome, NewIssue, NewTask, add_issue, add_task,
    edit_file_if_unchanged, set_issue_status, set_task_status, utc_date,
};

use crate::view::{IssueForm, TaskForm};

/// Result of a desktop save attempt.
#[derive(Debug)]
pub enum SaveResult {
    /// File written successfully.
    Saved,
    /// Validation or I/O failed; the draft should stay open.
    Failed {
        /// Message shown next to the control that triggered the save.
        message: String,
        /// True when the on-disk file drifted; the UI should offer Reload.
        conflict: bool,
    },
}

/// Sets a task status through the shared core writer.
#[must_use]
pub fn save_task_status(
    path: &Path,
    fingerprint: &ContentFingerprint,
    phase_id: &str,
    task_id: &str,
    status: &str,
) -> SaveResult {
    apply(path, fingerprint, |project, updated_on| {
        set_task_status(project, phase_id, task_id, status, updated_on)
    })
}

/// Sets an issue status through the shared core writer.
#[must_use]
pub fn save_issue_status(
    path: &Path,
    fingerprint: &ContentFingerprint,
    issue_id: &str,
    status: &str,
) -> SaveResult {
    apply(path, fingerprint, |project, updated_on| {
        set_issue_status(project, issue_id, status, updated_on)
    })
}

/// Adds a task from a validated form through the shared core writer.
#[must_use]
pub fn save_new_task(path: &Path, fingerprint: &ContentFingerprint, form: &TaskForm) -> SaveResult {
    if let Some(message) = form.validation_message() {
        return SaveResult::Failed {
            message: message.to_owned(),
            conflict: false,
        };
    }
    apply(path, fingerprint, |project, updated_on| {
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
    })
}

/// Adds an issue from a validated form through the shared core writer.
#[must_use]
pub fn save_new_issue(
    path: &Path,
    fingerprint: &ContentFingerprint,
    form: &IssueForm,
) -> SaveResult {
    if let Some(message) = form.validation_message() {
        return SaveResult::Failed {
            message: message.to_owned(),
            conflict: false,
        };
    }
    apply(path, fingerprint, |project, updated_on| {
        add_issue(
            project,
            NewIssue {
                title: form.title_for_edit(),
                severity: &form.severity,
                id: None,
                url: None,
                note: form.note_for_edit(),
            },
            updated_on,
        )
    })
}

fn apply(
    path: &Path,
    fingerprint: &ContentFingerprint,
    change: impl FnOnce(&craftward_core::Project, &str) -> Result<EditOutcome, EditError>,
) -> SaveResult {
    let Some(today) = utc_date(SystemTime::now()) else {
        return SaveResult::Failed {
            message: "cannot read the system clock".to_owned(),
            conflict: false,
        };
    };
    match edit_file_if_unchanged(path, fingerprint, &today, change) {
        Ok(_) => SaveResult::Saved,
        Err(error) => SaveResult::Failed {
            conflict: error.is_conflict(),
            message: error.to_string(),
        },
    }
}
