//! Editing commands, using a temporary directory instead of a real project.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn scratch(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let path = std::env::temp_dir().join(format!("craftward-edit-cli-{name}-{nanos}"));
    fs::create_dir_all(&path).expect("create scratch directory");
    path
}

fn run(directory: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_craftward"))
        .args(args)
        .current_dir(directory)
        .env("APPDATA", directory)
        .env("HOME", directory)
        .env("USERPROFILE", directory)
        .env("XDG_CONFIG_HOME", directory)
        .env_remove("CRAFTWARD_WORKSPACE")
        .output()
        .expect("failed to execute craftward")
}

fn project_file(directory: &Path) -> PathBuf {
    let path = directory.join("craftward.json");
    fs::write(
        &path,
        "{\n  \"schema_version\": 1,\n  \"id\": \"demo\",\n  \"name\": \"Demo\",\n  \"status\": \"idea\"\n}\n",
    )
    .expect("write project");
    path
}

fn stdout(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn succeed(directory: &Path, args: &[&str]) -> String {
    let output = run(directory, args);
    assert!(
        output.status.success(),
        "{}: {}\n{}",
        args.join(" "),
        stdout(&output),
        stderr(&output)
    );
    stdout(&output)
}

#[test]
fn show_and_project_set_edit_a_temporary_file() {
    let root = scratch("header");
    let path = project_file(&root);
    let file = path.to_str().unwrap();
    let shown = succeed(&root, &["show", file]);
    assert!(shown.contains("id: demo"), "{shown}");
    assert!(shown.contains("issues\n  none"), "{shown}");
    let header = succeed(
        &root,
        &[
            "project",
            "set",
            file,
            "--name",
            "Harbor Light",
            "--status",
            "in_development",
            "--version",
            "0.5.0",
            "--description",
            "Notes",
            "--source-dir",
            "src",
        ],
    );
    assert!(header.contains("updated project"));
    succeed(&root, &["project", "set", file, "--version", ""]);
    let stored = fs::read_to_string(&path).unwrap();
    assert!(!stored.contains("current_version"), "{stored}");
    assert!(stored.contains("\"updated\": \""));
    assert!(stored.contains("Harbor Light"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn phase_and_task_commands_edit_a_temporary_file() {
    let root = scratch("tasks");
    let path = project_file(&root);
    let file = path.to_str().unwrap();
    let phase = succeed(
        &root,
        &[
            "phase",
            "add",
            file,
            "--title",
            "Prüfung",
            "--goal",
            "Trust the file",
        ],
    );
    assert!(phase.contains("added phase pruefung"), "{phase}");
    let empty = succeed(&root, &["phase", "add", file, "--title", "Shelf"]);
    assert!(empty.contains("added phase shelf"));
    let task = succeed(
        &root,
        &[
            "task",
            "add",
            file,
            "pruefung",
            "--title",
            "Write schema",
            "--status",
            "in_progress",
            "--due",
            "2027-03-31",
            "--note",
            "Cover the rules",
        ],
    );
    assert!(task.contains("added task write-schema"));
    let status = succeed(
        &root,
        &["task", "status", file, "pruefung", "write-schema", "open"],
    );
    assert!(status.contains("set task write-schema to open"));
    let done = succeed(&root, &["task", "done", file, "pruefung", "write-schema"]);
    assert!(done.contains("set task write-schema to done"));
    succeed(
        &root,
        &[
            "task", "add", file, "pruefung", "--title", "Extra", "--id", "extra",
        ],
    );
    let removed = succeed(&root, &["task", "remove", file, "pruefung", "extra"]);
    assert!(removed.contains("removed task extra"));
    let removed_phase = succeed(&root, &["phase", "remove", file, "shelf"]);
    assert!(removed_phase.contains("removed phase shelf"));
    let view = succeed(&root, &["show", file]);
    assert!(
        view.contains("task write-schema [done] Write schema"),
        "{view}"
    );
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("Prüfung"));
    assert!(!text.contains("\\u"));
    assert!(text.ends_with('\n'));
    assert!(!text.contains('\r'));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn issue_and_watch_commands_edit_a_temporary_file() {
    let root = scratch("issues");
    let path = project_file(&root);
    let file = path.to_str().unwrap();
    let issue = succeed(
        &root,
        &[
            "issue",
            "add",
            file,
            "--title",
            "Contrast",
            "--severity",
            "medium",
            "--url",
            "https://example.com/issues",
            "--note",
            "Light theme",
        ],
    );
    assert!(issue.contains("added issue contrast"));
    succeed(&root, &["issue", "status", file, "contrast", "in_progress"]);
    succeed(
        &root,
        &[
            "issue",
            "add",
            file,
            "--title",
            "Other",
            "--severity",
            "low",
            "--id",
            "other",
        ],
    );
    let removed_issue = succeed(&root, &["issue", "remove", file, "other"]);
    assert!(removed_issue.contains("removed issue other"));
    let watch = succeed(
        &root,
        &[
            "watch",
            "add",
            file,
            "--kind",
            "expiry",
            "--title",
            "Renew domain",
            "--due",
            "2027-06-30",
            "--note",
            "Registrar",
        ],
    );
    assert!(watch.contains("added watch renew-domain"));
    succeed(
        &root,
        &[
            "watch",
            "add",
            file,
            "--kind",
            "review",
            "--title",
            "Quarterly",
            "--due",
            "2027-01-01",
            "--id",
            "quarterly",
        ],
    );
    let removed_watch = succeed(&root, &["watch", "remove", file, "quarterly"]);
    assert!(removed_watch.contains("removed watch quarterly"));
    let view = succeed(&root, &["show", file]);
    assert!(
        view.contains("issue contrast [in_progress] medium Contrast"),
        "{view}"
    );
    assert!(
        view.contains("watch renew-domain [expiry] Renew domain"),
        "{view}"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn rejected_header_edits_leave_the_file_and_exit() {
    let root = scratch("header-fail");
    let original = "{\n    \"schema_version\": 1,\n    \"id\": \"demo\",\n    \"name\": \"Demo\",\n    \"status\": \"idea\"\n}\n";
    let path = root.join("craftward.json");
    fs::write(&path, original).unwrap();
    let file = path.to_str().unwrap();
    let before = fs::read(&path).unwrap();
    let blank = run(&root, &["project", "set", file, "--name", "   "]);
    assert_eq!(blank.status.code(), Some(1), "{}", stderr(&blank));
    assert!(stderr(&blank).contains("invalid project file"));
    assert_eq!(fs::read(&path).unwrap(), before);
    let no_option = run(&root, &["project", "set", file]);
    assert_eq!(no_option.status.code(), Some(2));
    assert!(stderr(&no_option).contains("at least one option"));
    let bad_status = run(&root, &["project", "set", file, "--status", "nope"]);
    assert_eq!(bad_status.status.code(), Some(2), "{}", stderr(&bad_status));
    assert!(stderr(&bad_status).contains("idea, in_development, released"));
    assert_eq!(fs::read(&path).unwrap(), before);
    let symbols = run(&root, &["phase", "add", file, "--title", "!!!"]);
    assert_eq!(symbols.status.code(), Some(1));
    assert!(stderr(&symbols).contains("--id"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn rejected_task_and_phase_edits_exit_with_the_ids() {
    let root = scratch("task-fail");
    let path = project_file(&root);
    let file = path.to_str().unwrap();
    succeed(
        &root,
        &[
            "phase",
            "add",
            file,
            "--title",
            "Foundation",
            "--id",
            "foundation",
        ],
    );
    succeed(
        &root,
        &[
            "task",
            "add",
            file,
            "foundation",
            "--title",
            "Schema",
            "--id",
            "schema",
        ],
    );
    let unknown = run(&root, &["task", "done", file, "missing", "schema"]);
    assert_eq!(unknown.status.code(), Some(1));
    assert!(stderr(&unknown).contains("phases: foundation"));
    let unknown_task = run(&root, &["task", "done", file, "foundation", "missing"]);
    assert_eq!(unknown_task.status.code(), Some(1));
    assert!(stderr(&unknown_task).contains("tasks: schema"));
    let bad_status = run(
        &root,
        &["task", "status", file, "foundation", "schema", "nope"],
    );
    assert_eq!(bad_status.status.code(), Some(2));
    assert!(stderr(&bad_status).contains("open, in_progress, done, dropped"));
    let bad_date = run(
        &root,
        &[
            "task",
            "add",
            file,
            "foundation",
            "--title",
            "Later",
            "--due",
            "2026-13-01",
        ],
    );
    assert_eq!(bad_date.status.code(), Some(2));
    assert!(stderr(&bad_date).contains("YYYY-MM-DD"));
    let blocked = run(&root, &["phase", "remove", file, "foundation"]);
    assert_eq!(blocked.status.code(), Some(1));
    assert!(stderr(&blocked).contains("--force"));
    let forced = succeed(&root, &["phase", "remove", file, "foundation", "--force"]);
    assert!(forced.contains("removed phase foundation"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn rejected_issue_and_watch_edits_exit_with_the_allowed_values() {
    let root = scratch("issue-fail");
    let path = project_file(&root);
    let file = path.to_str().unwrap();
    let bad_severity = run(
        &root,
        &[
            "issue",
            "add",
            file,
            "--title",
            "Contrast",
            "--severity",
            "huge",
        ],
    );
    assert_eq!(bad_severity.status.code(), Some(2));
    assert!(stderr(&bad_severity).contains("low, medium, high, critical"));
    let bad_url = run(
        &root,
        &[
            "issue",
            "add",
            file,
            "--title",
            "Contrast",
            "--severity",
            "low",
            "--url",
            "ftp://example.com",
        ],
    );
    assert_eq!(bad_url.status.code(), Some(2));
    assert!(stderr(&bad_url).contains("http://, https://"));
    let unknown_issue = run(&root, &["issue", "remove", file, "missing"]);
    assert_eq!(unknown_issue.status.code(), Some(1));
    assert!(stderr(&unknown_issue).contains("issues: (none)"));
    let bad_kind = run(
        &root,
        &[
            "watch",
            "add",
            file,
            "--kind",
            "soon",
            "--title",
            "Renew",
            "--due",
            "2027-06-30",
        ],
    );
    assert_eq!(bad_kind.status.code(), Some(2));
    assert!(stderr(&bad_kind).contains("deadline, expiry, review, other"));
    let unknown_watch = run(&root, &["watch", "remove", file, "missing"]);
    assert_eq!(unknown_watch.status.code(), Some(1));
    assert!(stderr(&unknown_watch).contains("watch items: (none)"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn every_edit_command_has_help() {
    let root = scratch("help");
    let commands: &[&[&str]] = &[
        &["show", "--help"],
        &["project", "--help"],
        &["project", "set", "--help"],
        &["phase", "--help"],
        &["phase", "add", "--help"],
        &["phase", "remove", "--help"],
        &["task", "--help"],
        &["task", "add", "--help"],
        &["task", "status", "--help"],
        &["task", "done", "--help"],
        &["task", "remove", "--help"],
        &["issue", "--help"],
        &["issue", "add", "--help"],
        &["issue", "status", "--help"],
        &["issue", "remove", "--help"],
        &["watch", "--help"],
        &["watch", "add", "--help"],
        &["watch", "remove", "--help"],
    ];
    for command in commands {
        let output = run(&root, command);
        let text = stdout(&output);
        assert!(
            output.status.success(),
            "{command:?}: {text}{}",
            stderr(&output)
        );
        assert!(text.contains("Usage:"), "{command:?}: {text}");
    }
    let extra = run(&root, &["task", "add", "extra", "--help"]);
    assert_eq!(extra.status.code(), Some(2), "{}", stderr(&extra));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn an_ambiguous_project_argument_exits_2() {
    let root = scratch("ambiguous");
    let workspace = root.join("workspace.json");
    let app = root.join("app");
    let workspace_arg = workspace.to_str().unwrap();
    let app_arg = app.to_str().unwrap();
    succeed(
        &root,
        &[
            "--workspace",
            workspace_arg,
            "init",
            app_arg,
            "--id",
            "sample",
            "--name",
            "Sample",
        ],
    );
    succeed(&root, &["--workspace", workspace_arg, "add", app_arg]);
    fs::create_dir_all(root.join("sample")).unwrap();
    let shown = run(&root, &["--workspace", workspace_arg, "show", "sample"]);
    assert_eq!(shown.status.code(), Some(2), "{}", stderr(&shown));
    assert!(stderr(&shown).contains("both a workspace project id"));
    let _ = fs::remove_dir_all(root);
}
