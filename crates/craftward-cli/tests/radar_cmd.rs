//! `craftward radar` without using the network.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn scratch(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let path = std::env::temp_dir().join(format!("craftward-radar-cli-{name}-{nanos}"));
    fs::create_dir_all(&path).expect("scratch");
    path
}

fn run(directory: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_craftward"))
        .args(args)
        .current_dir(directory)
        .env("APPDATA", directory)
        .env("LOCALAPPDATA", directory)
        .env("HOME", directory)
        .env("USERPROFILE", directory)
        .env("XDG_CONFIG_HOME", directory)
        .env("XDG_CACHE_HOME", directory)
        .env_remove("CRAFTWARD_WORKSPACE")
        .env_remove("CRAFTWARD_CACHE_DIR")
        .output()
        .expect("run craftward")
}

fn project(directory: &Path, id: &str, lockfiles: &str) -> PathBuf {
    let path = directory.join("craftward.json");
    fs::write(
        &path,
        format!(
            "{{\n  \"schema_version\": 1,\n  \"id\": \"{id}\",\n  \"name\": \"{id}\",\n  \"status\": \"idea\",\n  \"stack\": {{\"components\": [], \"lockfiles\": {lockfiles}}}\n}}\n"
        ),
    )
    .unwrap();
    path
}

fn seed_clean(cache: &Path, now: u64) {
    let file = cache
        .join("queries")
        .join("crates.io")
        .join("serde")
        .join("1.0.0.json");
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(
        file,
        format!("{{\n  \"stored_at\": {now},\n  \"ids\": []\n}}\n"),
    )
    .unwrap();
}

fn seed_hit(cache: &Path, now: u64) {
    seed_clean(cache, now);
    let query = cache
        .join("queries")
        .join("crates.io")
        .join("serde")
        .join("1.0.0.json");
    fs::write(
        query,
        format!("{{\n  \"stored_at\": {now},\n  \"ids\": [\"RUSTSEC-2024-0001\"]\n}}\n"),
    )
    .unwrap();
    let advisory = cache.join("advisories").join("RUSTSEC-2024-0001.json");
    fs::create_dir_all(advisory.parent().unwrap()).unwrap();
    fs::write(
        advisory,
        format!(
            "{{\n  \"stored_at\": {now},\n  \"advisory\": {{\n    \"id\": \"RUSTSEC-2024-0001\",\n    \"aliases\": [\"CVE-2024-1\"],\n    \"summary\": \"Example\",\n    \"severity\": \"critical\",\n    \"affected\": [{{\"ecosystem\": \"crates.io\", \"name\": \"serde\", \"fixed\": [\"1.0.1\"]}}]\n  }}\n}}\n"
        ),
    )
    .unwrap();
}

fn cargo_lock(directory: &Path) {
    fs::write(
        directory.join("Cargo.lock"),
        "version = 4\n\n[[package]]\nname = \"serde\"\nversion = \"1.0.0\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\n",
    )
    .unwrap();
}

#[test]
fn exit_codes_json_and_a_project_without_lockfiles() {
    let root = scratch("codes");
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_secs();
    let clean = root.join("clean");
    fs::create_dir_all(&clean).unwrap();
    let clean_file = project(&clean, "clean", "[\"Cargo.lock\"]");
    cargo_lock(&clean);
    let cache = root.join("cache");
    seed_clean(&cache, now);
    let quiet = run(
        &root,
        &[
            "radar",
            "--offline",
            "--cache-dir",
            cache.to_str().unwrap(),
            clean_file.to_str().unwrap(),
        ],
    );
    assert_eq!(
        quiet.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&quiet.stderr)
    );
    let text = String::from_utf8_lossy(&quiet.stdout);
    assert!(text.contains("no findings"), "{text}");

    seed_hit(&cache, now);
    let hit = run(
        &root,
        &[
            "radar",
            "--offline",
            "--json",
            "--cache-dir",
            cache.to_str().unwrap(),
            clean_file.to_str().unwrap(),
        ],
    );
    assert_eq!(
        hit.status.code(),
        Some(3),
        "{}",
        String::from_utf8_lossy(&hit.stderr)
    );
    let json = String::from_utf8_lossy(&hit.stdout);
    assert!(json.contains("\"severity\": \"critical\""), "{json}");
    assert!(
        json.contains("https://osv.dev/vulnerability/RUSTSEC-2024-0001"),
        "{json}"
    );

    let broken = root.join("broken");
    fs::create_dir_all(&broken).unwrap();
    let broken_file = project(&broken, "broken", "[\"Cargo.lock\"]");
    fs::write(broken.join("Cargo.lock"), "version = 1\n").unwrap();
    let failed = run(
        &root,
        &[
            "radar",
            "--offline",
            "--cache-dir",
            cache.to_str().unwrap(),
            broken_file.to_str().unwrap(),
        ],
    );
    assert_eq!(failed.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&failed.stdout).contains("not supported"));

    let empty = root.join("empty");
    fs::create_dir_all(&empty).unwrap();
    let empty_file = empty.join("craftward.json");
    fs::write(
        &empty_file,
        "{\n  \"schema_version\": 1,\n  \"id\": \"empty\",\n  \"name\": \"Empty\",\n  \"status\": \"idea\"\n}\n",
    )
    .unwrap();
    let none = run(
        &root,
        &[
            "radar",
            "--offline",
            "--cache-dir",
            cache.to_str().unwrap(),
            empty_file.to_str().unwrap(),
        ],
    );
    assert_eq!(none.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&none.stdout).contains("no lockfiles configured"));

    let usage = run(&root, &["radar", "--refresh", "--offline"]);
    assert_eq!(usage.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&usage.stderr).contains("error:"));
    let help = run(&root, &["radar", "--help"]);
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("--cache-dir"));
    let _ = fs::remove_dir_all(root);
}
