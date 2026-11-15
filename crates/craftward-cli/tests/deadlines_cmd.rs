//! `stack android`, `deadlines`, and Play deadline exit status.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn scratch(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let path = std::env::temp_dir().join(format!("craftward-android-{name}-{nanos}"));
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
fn stack_android_writes_only_a_valid_file() {
    let root = scratch("android");
    let file = project(&root);
    let path = file.to_str().unwrap();
    let before = fs::read_to_string(&file).unwrap();
    let bad_factor = run(
        &root,
        &["stack", "android", path, "--form-factor", "fridge"],
    );
    assert_eq!(bad_factor.status.code(), Some(2), "{}", stderr(&bad_factor));
    assert_eq!(fs::read_to_string(&file).unwrap(), before);

    let bad_sdk = run(&root, &["stack", "android", path, "--target-sdk", "0"]);
    assert_eq!(bad_sdk.status.code(), Some(2), "{}", stderr(&bad_sdk));
    assert_eq!(fs::read_to_string(&file).unwrap(), before);

    let bad_path = run(
        &root,
        &["stack", "android", path, "--detect-from", "../secret"],
    );
    assert_eq!(bad_path.status.code(), Some(1), "{}", stderr(&bad_path));
    assert_eq!(fs::read_to_string(&file).unwrap(), before);

    let ok = run(
        &root,
        &[
            "stack",
            "android",
            path,
            "--target-sdk",
            "33",
            "--min-sdk",
            "24",
            "--form-factor",
            "wear",
            "--store",
            "google_play",
            "--detect-from",
            "app/build.gradle.kts",
            "--detect-from",
            "gradle/libs.versions.toml",
        ],
    );
    assert!(ok.status.success(), "{}", stderr(&ok));
    assert!(String::from_utf8_lossy(&ok.stdout).contains("updated android settings"));
    let text = fs::read_to_string(&file).unwrap();
    assert!(text.contains("\"target_sdk\": 33"), "{text}");
    assert!(text.contains("\"min_sdk\": 24"), "{text}");
    assert!(text.contains("\"form_factor\": \"wear\""), "{text}");
    assert!(text.contains("app/build.gradle.kts"), "{text}");
    assert!(text.contains("\"updated\""), "{text}");

    let cleared = run(&root, &["stack", "android", path, "--target-sdk", ""]);
    assert!(cleared.status.success(), "{}", stderr(&cleared));
    let text = fs::read_to_string(&file).unwrap();
    assert!(!text.contains("target_sdk"), "{text}");
    assert!(text.contains("\"min_sdk\": 24"), "{text}");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn radar_exit_codes_follow_strict_and_a_raised_target_sdk() {
    let root = scratch("radar");
    let file = project(&root);
    let path = file.to_str().unwrap();
    fs::create_dir_all(root.join("app")).unwrap();
    fs::create_dir_all(root.join("gradle")).unwrap();
    fs::write(
        root.join("gradle/libs.versions.toml"),
        "[versions]\ntargetSdk = \"30\"\n",
    )
    .unwrap();
    fs::write(
        root.join("app/build.gradle.kts"),
        "targetSdk = libs.versions.targetSdk.get().toInt()\n",
    )
    .unwrap();
    fs::create_dir_all(root.join("data")).unwrap();
    fs::write(root.join("data/platform-deadlines.json"), PAST).unwrap();
    let data = root.join("data");
    let cache = root.join("cache");
    let set = run(
        &root,
        &[
            "stack",
            "android",
            path,
            "--detect-from",
            "app/build.gradle.kts",
            "--detect-from",
            "gradle/libs.versions.toml",
        ],
    );
    assert!(set.status.success(), "{}", stderr(&set));

    let first = radar(&root, path, &data, &cache, &[]);
    assert_eq!(first.status.code(), Some(3), "{}", stdout(&first));
    let text = stdout(&first);
    assert!(text.contains("platform_deadline"), "{text}");
    assert!(text.contains("targetSdk 30"), "{text}");
    assert!(text.contains("app/build.gradle.kts:1"), "{text}");

    fs::write(
        root.join("gradle/libs.versions.toml"),
        "[versions]\ntargetSdk = \"40\"\n",
    )
    .unwrap();
    let second = radar(&root, path, &data, &cache, &[]);
    assert!(second.status.success(), "{}", stdout(&second));
    assert!(
        stdout(&second).contains("resolved: 1"),
        "{}",
        stdout(&second)
    );

    fs::write(root.join("data/platform-deadlines.json"), FUTURE).unwrap();
    fs::write(
        root.join("gradle/libs.versions.toml"),
        "[versions]\ntargetSdk = \"30\"\n",
    )
    .unwrap();
    let quiet = radar(&root, path, &data, &cache, &[]);
    assert!(quiet.status.success(), "{}", stdout(&quiet));
    let strict = radar(
        &root,
        path,
        &data,
        &cache,
        &["--deadline-warning-days", "4000", "--strict"],
    );
    assert_eq!(strict.status.code(), Some(3), "{}", stdout(&strict));
    assert!(
        stdout(&strict).contains("platform_deadline_soon"),
        "{}",
        stdout(&strict)
    );
    let loose = radar(
        &root,
        path,
        &data,
        &cache,
        &["--deadline-warning-days", "4000"],
    );
    assert!(loose.status.success(), "{}", stdout(&loose));
    let _ = fs::remove_dir_all(root);
}

fn radar(
    root: &Path,
    project: &str,
    data: &Path,
    cache: &Path,
    extra: &[&str],
) -> std::process::Output {
    let mut args = vec![
        "radar",
        project,
        "--offline",
        "--data-dir",
        data.to_str().unwrap(),
        "--cache-dir",
        cache.to_str().unwrap(),
    ];
    args.extend(extra);
    run(root, &args)
}

fn stdout(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

const PAST: &str = r#"{
  "schema_version": 1,
  "data_version": "2026-11-16",
  "deadlines": [{
    "platform": "google_play",
    "form_factor": "phone",
    "requirement": "new_apps_and_updates",
    "api_level": 35,
    "deadline": "2020-01-01",
    "source": "https://example.com/past",
    "checked_on": "2026-11-15",
    "note": "past"
  }]
}
"#;

const FUTURE: &str = r#"{
  "schema_version": 1,
  "data_version": "2026-11-16",
  "deadlines": [{
    "platform": "google_play",
    "form_factor": "phone",
    "requirement": "new_apps_and_updates",
    "api_level": 35,
    "deadline": "2036-01-01",
    "source": "https://example.com/future",
    "checked_on": "2026-11-15",
    "note": "future"
  }]
}
"#;
