//! Rules for schema versions 1 and 2 of a project file.

use std::fs;
use std::path::PathBuf;

use craftward_core::{LoadError, ValidationError, load_from_path, load_from_str};

fn minimal() -> String {
    r#"{"schema_version":1,"id":"harbor-light","name":"Harbor Light","status":"idea"}"#.to_owned()
}

fn validation_errors(text: &str) -> Vec<ValidationError> {
    match load_from_str(text) {
        Err(LoadError::Validation(errors)) => errors,
        Ok(_) => panic!("expected validation errors, but the file was accepted"),
        Err(error) => panic!("expected validation errors, got {error}"),
    }
}

fn assert_only(text: &str, path: &str, message: &str) {
    let errors = validation_errors(text);
    assert_eq!(
        errors,
        vec![ValidationError {
            path: path.to_owned(),
            message: message.to_owned(),
        }],
        "unexpected errors: {errors:?}"
    );
}

#[test]
fn example_files_are_valid() {
    for name in ["minimal.craftward.json", "full.craftward.json"] {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples")
            .join(name);
        let project = load_from_path(&path).unwrap_or_else(|error| {
            panic!("{} should be valid: {error}", path.display());
        });
        assert_eq!(project.schema_version, 1);
        assert_eq!(project.id, "harbor-light");
    }
}

#[test]
fn same_task_id_is_allowed_in_different_phases() {
    let text = r#"
    {
      "schema_version": 1,
      "id": "harbor-light",
      "name": "Harbor Light",
      "status": "idea",
      "plan": {
        "phases": [
          {
            "id": "one",
            "title": "One",
            "tasks": [{ "id": "schema", "title": "Write it", "status": "open" }]
          },
          {
            "id": "two",
            "title": "Two",
            "tasks": [{ "id": "schema", "title": "Tag it", "status": "done" }]
          }
        ]
      }
    }
    "#;
    let project = load_from_str(text).expect("task ids are scoped to their phase");
    let plan = project.plan.expect("plan");
    assert_eq!(plan.phases.len(), 2);
}

#[test]
fn optional_null_is_treated_as_absent() {
    let text = r#"
    {
      "schema_version": 1,
      "id": "harbor-light",
      "name": "Harbor Light",
      "status": "idea",
      "description": null
    }
    "#;
    let project = load_from_str(text).expect("null optional field");
    assert_eq!(project.description, None);
}

#[test]
fn repeated_hyphen_inside_an_id_is_valid() {
    let text = r#"{"schema_version":1,"id":"a--b","name":"A","status":"paused"}"#;
    assert!(load_from_str(text).is_ok());
}

#[test]
fn rejects_a_status_outside_the_set() {
    let text =
        r#"{"schema_version":1,"id":"harbor-light","name":"Harbor Light","status":"archived"}"#;
    assert_only(
        text,
        "status",
        "must be one of idea, in_development, released, maintenance, paused, discontinued",
    );
}

#[test]
fn rejects_a_duplicate_phase_id() {
    let text = r#"
    {
      "schema_version": 1,
      "id": "harbor-light",
      "name": "Harbor Light",
      "status": "idea",
      "plan": {
        "phases": [
          { "id": "foundation", "title": "Foundation", "tasks": [] },
          { "id": "foundation", "title": "Again", "tasks": [] }
        ]
      }
    }
    "#;
    assert_only(text, "plan.phases[1].id", "duplicate id");
}

#[test]
fn rejects_a_duplicate_task_id_in_one_phase() {
    let text = r#"
    {
      "schema_version": 1,
      "id": "harbor-light",
      "name": "Harbor Light",
      "status": "idea",
      "plan": {
        "phases": [
          {
            "id": "foundation",
            "title": "Foundation",
            "tasks": [
              { "id": "schema", "title": "Write it", "status": "open" },
              { "id": "schema", "title": "Write it again", "status": "open" }
            ]
          }
        ]
      }
    }
    "#;
    assert_only(text, "plan.phases[0].tasks[1].id", "duplicate id");
}

#[test]
fn rejects_a_duplicate_issue_id() {
    let text = r#"
    {
      "schema_version": 1,
      "id": "harbor-light",
      "name": "Harbor Light",
      "status": "idea",
      "issues": [
        { "id": "theme", "title": "Contrast", "severity": "low", "status": "open" },
        { "id": "theme", "title": "Contrast again", "severity": "high", "status": "resolved" }
      ]
    }
    "#;
    assert_only(text, "issues[1].id", "duplicate id");
}

#[test]
fn rejects_a_duplicate_watch_item_id() {
    let text = r#"
    {
      "schema_version": 1,
      "id": "harbor-light",
      "name": "Harbor Light",
      "status": "idea",
      "watch_items": [
        { "id": "renewal", "kind": "expiry", "title": "Renew", "due": "2027-06-30" },
        { "id": "renewal", "kind": "review", "title": "Review", "due": "2027-07-01" }
      ]
    }
    "#;
    assert_only(text, "watch_items[1].id", "duplicate id");
}

#[test]
fn rejects_a_duplicate_tag() {
    let text = r#"
    {
      "schema_version": 1,
      "id": "harbor-light",
      "name": "Harbor Light",
      "status": "idea",
      "tags": ["rust", "rust"]
    }
    "#;
    assert_only(text, "tags[1]", "duplicate tag");
}

#[test]
fn rejects_a_date_that_is_not_on_the_calendar() {
    let text = r#"
    {
      "schema_version": 1,
      "id": "harbor-light",
      "name": "Harbor Light",
      "status": "idea",
      "updated": "2026-02-30"
    }
    "#;
    assert_only(text, "updated", "is not a calendar date");
}

#[test]
fn rejects_a_date_with_the_wrong_shape() {
    let text = r#"
    {
      "schema_version": 1,
      "id": "harbor-light",
      "name": "Harbor Light",
      "status": "idea",
      "plan": {
        "phases": [
          {
            "id": "foundation",
            "title": "Foundation",
            "tasks": [
              { "id": "schema", "title": "Write it", "status": "open", "due": "2026/02/28" }
            ]
          }
        ]
      }
    }
    "#;
    assert_only(text, "plan.phases[0].tasks[0].due", "must use YYYY-MM-DD");
}

#[test]
fn rejects_an_absolute_lockfile_path() {
    let text = r#"
    {
      "schema_version": 1,
      "id": "harbor-light",
      "name": "Harbor Light",
      "status": "idea",
      "stack": { "components": [], "lockfiles": ["C:/deps/Cargo.lock"] }
    }
    "#;
    assert_only(text, "stack.lockfiles[0]", "must be a relative path");
}

#[test]
fn rejects_a_parent_directory_segment_in_a_lockfile_path() {
    let text = r#"
    {
      "schema_version": 1,
      "id": "harbor-light",
      "name": "Harbor Light",
      "status": "idea",
      "stack": { "components": [], "lockfiles": ["deps/../Cargo.lock"] }
    }
    "#;
    assert_only(
        text,
        "stack.lockfiles[0]",
        "must not contain a '..' path segment",
    );
}

#[test]
fn rejects_an_unknown_field() {
    let text = r#"
    {
      "schema_version": 1,
      "id": "harbor-light",
      "name": "Harbor Light",
      "status": "idea",
      "owner": "Ada"
    }
    "#;
    assert_only(text, "owner", "unknown field");
}

#[test]
fn rejects_schema_version_three_before_other_checks() {
    let text = r#"{"schema_version":3,"id":"-","name":"","status":"nope"}"#;
    let error = load_from_str(text).expect_err("newer schema");
    assert_eq!(error, LoadError::UnsupportedSchema { version: 3 });
    assert_eq!(
        error.to_string(),
        "file was written by a newer Craftward version"
    );
}

#[test]
fn rejects_a_schema_version_that_is_not_one_or_two() {
    assert_only(
        r#"{"schema_version":0,"id":"harbor-light","name":"Harbor Light","status":"idea"}"#,
        "schema_version",
        "must be 1 or 2",
    );
    assert_only(
        r#"{"schema_version":1.0,"id":"harbor-light","name":"Harbor Light","status":"idea"}"#,
        "schema_version",
        "must be 1 or 2",
    );
    assert_only(
        r#"{"schema_version":"1","id":"harbor-light","name":"Harbor Light","status":"idea"}"#,
        "schema_version",
        "must be 1 or 2",
    );
    assert_only(
        r#"{"id":"harbor-light","name":"Harbor Light","status":"idea"}"#,
        "schema_version",
        "is required",
    );
}

#[test]
fn accepts_schema_version_two_with_category() {
    let text = r#"
    {
      "schema_version": 2,
      "id": "harbor-light",
      "name": "Harbor Light",
      "status": "idea",
      "category": "  Security   Tools  "
    }
    "#;
    let project = load_from_str(text).expect("v2 with category");
    assert_eq!(project.schema_version, 2);
    assert_eq!(project.category.as_deref(), Some("Security Tools"));
}

#[test]
fn rejects_category_on_schema_version_one() {
    let text = r#"
    {
      "schema_version": 1,
      "id": "harbor-light",
      "name": "Harbor Light",
      "status": "idea",
      "category": "Tools"
    }
    "#;
    assert_only(text, "category", "unknown field");
}

#[test]
fn rejects_an_invalid_category() {
    let text = r#"
    {
      "schema_version": 2,
      "id": "harbor-light",
      "name": "Harbor Light",
      "status": "idea",
      "category": "bad\nname"
    }
    "#;
    assert_only(
        text,
        "category",
        "must be 1–40 characters after trimming, with no control characters",
    );
}

#[test]
fn rejects_an_empty_name() {
    assert_only(
        r#"{"schema_version":1,"id":"harbor-light","name":"","status":"idea"}"#,
        "name",
        "must not be empty or whitespace",
    );
    assert_only(
        r#"{"schema_version":1,"id":"harbor-light","name":"  \t","status":"idea"}"#,
        "name",
        "must not be empty or whitespace",
    );
}

#[test]
fn rejects_a_blank_title() {
    let text = r#"
    {
      "schema_version": 1,
      "id": "harbor-light",
      "name": "Harbor Light",
      "status": "idea",
      "issues": [
        { "id": "theme", "title": "   ", "severity": "low", "status": "open" }
      ]
    }
    "#;
    assert_only(text, "issues[0].title", "must not be empty or whitespace");
}

#[test]
fn rejects_an_id_that_breaks_the_short_name_rule() {
    let slug = "must be 1 to 64 characters from a-z, 0-9, and hyphen, and must not start or end with a hyphen";
    assert_only(
        r#"{"schema_version":1,"id":"-harbor","name":"Harbor Light","status":"idea"}"#,
        "id",
        slug,
    );
    assert_only(
        r#"{"schema_version":1,"id":"harbor-","name":"Harbor Light","status":"idea"}"#,
        "id",
        slug,
    );
    assert_only(
        r#"{"schema_version":1,"id":"Harbor","name":"Harbor Light","status":"idea"}"#,
        "id",
        slug,
    );
    let long = "a".repeat(65);
    assert_only(
        &format!(r#"{{"schema_version":1,"id":"{long}","name":"Harbor Light","status":"idea"}}"#),
        "id",
        slug,
    );
}

#[test]
fn rejects_a_url_that_is_not_http() {
    let text = r#"
    {
      "schema_version": 1,
      "id": "harbor-light",
      "name": "Harbor Light",
      "status": "idea",
      "links": [{ "label": "Repo", "url": "ftp://example.com/harbor" }]
    }
    "#;
    assert_only(text, "links[0].url", "must start with http:// or https://");
}

#[test]
fn rejects_a_component_kind_outside_the_set() {
    let text = r#"
    {
      "schema_version": 1,
      "id": "harbor-light",
      "name": "Harbor Light",
      "status": "idea",
      "stack": {
        "components": [{ "kind": "database", "name": "PostgreSQL" }],
        "lockfiles": []
      }
    }
    "#;
    assert_only(
        text,
        "stack.components[0].kind",
        "must be one of platform, language, framework, runtime, sdk, service, tool, other",
    );
}

#[test]
fn rejects_a_task_status_outside_the_set() {
    let text = r#"
    {
      "schema_version": 1,
      "id": "harbor-light",
      "name": "Harbor Light",
      "status": "idea",
      "plan": {
        "phases": [
          {
            "id": "foundation",
            "title": "Foundation",
            "tasks": [{ "id": "schema", "title": "Write it", "status": "later" }]
          }
        ]
      }
    }
    "#;
    assert_only(
        text,
        "plan.phases[0].tasks[0].status",
        "must be one of open, in_progress, done, dropped",
    );
}

#[test]
fn rejects_an_issue_severity_and_status_outside_the_set() {
    let text = r#"
    {
      "schema_version": 1,
      "id": "harbor-light",
      "name": "Harbor Light",
      "status": "idea",
      "issues": [
        { "id": "theme", "title": "Contrast", "severity": "urgent", "status": "closed" }
      ]
    }
    "#;
    let errors = validation_errors(text);
    assert_eq!(errors.len(), 2);
    assert_eq!(errors[0].path, "issues[0].severity");
    assert_eq!(
        errors[0].message,
        "must be one of low, medium, high, critical"
    );
    assert_eq!(errors[1].path, "issues[0].status");
    assert_eq!(
        errors[1].message,
        "must be one of open, in_progress, resolved, wont_fix"
    );
}

#[test]
fn rejects_a_watch_item_kind_outside_the_set() {
    let text = r#"
    {
      "schema_version": 1,
      "id": "harbor-light",
      "name": "Harbor Light",
      "status": "idea",
      "watch_items": [
        { "id": "renewal", "kind": "reminder", "title": "Renew", "due": "2027-06-30" }
      ]
    }
    "#;
    assert_only(
        text,
        "watch_items[0].kind",
        "must be one of deadline, expiry, review, other",
    );
}

#[test]
fn rejects_a_watch_item_without_a_due_date() {
    let text = r#"
    {
      "schema_version": 1,
      "id": "harbor-light",
      "name": "Harbor Light",
      "status": "idea",
      "watch_items": [
        { "id": "renewal", "kind": "expiry", "title": "Renew" }
      ]
    }
    "#;
    assert_only(text, "watch_items[0].due", "is required");
}

#[test]
fn reports_every_error_in_one_file() {
    let text = r#"
    {
      "schema_version": 1,
      "id": "-bad",
      "name": "   ",
      "status": "archived",
      "owner": true,
      "updated": "2026-02-30",
      "tags": ["rust", "rust"],
      "stack": { "components": [], "lockfiles": ["/tmp/Cargo.lock", "deps/../Cargo.lock"] }
    }
    "#;
    let errors = validation_errors(text);
    let found: Vec<(&str, &str)> = errors
        .iter()
        .map(|error| (error.path.as_str(), error.message.as_str()))
        .collect();
    let expected = [
        (
            "id",
            "must be 1 to 64 characters from a-z, 0-9, and hyphen, and must not start or end with a hyphen",
        ),
        ("name", "must not be empty or whitespace"),
        (
            "status",
            "must be one of idea, in_development, released, maintenance, paused, discontinued",
        ),
        ("owner", "unknown field"),
        ("updated", "is not a calendar date"),
        ("tags[1]", "duplicate tag"),
        ("stack.lockfiles[0]", "must be a relative path"),
        ("stack.lockfiles[1]", "must not contain a '..' path segment"),
    ];
    for item in expected {
        assert!(found.contains(&item), "missing {item:?} in {found:?}");
    }
    assert_eq!(errors.len(), expected.len());
}

#[test]
fn syntax_error_names_the_line_and_column() {
    let text = "{\n  \"schema_version\":\n";
    let parsed = serde_json::from_str::<serde_json::Value>(text).expect_err("broken json");
    match load_from_str(text) {
        Err(LoadError::Syntax {
            line,
            column,
            message,
        }) => {
            assert_eq!(line, parsed.line());
            assert_eq!(column, parsed.column());
            assert_eq!(message, parsed.to_string());
        }
        other => panic!("expected a syntax error, got {other:?}"),
    }
}

#[test]
fn missing_file_is_a_read_error() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("this-file-is-not-there.json");
    match load_from_path(&path) {
        Err(LoadError::Read(_)) => {}
        other => panic!("expected a read error, got {other:?}"),
    }
}

#[test]
fn invalid_utf8_is_a_read_error() {
    let path = std::env::temp_dir().join("craftward-invalid-utf8.json");
    fs::write(&path, [0xFF, 0xFE, b'{']).expect("write temp file");
    match load_from_path(&path) {
        Err(LoadError::Read(_)) => {}
        other => panic!("expected a read error, got {other:?}"),
    }
    let _ = fs::remove_file(path);
}

#[test]
fn minimal_document_is_valid() {
    let project = load_from_str(&minimal()).expect("minimal document");
    assert_eq!(project.id, "harbor-light");
    assert_eq!(project.status, craftward_core::ProjectStatus::Idea);
}
