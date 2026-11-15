//! Exit status and output of `craftward validate`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn craftward(args: &[&str]) -> std::process::Output {
    let scratch = std::env::temp_dir().join(format!("craftward-cli-env-{}", std::process::id()));
    fs::create_dir_all(&scratch).expect("create scratch directory");
    Command::new(env!("CARGO_BIN_EXE_craftward"))
        .args(args)
        .env("APPDATA", &scratch)
        .env("HOME", &scratch)
        .env("USERPROFILE", &scratch)
        .env("XDG_CONFIG_HOME", &scratch)
        .env_remove("CRAFTWARD_WORKSPACE")
        .output()
        .expect("failed to execute craftward")
}

fn example(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples")
        .join(name)
}

/// The path text the CLI prints: components rebuilt with this system's separator.
fn printed(path: &Path) -> String {
    path.components()
        .collect::<PathBuf>()
        .to_string_lossy()
        .into_owned()
}

fn write_temp(name: &str, contents: &str) -> String {
    let path = std::env::temp_dir().join(name);
    fs::write(&path, contents).expect("write temp file");
    path.to_string_lossy().into_owned()
}

#[test]
fn validate_accepts_the_example_files() {
    let minimal = example("minimal.craftward.json");
    let full = example("full.craftward.json");
    let output = craftward(&[
        "validate",
        &minimal.to_string_lossy(),
        &full.to_string_lossy(),
    ]);
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "expected exit 0, got {:?}\n{stdout}{stderr}",
        output.status.code()
    );
    assert!(stderr.is_empty(), "unexpected stderr: {stderr}");
    assert!(stdout.contains(&format!("{}: ok", printed(&minimal))));
    assert!(stdout.contains(&format!("{}: ok", printed(&full))));
}

#[test]
fn validate_reports_an_invalid_file_and_exits_1() {
    let path = write_temp(
        "craftward-invalid.json",
        r#"{"schema_version":1,"id":"harbor-light","name":"","status":"idea"}"#,
    );
    let output = craftward(&["validate", &path]);
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert_eq!(output.status.code(), Some(1));
    assert!(stdout.contains("name: must not be empty or whitespace"));
    let _ = fs::remove_file(path);
}

#[test]
fn validate_exits_1_when_any_file_fails() {
    let minimal = example("minimal.craftward.json");
    let missing = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("missing-craftward.json");
    let output = craftward(&[
        "validate",
        &minimal.to_string_lossy(),
        &missing.to_string_lossy(),
    ]);
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert_eq!(output.status.code(), Some(1));
    assert!(stdout.contains(&format!("{}: ok", printed(&minimal))));
    assert!(stdout.contains("failed to read file"));
}

#[test]
fn validate_without_a_path_checks_the_workspace() {
    let directory =
        std::env::temp_dir().join(format!("craftward-empty-workspace-{}", std::process::id()));
    fs::create_dir_all(&directory).expect("create directory");
    let workspace = directory.join("workspace.json");
    let workspace = workspace.to_string_lossy().into_owned();
    let output = craftward(&["validate", "--workspace", &workspace]);
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        output.status.success(),
        "empty workspace should validate: {stdout}{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.contains("no projects"));
    let _ = fs::remove_dir_all(directory);
}

#[test]
fn unrecognized_argument_exits_2() {
    let output = craftward(&["nope"]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(stderr.starts_with("error:"));
}

#[test]
fn help_describes_validate_and_exits_0() {
    let output = craftward(&["--help"]);
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert!(stderr.is_empty());
    assert!(stdout.contains("craftward validate [PATH]..."));
    assert!(stdout.contains("plan.phases[0].tasks[1].due"));
}
