//! Workspace commands, using a temporary directory instead of the real config folder.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn scratch(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let path = std::env::temp_dir().join(format!("craftward-cli-{name}-{nanos}"));
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

fn workspace_arg(directory: &Path) -> String {
    directory
        .join("workspace.json")
        .to_string_lossy()
        .into_owned()
}

#[test]
fn commands_round_trip_in_a_temporary_workspace() {
    let root = scratch("round");
    let workspace = workspace_arg(&root);
    let app = root.join("app");
    let init = run(
        &root,
        &[
            "--workspace",
            &workspace,
            "init",
            app.to_str().unwrap(),
            "--id",
            "harbor-light",
            "--name",
            "Harbor Light",
        ],
    );
    assert!(
        init.status.success(),
        "init failed: {}",
        String::from_utf8_lossy(&init.stderr)
    );
    assert!(!root.join("workspace.json").exists());
    let again = run(
        &root,
        &[
            "init",
            app.to_str().unwrap(),
            "--id",
            "other",
            "--name",
            "Other",
        ],
    );
    assert_eq!(again.status.code(), Some(1));

    let add = run(
        &root,
        &["--workspace", &workspace, "add", app.to_str().unwrap()],
    );
    assert!(
        add.status.success(),
        "add failed: {}",
        String::from_utf8_lossy(&add.stderr)
    );
    let list = run(&root, &["--workspace", &workspace, "list"]);
    let list_out = String::from_utf8(list.stdout).unwrap();
    assert!(list.status.success(), "{list_out}");
    assert!(list_out.contains("harbor-light"));
    assert!(list_out.contains("Harbor Light"));
    assert!(list_out.contains("idea"));

    let validate = run(&root, &["--workspace", &workspace, "validate"]);
    assert!(validate.status.success());
    assert!(String::from_utf8(validate.stdout).unwrap().contains(": ok"));

    let remove = run(
        &root,
        &["--workspace", &workspace, "remove", "harbor-light"],
    );
    assert!(remove.status.success());
    let after = run(&root, &["--workspace", &workspace, "list"]);
    assert!(after.status.success());
    assert!(
        String::from_utf8(after.stdout)
            .unwrap()
            .contains("no projects")
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn printed_paths_use_the_platform_separator() {
    let root = scratch("seps");
    let workspace = workspace_arg(&root);
    let app = root.join("app");
    let init = run(
        &root,
        &[
            "--workspace",
            &workspace,
            "init",
            app.to_str().unwrap(),
            "--id",
            "harbor-light",
            "--name",
            "Harbor Light",
        ],
    );
    assert!(
        init.status.success(),
        "init failed: {}",
        String::from_utf8_lossy(&init.stderr)
    );
    let add = run(
        &root,
        &["--workspace", &workspace, "add", app.to_str().unwrap()],
    );
    assert!(
        add.status.success(),
        "add failed: {}",
        String::from_utf8_lossy(&add.stderr)
    );
    let validate = run(&root, &["--workspace", &workspace, "validate"]);
    let stdout = String::from_utf8(validate.stdout).unwrap();
    assert!(validate.status.success(), "{stdout}");
    let printed = stdout
        .lines()
        .find_map(|line| line.strip_suffix(": ok"))
        .expect("validation line");
    let expected = app.join("craftward.json");
    assert_eq!(printed, expected.to_string_lossy());
    let foreign = if std::path::MAIN_SEPARATOR == '\\' {
        '/'
    } else {
        '\\'
    };
    assert!(
        !printed.contains(foreign),
        "printed path mixes separators: {printed}"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn add_rejects_duplicates_and_invalid_files() {
    let root = scratch("reject");
    let workspace = workspace_arg(&root);
    let first = root.join("one");
    let second = root.join("two");
    fs::create_dir_all(&first).unwrap();
    fs::create_dir_all(&second).unwrap();
    let body = "{\n  \"schema_version\": 1,\n  \"id\": \"harbor-light\",\n  \"name\": \"Harbor\",\n  \"status\": \"idea\"\n}\n";
    fs::write(first.join("craftward.json"), body).unwrap();
    fs::write(second.join("craftward.json"), body).unwrap();
    assert!(
        run(
            &root,
            &["--workspace", &workspace, "add", first.to_str().unwrap()]
        )
        .status
        .success()
    );
    let duplicate_path = run(
        &root,
        &[
            "--workspace",
            &workspace,
            "add",
            first.join("craftward.json").to_str().unwrap(),
        ],
    );
    assert_eq!(duplicate_path.status.code(), Some(1));
    let duplicate_id = run(
        &root,
        &["--workspace", &workspace, "add", second.to_str().unwrap()],
    );
    assert_eq!(duplicate_id.status.code(), Some(1));
    assert!(
        String::from_utf8(duplicate_id.stderr)
            .unwrap()
            .contains("already listed")
    );
    let broken = root.join("broken.json");
    fs::write(&broken, "{").unwrap();
    let invalid = run(
        &root,
        &["--workspace", &workspace, "add", broken.to_str().unwrap()],
    );
    assert_eq!(invalid.status.code(), Some(1));
    let unknown = run(&root, &["--workspace", &workspace, "remove", "missing"]);
    assert_eq!(unknown.status.code(), Some(1));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn list_exits_1_when_a_project_file_is_missing_or_invalid() {
    let root = scratch("list-bad");
    let workspace = root.join("workspace.json");
    fs::write(
        &workspace,
        "{\n  \"schema_version\": 1,\n  \"projects\": [\n    { \"path\": \"gone/craftward.json\" }\n  ]\n}\n",
    )
    .unwrap();
    let output = run(&root, &["--workspace", workspace.to_str().unwrap(), "list"]);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(output.status.code(), Some(1), "{stdout}");
    assert!(stdout.contains("gone"));
    fs::create_dir_all(root.join("bad")).unwrap();
    fs::write(root.join("bad").join("craftward.json"), "{").unwrap();
    fs::write(
        &workspace,
        "{\n  \"schema_version\": 1,\n  \"projects\": [\n    { \"path\": \"bad/craftward.json\" }\n  ]\n}\n",
    )
    .unwrap();
    let invalid = run(&root, &["--workspace", workspace.to_str().unwrap(), "list"]);
    assert_eq!(invalid.status.code(), Some(1));
    assert!(
        String::from_utf8(invalid.stdout)
            .unwrap()
            .contains("syntax error")
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn workspace_path_reports_the_selected_source() {
    let root = scratch("path");
    let workspace = workspace_arg(&root);
    let chosen = run(&root, &["--workspace", &workspace, "workspace", "path"]);
    let stdout = String::from_utf8(chosen.stdout).unwrap();
    assert!(chosen.status.success(), "{stdout}");
    assert!(stdout.contains("command-line option"));
    assert!(stdout.contains("workspace.json"));

    let default_location = run(&root, &["workspace", "path"]);
    let stdout = String::from_utf8(default_location.stdout).unwrap();
    assert!(default_location.status.success(), "{stdout}");
    assert!(stdout.contains("default location"));
    assert!(stdout.contains(root.to_string_lossy().as_ref()));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn validate_warns_about_a_missing_source_directory_and_exits_0() {
    let root = scratch("warn");
    let project = root.join("craftward.json");
    fs::write(
        &project,
        "{\n  \"schema_version\": 1,\n  \"id\": \"harbor-light\",\n  \"name\": \"Harbor\",\n  \"status\": \"idea\",\n  \"source_dir\": \"missing-src\",\n  \"stack\": { \"components\": [], \"lockfiles\": [\"nope.lock\"] }\n}\n",
    )
    .unwrap();
    let output = run(&root, &["validate", project.to_str().unwrap()]);
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(output.status.success(), "{stdout}{stderr}");
    assert!(stdout.contains(": ok"), "{stdout}");
    assert!(stderr.contains("warning:"), "{stderr}");
    assert!(stderr.contains("missing-src"), "{stderr}");
    assert!(stderr.contains("nope.lock"), "{stderr}");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn roadmap_writes_the_next_file_in_a_temporary_directory() {
    let root = scratch("roadmap");
    let project = root.join("craftward.json");
    fs::write(
        &project,
        "{\n  \"schema_version\": 1,\n  \"id\": \"harbor-light\",\n  \"name\": \"Harbor\",\n  \"status\": \"idea\"\n}\n",
    )
    .unwrap();
    let directory = root.join("pages");
    let first = run(
        &root,
        &[
            "roadmap",
            project.to_str().unwrap(),
            "--dir",
            directory.to_str().unwrap(),
            "--lang",
            "de",
        ],
    );
    let stdout = String::from_utf8(first.stdout).unwrap();
    assert!(
        first.status.success(),
        "{stdout}{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(stdout.contains("harbor-light_roadmap_v1.html"), "{stdout}");
    let saved = fs::read_to_string(directory.join("harbor-light_roadmap_v1.html")).unwrap();
    assert!(saved.contains("Gesamtfortschritt"), "{saved}");
    let second = run(
        &root,
        &[
            "roadmap",
            project.to_str().unwrap(),
            "--dir",
            directory.to_str().unwrap(),
        ],
    );
    assert!(second.status.success());
    assert!(
        String::from_utf8(second.stdout)
            .unwrap()
            .contains("harbor-light_roadmap_v2.html")
    );
    assert_eq!(
        fs::read_to_string(directory.join("harbor-light_roadmap_v1.html")).unwrap(),
        saved
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn roadmap_rejects_an_id_that_is_also_an_existing_path() {
    let root = scratch("ambiguous");
    let workspace = workspace_arg(&root);
    let registered = root.join("registered");
    let init = run(
        &root,
        &[
            "--workspace",
            &workspace,
            "init",
            registered.to_str().unwrap(),
            "--id",
            "trulog",
            "--name",
            "Registered",
        ],
    );
    assert!(
        init.status.success(),
        "{}",
        String::from_utf8_lossy(&init.stderr)
    );
    let add = run(
        &root,
        &[
            "--workspace",
            &workspace,
            "add",
            registered.to_str().unwrap(),
        ],
    );
    assert!(
        add.status.success(),
        "{}",
        String::from_utf8_lossy(&add.stderr)
    );

    let folder = root.join("trulog");
    fs::create_dir_all(&folder).unwrap();
    fs::write(
        folder.join("craftward.json"),
        "{\n  \"schema_version\": 1,\n  \"id\": \"local-copy\",\n  \"name\": \"Local\",\n  \"status\": \"idea\"\n}\n",
    )
    .unwrap();
    let pages = root.join("pages");
    let ambiguous = run(
        &root,
        &[
            "--workspace",
            &workspace,
            "roadmap",
            "trulog",
            "--dir",
            pages.to_str().unwrap(),
        ],
    );
    let stderr = String::from_utf8(ambiguous.stderr).unwrap();
    assert_eq!(ambiguous.status.code(), Some(2), "{stderr}");
    assert!(stderr.starts_with("error:"), "{stderr}");
    let hint = format!(".{sep}trulog", sep = std::path::MAIN_SEPARATOR);
    assert!(stderr.contains(&hint), "{stderr}");
    assert!(stderr.contains("project file"), "{stderr}");
    assert!(!pages.join("trulog_roadmap_v1.html").exists());
    assert!(!pages.join("local-copy_roadmap_v1.html").exists());

    let explicit = run(
        &root,
        &[
            "--workspace",
            &workspace,
            "roadmap",
            &hint,
            "--dir",
            pages.to_str().unwrap(),
        ],
    );
    let stdout = String::from_utf8(explicit.stdout).unwrap();
    assert!(
        explicit.status.success(),
        "{stdout}{}",
        String::from_utf8_lossy(&explicit.stderr)
    );
    assert!(stdout.contains("local-copy_roadmap_v1.html"), "{stdout}");
    assert!(!pages.join("trulog_roadmap_v1.html").exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn each_command_has_help() {
    let root = scratch("help");
    for command in [
        "init",
        "add",
        "remove",
        "list",
        "validate",
        "workspace",
        "roadmap",
        "show",
        "project",
        "phase",
        "task",
        "issue",
        "watch",
        "radar",
    ] {
        let output = run(&root, &[command, "--help"]);
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(output.status.success(), "{command}: {stdout}");
        assert!(stdout.contains("Usage:"));
    }
    let path_help = run(&root, &["workspace", "path", "--help"]);
    assert!(path_help.status.success());
    let _ = fs::remove_dir_all(root);
}
