//! `craftward settings` show and set.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn scratch(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let path = std::env::temp_dir().join(format!("craftward-cli-settings-{name}-{nanos}"));
    fs::create_dir_all(&path).expect("create scratch directory");
    path
}

fn run(directory: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_craftward"))
        .args(args)
        .current_dir(directory)
        .env("APPDATA", directory.join("appdata"))
        .env("LOCALAPPDATA", directory.join("local"))
        .env("HOME", directory.join("home"))
        .env("USERPROFILE", directory.join("home"))
        .env("XDG_CONFIG_HOME", directory.join("config"))
        .env("XDG_DATA_HOME", directory.join("share"))
        .env("XDG_CACHE_HOME", directory.join("cachehome"))
        .env_remove("CRAFTWARD_WORKSPACE")
        .env_remove("CRAFTWARD_DATA_DIR")
        .env_remove("CRAFTWARD_CACHE_DIR")
        .output()
        .expect("failed to execute craftward")
}

#[test]
fn settings_show_reports_defaults() {
    let root = scratch("show");
    fs::create_dir_all(root.join("config")).unwrap();
    fs::create_dir_all(root.join("share")).unwrap();
    fs::create_dir_all(root.join("cachehome")).unwrap();
    fs::create_dir_all(root.join("appdata")).unwrap();
    fs::create_dir_all(root.join("local")).unwrap();
    let output = run(&root, &["settings", "show"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("settings_file:"), "{stdout}");
    assert!(stdout.contains("workspace:"), "{stdout}");
    assert!(stdout.contains("data_dir:"), "{stdout}");
    assert!(stdout.contains("cache_dir:"), "{stdout}");
    assert!(stdout.contains("default location"), "{stdout}");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn settings_set_workspace_relocates_and_show_reports_settings() {
    let root = scratch("set-ws");
    fs::create_dir_all(root.join("config")).unwrap();
    fs::create_dir_all(root.join("share")).unwrap();
    fs::create_dir_all(root.join("cachehome")).unwrap();
    fs::create_dir_all(root.join("appdata")).unwrap();
    fs::create_dir_all(root.join("local")).unwrap();

    let old_ws = root.join("old").join("workspace.json");
    let new_ws = root.join("moved").join("workspace.json");

    let seed = run(
        &root,
        &["settings", "set", "workspace", old_ws.to_str().unwrap()],
    );
    assert!(
        seed.status.success(),
        "{}",
        String::from_utf8_lossy(&seed.stderr)
    );
    assert!(old_ws.exists());
    fs::write(
        &old_ws,
        "{\n  \"schema_version\": 1,\n  \"projects\": []\n}\n",
    )
    .unwrap();

    let set_ok = run(
        &root,
        &["settings", "set", "workspace", new_ws.to_str().unwrap()],
    );
    assert!(
        set_ok.status.success(),
        "{}",
        String::from_utf8_lossy(&set_ok.stderr)
    );
    assert!(new_ws.exists());
    assert!(!old_ws.exists());

    let show = run(&root, &["settings", "show"]);
    assert!(show.status.success());
    let stdout = String::from_utf8_lossy(&show.stdout);
    assert!(stdout.contains("settings file"), "{stdout}");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn settings_set_refuses_occupied_data_target() {
    let root = scratch("occupied");
    fs::create_dir_all(root.join("config")).unwrap();
    fs::create_dir_all(root.join("share")).unwrap();
    fs::create_dir_all(root.join("cachehome")).unwrap();
    fs::create_dir_all(root.join("appdata")).unwrap();
    fs::create_dir_all(root.join("local")).unwrap();

    let data_a = root.join("data-a");
    let data_b = root.join("data-b");
    fs::create_dir_all(&data_a).unwrap();
    fs::create_dir_all(&data_b).unwrap();
    fs::write(data_b.join("craftward.db"), b"occupied").unwrap();

    let seed = run(
        &root,
        &["settings", "set", "data-dir", data_a.to_str().unwrap()],
    );
    assert!(
        seed.status.success(),
        "{}",
        String::from_utf8_lossy(&seed.stderr)
    );

    let set = run(
        &root,
        &["settings", "set", "data-dir", data_b.to_str().unwrap()],
    );
    assert_eq!(set.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&set.stderr);
    assert!(stderr.contains("already exists"), "{stderr}");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn settings_set_refuses_env_fixed_workspace() {
    let root = scratch("env-fixed");
    fs::create_dir_all(root.join("config")).unwrap();
    fs::create_dir_all(root.join("share")).unwrap();
    fs::create_dir_all(root.join("cachehome")).unwrap();
    fs::create_dir_all(root.join("appdata")).unwrap();
    fs::create_dir_all(root.join("local")).unwrap();
    let fixed = root.join("fixed").join("workspace.json");
    fs::create_dir_all(fixed.parent().unwrap()).unwrap();
    fs::write(
        &fixed,
        "{\n  \"schema_version\": 1,\n  \"projects\": []\n}\n",
    )
    .unwrap();
    let target = root.join("other").join("workspace.json");
    let output = Command::new(env!("CARGO_BIN_EXE_craftward"))
        .args(["settings", "set", "workspace", target.to_str().unwrap()])
        .current_dir(&root)
        .env("APPDATA", root.join("appdata"))
        .env("LOCALAPPDATA", root.join("local"))
        .env("HOME", root.join("home"))
        .env("USERPROFILE", root.join("home"))
        .env("XDG_CONFIG_HOME", root.join("config"))
        .env("XDG_DATA_HOME", root.join("share"))
        .env("XDG_CACHE_HOME", root.join("cachehome"))
        .env("CRAFTWARD_WORKSPACE", &fixed)
        .env_remove("CRAFTWARD_DATA_DIR")
        .env_remove("CRAFTWARD_CACHE_DIR")
        .output()
        .expect("execute");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("fixed by"), "{stderr}");
    assert!(!target.exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn global_workspace_option_is_not_the_set_destination() {
    let root = scratch("global-vs-positional");
    fs::create_dir_all(root.join("config")).unwrap();
    fs::create_dir_all(root.join("share")).unwrap();
    fs::create_dir_all(root.join("cachehome")).unwrap();
    fs::create_dir_all(root.join("appdata")).unwrap();
    fs::create_dir_all(root.join("local")).unwrap();

    let via_global = root.join("via-global").join("workspace.json");
    let via_positional = root.join("via-positional").join("workspace.json");
    fs::create_dir_all(via_global.parent().unwrap()).unwrap();
    fs::write(
        &via_global,
        "{\n  \"schema_version\": 1,\n  \"projects\": []\n}\n",
    )
    .unwrap();

    // Global --workspace fixes the current location for this call; the
    // positional path is the only candidate destination, so set must refuse.
    let refused = run(
        &root,
        &[
            "--workspace",
            via_global.to_str().unwrap(),
            "settings",
            "set",
            "workspace",
            via_positional.to_str().unwrap(),
        ],
    );
    assert_eq!(
        refused.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&refused.stderr)
    );
    let stderr = String::from_utf8_lossy(&refused.stderr);
    assert!(
        stderr.contains("fixed by") && stderr.contains("command-line"),
        "{stderr}"
    );
    assert!(!via_positional.exists());
    assert!(via_global.exists());

    // Without the global option, the positional argument is the destination.
    let ok = run(
        &root,
        &[
            "settings",
            "set",
            "workspace",
            via_positional.to_str().unwrap(),
        ],
    );
    assert!(
        ok.status.success(),
        "{}",
        String::from_utf8_lossy(&ok.stderr)
    );
    assert!(via_positional.exists());

    let show = run(&root, &["settings", "show"]);
    let stdout = String::from_utf8_lossy(&show.stdout);
    assert!(
        stdout.contains(via_positional.to_str().unwrap()) || stdout.contains("via-positional"),
        "{stdout}"
    );
    assert!(!stdout.contains("via-global"), "{stdout}");
    let _ = fs::remove_dir_all(root);
}
