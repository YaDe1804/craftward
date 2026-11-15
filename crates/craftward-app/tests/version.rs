//! Checks `--version` output for the `craftward-app` binary.

use std::process::Command;

#[test]
fn version_flag_prints_name_and_version() {
    let output = Command::new(env!("CARGO_BIN_EXE_craftward-app"))
        .arg("--version")
        .output()
        .expect("failed to execute craftward-app --version");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "craftward-app --version failed: {stderr}"
    );
    assert!(stderr.is_empty(), "unexpected stderr: {stderr}");

    let stdout = String::from_utf8(output.stdout).expect("stdout is not valid UTF-8");
    let line = stdout.trim_end_matches(['\r', '\n']);
    let version = craftward_core::version();
    assert_eq!(line, format!("craftward-app {version}"));
}
