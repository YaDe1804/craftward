//! `craftward stack` writes only a valid project file.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn scratch(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let path = std::env::temp_dir().join(format!("craftward-stack-{name}-{nanos}"));
    fs::create_dir_all(&path).expect("scratch");
    path
}

fn run(directory: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_craftward"))
        .args(args)
        .current_dir(directory)
        .env_remove("CRAFTWARD_WORKSPACE")
        .output()
        .expect("run craftward")
}

fn project(directory: &Path) -> PathBuf {
    let path = directory.join("craftward.json");
    fs::write(
        &path,
        "{\n  \"schema_version\": 1,\n  \"id\": \"demo\",\n  \"name\": \"Demo\",\n  \"status\": \"idea\"\n}\n",
    )
    .unwrap();
    path
}

#[test]
fn stack_commands_add_change_clear_and_reject() {
    let root = scratch("stack");
    let file = project(&root);
    let path = file.to_str().unwrap();
    let added = run(
        &root,
        &[
            "stack",
            "add",
            path,
            "--kind",
            "runtime",
            "--name",
            "PostgreSQL",
            "--version",
            "16.1",
            "--note",
            "primary",
            "--eol-product",
            "postgresql",
        ],
    );
    assert!(
        added.status.success(),
        "{}",
        String::from_utf8_lossy(&added.stderr)
    );
    assert!(String::from_utf8_lossy(&added.stdout).contains("added component PostgreSQL"));
    let text = fs::read_to_string(&file).unwrap();
    assert!(text.contains("\"eol_product\": \"postgresql\""), "{text}");
    assert!(text.contains("\"updated\""), "{text}");

    let changed = run(
        &root,
        &[
            "stack",
            "set",
            path,
            "postgresql",
            "--version",
            "",
            "--note",
            "",
            "--eol-product",
            "",
        ],
    );
    assert!(
        changed.status.success(),
        "{}",
        String::from_utf8_lossy(&changed.stderr)
    );
    let cleared = fs::read_to_string(&file).unwrap();
    assert!(!cleared.contains("eol_product"), "{cleared}");
    assert!(!cleared.contains("\"version\""), "{cleared}");
    assert!(!cleared.contains("\"note\""), "{cleared}");
    set_eol_product_none(&root, path, &file);

    let unknown = run(&root, &["stack", "remove", path, "missing"]);
    assert_eq!(unknown.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&unknown.stderr).contains("unknown component"),
        "{}",
        String::from_utf8_lossy(&unknown.stderr)
    );

    let duplicate = run(
        &root,
        &[
            "stack",
            "add",
            path,
            "--kind",
            "tool",
            "--name",
            "postgresql",
        ],
    );
    assert_eq!(duplicate.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&duplicate.stderr).contains("duplicate component"));

    let before = fs::read_to_string(&file).unwrap();
    let invalid = run(
        &root,
        &["stack", "lockfile", "add", path, "/tmp/Cargo.lock"],
    );
    assert_eq!(invalid.status.code(), Some(1));
    assert_eq!(fs::read_to_string(&file).unwrap(), before);

    let lock = run(&root, &["stack", "lockfile", "add", path, "Cargo.lock"]);
    assert!(
        lock.status.success(),
        "{}",
        String::from_utf8_lossy(&lock.stderr)
    );
    let removed = run(&root, &["stack", "lockfile", "remove", path, "Cargo.lock"]);
    assert!(removed.status.success());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn show_prints_stack_components_and_lockfiles() {
    let root = scratch("show-stack");
    let file = project(&root);
    let path = file.to_str().unwrap();
    assert!(
        run(
            &root,
            &[
                "stack",
                "add",
                path,
                "--kind",
                "runtime",
                "--name",
                "PostgreSQL",
            ],
        )
        .status
        .success()
    );
    assert!(
        run(&root, &["stack", "lockfile", "add", path, "Cargo.lock"])
            .status
            .success()
    );
    let shown = run(&root, &["show", path]);
    assert!(
        shown.status.success(),
        "{}",
        String::from_utf8_lossy(&shown.stderr)
    );
    let view = String::from_utf8_lossy(&shown.stdout);
    assert!(view.contains("component [runtime] PostgreSQL"), "{view}");
    assert!(view.contains("lockfile Cargo.lock"), "{view}");
    let _ = fs::remove_dir_all(root);
}

fn set_eol_product_none(root: &Path, path: &str, file: &Path) {
    let disabled = run(
        root,
        &["stack", "set", path, "postgresql", "--eol-product", "none"],
    );
    assert!(
        disabled.status.success(),
        "{}",
        String::from_utf8_lossy(&disabled.stderr)
    );
    assert!(
        fs::read_to_string(file)
            .unwrap()
            .contains("\"eol_product\": \"none\"")
    );
}

#[test]
fn an_ambiguous_component_name_is_an_error() {
    let root = scratch("ambiguous");
    let file = project(&root);
    let path = file.to_str().unwrap();
    fs::write(
        &file,
        "{\n  \"schema_version\": 1,\n  \"id\": \"demo\",\n  \"name\": \"Demo\",\n  \"status\": \"idea\",\n  \"stack\": {\"components\": [{\"kind\": \"tool\", \"name\": \"Node\"}, {\"kind\": \"runtime\", \"name\": \"node\"}], \"lockfiles\": []}\n}\n",
    )
    .unwrap();
    let output = run(&root, &["stack", "set", path, "NODE", "--note", "x"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("more than one component"));
    let _ = fs::remove_dir_all(root);
}
