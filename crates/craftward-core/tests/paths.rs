//! `source_dir` and lockfile resolution.

use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use craftward_core::{MissingPathKind, load_from_str, missing_paths, resolve_project_paths};

fn scratch(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let path = std::env::temp_dir().join(format!("craftward-paths-{name}-{nanos}"));
    fs::create_dir_all(&path).unwrap();
    path
}

fn project(source_dir: Option<&str>, lockfiles: &[&str]) -> craftward_core::Project {
    let mut text =
        String::from(r#"{"schema_version":1,"id":"harbor-light","name":"Harbor","status":"idea""#);
    if let Some(source_dir) = source_dir {
        let _ = write!(text, r#","source_dir":"{source_dir}""#);
    }
    if !lockfiles.is_empty() {
        let joined = lockfiles
            .iter()
            .map(|path| format!("\"{path}\""))
            .collect::<Vec<_>>()
            .join(",");
        let _ = write!(
            text,
            r#","stack":{{"components":[],"lockfiles":[{joined}]}}"#
        );
    }
    text.push('}');
    load_from_str(&text).expect("project")
}

#[test]
fn source_dir_may_be_absent_relative_absolute_or_parent() {
    assert!(project(None, &[]).source_dir.is_none());
    assert_eq!(
        project(Some("src/app"), &[]).source_dir.as_deref(),
        Some("src/app")
    );
    assert_eq!(
        project(Some("../repo"), &[]).source_dir.as_deref(),
        Some("../repo")
    );
    assert_eq!(
        project(Some("C:/src"), &[]).source_dir.as_deref(),
        Some("C:/src")
    );
    assert_eq!(
        project(Some("/src"), &[]).source_dir.as_deref(),
        Some("/src")
    );
    let error = load_from_str(
        r#"{"schema_version":1,"id":"harbor-light","name":"Harbor","status":"idea","source_dir":""}"#,
    )
    .expect_err("empty");
    let craftward_core::LoadError::Validation(errors) = error else {
        panic!("expected validation");
    };
    assert!(errors.iter().any(|error| error.path == "source_dir"));
}

#[test]
fn lockfiles_resolve_from_source_dir() {
    let root = scratch("resolve");
    let outside = root.join("outside");
    let repo = root.join("repo");
    fs::create_dir_all(&outside).unwrap();
    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join("Cargo.lock"), "").unwrap();
    let project_file = outside.join("craftward.json");

    let absent = project(None, &["Cargo.lock"]);
    let resolved = resolve_project_paths(&project_file, &absent);
    assert_eq!(resolved.source_dir, outside);
    assert_eq!(resolved.lockfiles, vec![outside.join("Cargo.lock")]);

    let relative = project(Some("src"), &["Cargo.lock"]);
    let from_repo = root.join("repo").join("craftward.json");
    let resolved = resolve_project_paths(&from_repo, &relative);
    assert_eq!(resolved.source_dir, repo.join("src"));
    assert_eq!(
        resolved.lockfiles,
        vec![repo.join("src").join("Cargo.lock")]
    );

    let parent = project(Some("../repo"), &["Cargo.lock"]);
    let resolved = resolve_project_paths(&project_file, &parent);
    assert_eq!(
        fs::canonicalize(&resolved.source_dir).unwrap(),
        fs::canonicalize(&repo).unwrap()
    );
    assert_eq!(
        fs::canonicalize(&resolved.lockfiles[0]).unwrap(),
        fs::canonicalize(repo.join("Cargo.lock")).unwrap()
    );
    let shown = resolved.source_dir.to_string_lossy();
    let foreign = if std::path::MAIN_SEPARATOR == '\\' {
        '/'
    } else {
        '\\'
    };
    assert!(!shown.contains(foreign), "mixed separator: {shown}");

    let absolute = std::path::absolute(&repo).unwrap();
    let stored = absolute.to_string_lossy().replace('\\', "/");
    let pointed = project(Some(&stored), &["Cargo.lock"]);
    let resolved = resolve_project_paths(&project_file, &pointed);
    assert_eq!(
        fs::canonicalize(&resolved.source_dir).unwrap(),
        fs::canonicalize(&repo).unwrap()
    );
    assert!(
        !resolved.source_dir.to_string_lossy().starts_with(r"\\?\"),
        "{}",
        resolved.source_dir.display()
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn missing_paths_warn_without_rejecting_the_file() {
    let root = scratch("missing");
    let project_file = root.join("craftward.json");
    let loaded = project(Some("missing-src"), &["nope.lock"]);
    fs::write(&project_file, "").unwrap();
    assert!(load_from_str(
        r#"{"schema_version":1,"id":"harbor-light","name":"Harbor","status":"idea","source_dir":"missing-src","stack":{"components":[],"lockfiles":["nope.lock"]}}"#
    )
    .is_ok());
    let missing = missing_paths(&project_file, &loaded);
    assert!(
        missing
            .iter()
            .any(|item| item.kind == MissingPathKind::SourceDir)
    );
    assert!(
        missing
            .iter()
            .any(|item| item.kind == MissingPathKind::Lockfile)
    );
    let _ = fs::remove_dir_all(root);
}
