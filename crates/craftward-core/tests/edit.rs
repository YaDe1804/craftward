//! Project edits: valid files are replaced, invalid edits are not.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use craftward_core::{
    EditError, NewIssue, NewPhase, NewTask, NewWatch, ProjectChanges, ProjectUpdate, add_issue,
    add_phase, add_task, add_watch, edit_file, load_from_path, remove_issue, remove_phase,
    remove_task, remove_watch, render_project, set_issue_status, set_project, set_task_status,
};

const DAY: &str = "2026-11-15";

fn scratch(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let path = std::env::temp_dir().join(format!("craftward-edit-{name}-{nanos}"));
    fs::create_dir_all(&path).expect("create scratch directory");
    path
}

fn write_project(directory: &Path, body: &str) -> PathBuf {
    let path = directory.join("craftward.json");
    fs::write(&path, body).expect("write project");
    path
}

fn minimal(directory: &Path) -> PathBuf {
    write_project(
        directory,
        "{\n  \"schema_version\": 1,\n  \"id\": \"demo\",\n  \"name\": \"Demo\",\n  \"status\": \"idea\"\n}\n",
    )
}

fn changes<'a>() -> ProjectChanges<'a> {
    ProjectChanges {
        name: None,
        status: None,
        version: ProjectUpdate::Keep,
        description: ProjectUpdate::Keep,
        source_dir: None,
    }
}

fn phase(path: &Path, title: &str, id: Option<&str>, goal: Option<&str>) -> String {
    edit_file(path, DAY, |project, updated| {
        add_phase(project, NewPhase { title, id, goal }, updated)
    })
    .expect("add phase")
    .created_id
    .expect("phase id")
}

fn task(
    path: &Path,
    phase_id: &str,
    title: &str,
    id: Option<&str>,
    status: Option<&str>,
    due: Option<&str>,
    note: Option<&str>,
) -> String {
    edit_file(path, DAY, |project, updated| {
        add_task(
            project,
            NewTask {
                phase: phase_id,
                title,
                id,
                status,
                due,
                note,
            },
            updated,
        )
    })
    .expect("add task")
    .created_id
    .expect("task id")
}

fn issue(
    path: &Path,
    title: &str,
    severity: &str,
    id: Option<&str>,
    url: Option<&str>,
    note: Option<&str>,
) -> String {
    edit_file(path, DAY, |project, updated| {
        add_issue(
            project,
            NewIssue {
                title,
                severity,
                id,
                url,
                note,
            },
            updated,
        )
    })
    .expect("add issue")
    .created_id
    .expect("issue id")
}

fn watch(
    path: &Path,
    kind: &str,
    title: &str,
    due: &str,
    id: Option<&str>,
    note: Option<&str>,
) -> String {
    edit_file(path, DAY, |project, updated| {
        add_watch(
            project,
            NewWatch {
                kind,
                title,
                due,
                id,
                note,
            },
            updated,
        )
    })
    .expect("add watch")
    .created_id
    .expect("watch id")
}

#[test]
fn header_fields_can_be_set_and_cleared() {
    let root = scratch("header");
    let path = minimal(&root);
    edit_file(&path, DAY, |project, updated| {
        let mut fields = changes();
        fields.name = Some("Harbor Light");
        fields.status = Some("in_development");
        fields.version = ProjectUpdate::Set("0.5.0");
        fields.description = ProjectUpdate::Set("Notes");
        fields.source_dir = Some("src");
        set_project(project, fields, updated)
    })
    .expect("set header");
    edit_file(&path, DAY, |project, updated| {
        let mut fields = changes();
        fields.version = ProjectUpdate::Clear;
        fields.description = ProjectUpdate::Clear;
        set_project(project, fields, updated)
    })
    .expect("clear fields");
    let project = load_from_path(&path).expect("reload");
    assert_eq!(project.name, "Harbor Light");
    assert_eq!(project.status.as_str(), "in_development");
    assert_eq!(project.current_version, None);
    assert_eq!(project.description, None);
    assert_eq!(project.source_dir.as_deref(), Some("src"));
    assert_eq!(project.updated.as_deref(), Some(DAY));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn phases_and_tasks_can_be_added_updated_and_removed() {
    let root = scratch("tasks");
    let path = minimal(&root);
    assert_eq!(
        phase(
            &path,
            "Foundation",
            Some("foundation"),
            Some("Trust the file")
        ),
        "foundation"
    );
    assert_eq!(phase(&path, "Shelf", None, None), "shelf");
    assert_eq!(
        task(
            &path,
            "foundation",
            "Write schema",
            Some("schema"),
            Some("in_progress"),
            Some("2027-03-31"),
            Some("Cover the rules"),
        ),
        "schema"
    );
    edit_file(&path, DAY, |project, updated| {
        set_task_status(project, "foundation", "schema", "done", updated)
    })
    .expect("set task status");
    assert_eq!(
        task(
            &path,
            "foundation",
            "Extra",
            Some("extra"),
            None,
            None,
            None
        ),
        "extra"
    );
    edit_file(&path, DAY, |project, updated| {
        remove_task(project, "foundation", "extra", updated)
    })
    .expect("remove task");
    edit_file(&path, DAY, |project, updated| {
        remove_phase(project, "shelf", false, updated)
    })
    .expect("remove empty phase");
    let project = load_from_path(&path).expect("reload");
    let text = render_project(&project);
    assert!(text.contains("phase foundation"), "{text}");
    assert!(text.contains("task schema [done] Write schema"), "{text}");
    assert_eq!(project.updated.as_deref(), Some(DAY));
    let stored = fs::read_to_string(&path).expect("read");
    assert!(stored.contains("\n  \"schema_version\": 1"));
    assert!(stored.ends_with('\n'));
    assert!(!stored.contains('\r'));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn issues_and_watches_can_be_added_updated_and_removed() {
    let root = scratch("issues");
    let path = minimal(&root);
    assert_eq!(
        issue(
            &path,
            "Contrast",
            "medium",
            Some("contrast"),
            Some("https://example.com/issues"),
            Some("Light theme"),
        ),
        "contrast"
    );
    edit_file(&path, DAY, |project, updated| {
        set_issue_status(project, "contrast", "resolved", updated)
    })
    .expect("set issue status");
    assert_eq!(
        issue(&path, "Other", "low", Some("other"), None, None),
        "other"
    );
    edit_file(&path, DAY, |project, updated| {
        remove_issue(project, "other", updated)
    })
    .expect("remove issue");
    assert_eq!(
        watch(
            &path,
            "expiry",
            "Renew domain",
            "2027-06-30",
            Some("domain"),
            Some("Registrar"),
        ),
        "domain"
    );
    assert_eq!(
        watch(
            &path,
            "review",
            "Quarterly",
            "2027-01-01",
            Some("quarterly"),
            None
        ),
        "quarterly"
    );
    edit_file(&path, DAY, |project, updated| {
        remove_watch(project, "quarterly", updated)
    })
    .expect("remove watch");
    let project = load_from_path(&path).expect("reload");
    let text = render_project(&project);
    assert!(text.contains("issues\n  none"), "{text}");
    assert!(
        text.contains("watch domain [expiry] Renew domain"),
        "{text}"
    );
    assert_eq!(project.updated.as_deref(), Some(DAY));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn an_invalid_edit_leaves_the_bytes_unchanged() {
    let root = scratch("bytes");
    let original = "{\n    \"schema_version\": 1,\n    \"id\": \"demo\",\n    \"name\": \"Demo\",\n    \"status\": \"idea\"\n}\n";
    let path = write_project(&root, original);
    let before = fs::read(&path).expect("read before");
    let error = edit_file(&path, DAY, |project, updated| {
        let mut fields = changes();
        fields.name = Some("   ");
        set_project(project, fields, updated)
    })
    .expect_err("blank name");
    assert!(matches!(error, EditError::InvalidFile(_)), "{error}");
    assert!(
        error
            .to_string()
            .contains("must not be empty or whitespace")
    );
    assert_eq!(fs::read(&path).expect("read after"), before);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn ids_collide_with_a_numbered_suffix() {
    let root = scratch("ids");
    let path = minimal(&root);
    assert_eq!(phase(&path, "Prüfung", None, None), "pruefung");
    assert_eq!(phase(&path, "Prüfung", None, None), "pruefung-2");
    assert_eq!(phase(&path, "Prüfung", None, None), "pruefung-3");
    let long = "a".repeat(80);
    let wide = phase(&path, &long, None, None);
    assert_eq!(wide.len(), 64);
    let numbered = phase(&path, &long, None, None);
    assert!(numbered.ends_with("-2"), "{numbered}");
    assert_eq!(numbered.len(), 64);
    let stored = fs::read_to_string(&path).expect("read");
    assert!(stored.contains("Prüfung"));
    assert!(!stored.contains("\\u"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_title_with_no_usable_characters_asks_for_an_id() {
    let root = scratch("symbols");
    let path = minimal(&root);
    let before = fs::read(&path).expect("before");
    let symbols = edit_file(&path, DAY, |project, updated| {
        add_issue(
            project,
            NewIssue {
                title: "!!!",
                severity: "low",
                id: None,
                url: None,
                note: None,
            },
            updated,
        )
    })
    .expect_err("symbols");
    assert!(matches!(symbols, EditError::NeedsId), "{symbols}");
    assert!(symbols.to_string().contains("--id"));
    assert_eq!(fs::read(&path).expect("after"), before);
    assert_eq!(issue(&path, "Straße", "high", None, None, None), "strasse");
    let stored = fs::read_to_string(&path).expect("read");
    assert!(stored.contains("Straße"));
    assert!(!stored.contains("\\u"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn phase_remove_keeps_tasks_unless_forced() {
    let root = scratch("force");
    let path = minimal(&root);
    phase(&path, "Foundation", Some("foundation"), None);
    task(
        &path,
        "foundation",
        "Schema",
        Some("schema"),
        None,
        None,
        None,
    );
    let before = fs::read(&path).expect("before");
    let blocked = edit_file(&path, DAY, |project, updated| {
        remove_phase(project, "foundation", false, updated)
    })
    .expect_err("blocked");
    assert!(
        matches!(blocked, EditError::PhaseNotEmpty { .. }),
        "{blocked}"
    );
    assert!(blocked.to_string().contains("--force"));
    assert_eq!(fs::read(&path).expect("after block"), before);
    edit_file(&path, DAY, |project, updated| {
        remove_phase(project, "foundation", true, updated)
    })
    .expect("forced");
    let project = load_from_path(&path).expect("reload");
    assert_eq!(project.plan.expect("plan").phases.len(), 0);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn unknown_ids_list_what_exists() {
    let root = scratch("missing");
    let path = minimal(&root);
    phase(&path, "Foundation", Some("foundation"), None);
    task(
        &path,
        "foundation",
        "Schema",
        Some("schema"),
        None,
        None,
        None,
    );
    let missing_phase = edit_file(&path, DAY, |project, updated| {
        add_task(
            project,
            NewTask {
                phase: "missing",
                title: "Schema",
                id: None,
                status: None,
                due: None,
                note: None,
            },
            updated,
        )
    })
    .expect_err("phase");
    let phase_text = missing_phase.to_string();
    assert!(
        phase_text.contains("unknown phase: missing"),
        "{phase_text}"
    );
    assert!(phase_text.contains("phases: foundation"), "{phase_text}");
    let missing_task = edit_file(&path, DAY, |project, updated| {
        set_task_status(project, "foundation", "missing", "done", updated)
    })
    .expect_err("task");
    let task_text = missing_task.to_string();
    assert!(task_text.contains("unknown task: missing"), "{task_text}");
    assert!(task_text.contains("tasks: schema"), "{task_text}");
    let missing_issue = edit_file(&path, DAY, |project, updated| {
        set_issue_status(project, "missing", "open", updated)
    })
    .expect_err("issue");
    assert!(missing_issue.to_string().contains("issues: (none)"));
    let missing_watch = edit_file(&path, DAY, |project, updated| {
        remove_watch(project, "missing", updated)
    })
    .expect_err("watch");
    assert!(missing_watch.to_string().contains("watch items: (none)"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn invalid_values_name_the_allowed_set() {
    let root = scratch("values");
    let path = minimal(&root);
    phase(&path, "Foundation", Some("foundation"), None);
    let bad_status = edit_file(&path, DAY, |project, updated| {
        set_task_status(project, "foundation", "schema", "nope", updated)
    })
    .expect_err("status");
    assert!(bad_status.is_usage());
    assert!(
        bad_status
            .to_string()
            .contains("open, in_progress, done, dropped")
    );
    let bad_date = edit_file(&path, DAY, |project, updated| {
        add_task(
            project,
            NewTask {
                phase: "foundation",
                title: "Later",
                id: None,
                status: None,
                due: Some("2026-13-01"),
                note: None,
            },
            updated,
        )
    })
    .expect_err("date");
    assert!(bad_date.is_usage());
    assert!(bad_date.to_string().contains("YYYY-MM-DD"));
    let bad_severity = edit_file(&path, DAY, |project, updated| {
        add_issue(
            project,
            NewIssue {
                title: "Contrast",
                severity: "huge",
                id: None,
                url: None,
                note: None,
            },
            updated,
        )
    })
    .expect_err("severity");
    assert!(
        bad_severity
            .to_string()
            .contains("low, medium, high, critical")
    );
    let bad_url = edit_file(&path, DAY, |project, updated| {
        add_issue(
            project,
            NewIssue {
                title: "Contrast",
                severity: "low",
                id: None,
                url: Some("ftp://example.com"),
                note: None,
            },
            updated,
        )
    })
    .expect_err("url");
    assert!(bad_url.to_string().contains("http://, https://"));
    let bad_kind = edit_file(&path, DAY, |project, updated| {
        add_watch(
            project,
            NewWatch {
                kind: "soon",
                title: "Renew",
                due: "2027-06-30",
                id: None,
                note: None,
            },
            updated,
        )
    })
    .expect_err("kind");
    assert!(
        bad_kind
            .to_string()
            .contains("deadline, expiry, review, other")
    );
    let _ = fs::remove_dir_all(root);
}
