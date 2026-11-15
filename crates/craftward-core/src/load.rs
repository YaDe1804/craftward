//! Load and check schema-version-1 project files.

use std::fs;
use std::path::Path;

use serde_json::{Map, Number, Value};

use crate::date::{DateError, validate_date};
use crate::error::{LoadError, ValidationError};
use crate::model::Project;

const PROJECT_FIELDS: &[&str] = &[
    "schema_version",
    "id",
    "name",
    "status",
    "description",
    "current_version",
    "tags",
    "updated",
    "links",
    "stack",
    "plan",
    "issues",
    "watch_items",
];

const PROJECT_STATUSES: &[&str] = &[
    "idea",
    "in_development",
    "released",
    "maintenance",
    "paused",
    "discontinued",
];

const COMPONENT_KINDS: &[&str] = &[
    "platform",
    "language",
    "framework",
    "runtime",
    "sdk",
    "service",
    "tool",
    "other",
];

const TASK_STATUSES: &[&str] = &["open", "in_progress", "done", "dropped"];

const ISSUE_SEVERITIES: &[&str] = &["low", "medium", "high", "critical"];

const ISSUE_STATUSES: &[&str] = &["open", "in_progress", "resolved", "wont_fix"];

const WATCH_KINDS: &[&str] = &["deadline", "expiry", "review", "other"];

const LINK_FIELDS: &[&str] = &["label", "url"];
const STACK_FIELDS: &[&str] = &["components", "lockfiles"];
const COMPONENT_FIELDS: &[&str] = &["kind", "name", "version", "note"];
const PLAN_FIELDS: &[&str] = &["phases"];
const PHASE_FIELDS: &[&str] = &["id", "title", "goal", "tasks"];
const TASK_FIELDS: &[&str] = &["id", "title", "status", "due", "note"];
const ISSUE_FIELDS: &[&str] = &["id", "title", "severity", "status", "url", "note"];
const WATCH_FIELDS: &[&str] = &["id", "kind", "title", "due", "note"];

/// Loads a project from JSON text.
///
/// # Errors
///
/// Returns [`LoadError::Syntax`] when `text` is not JSON,
/// [`LoadError::UnsupportedSchema`] when `schema_version` is an integer greater
/// than 1, and [`LoadError::Validation`] when a schema-version-1 file breaks
/// one or more rules. Validation reports every problem it finds.
pub fn load_from_str(text: &str) -> Result<Project, LoadError> {
    let value = serde_json::from_str(text).map_err(|error| syntax_error(&error))?;
    load_value(value)
}

/// Loads a project from a UTF-8 file.
///
/// # Errors
///
/// Returns [`LoadError::Read`] when the file cannot be read, then the same
/// errors as [`load_from_str`].
pub fn load_from_path(path: &Path) -> Result<Project, LoadError> {
    let text = fs::read_to_string(path).map_err(LoadError::Read)?;
    load_from_str(&text)
}

fn syntax_error(error: &serde_json::Error) -> LoadError {
    LoadError::Syntax {
        line: error.line(),
        column: error.column(),
        message: error.to_string(),
    }
}

fn load_value(value: Value) -> Result<Project, LoadError> {
    if let Some(version) = newer_schema_version(&value) {
        return Err(LoadError::UnsupportedSchema { version });
    }
    let mut errors = Vec::new();
    validate_project(&value, &mut errors);
    if !errors.is_empty() {
        return Err(LoadError::Validation(errors));
    }
    serde_json::from_value(value).map_err(|error| {
        LoadError::Validation(vec![ValidationError {
            path: "$".to_owned(),
            message: format!("internal validation mismatch: {error}"),
        }])
    })
}

fn newer_schema_version(value: &Value) -> Option<u64> {
    let version = value.as_object()?.get("schema_version")?.as_number()?;
    let version = u64_number(version)?;
    (version > 1).then_some(version)
}

fn u64_number(number: &Number) -> Option<u64> {
    if number.is_u64() {
        number.as_u64()
    } else {
        None
    }
}

fn validate_project(value: &Value, errors: &mut Vec<ValidationError>) {
    let Some(object) = expect_object(value, "$", errors) else {
        return;
    };
    reject_unknown(object, "$", PROJECT_FIELDS, errors);
    require_schema_version(object, errors);
    require_slug(object, "id", "$", errors);
    require_text(object, "name", "$", errors);
    require_choice(object, "status", "$", PROJECT_STATUSES, errors);
    optional_string(object, "description", "$", errors);
    optional_string(object, "current_version", "$", errors);
    optional_tags(object, errors);
    optional_date(object, "updated", "$", errors);
    optional_links(object, errors);
    optional_stack(object, errors);
    optional_plan(object, errors);
    optional_issues(object, errors);
    optional_watch_items(object, errors);
}

fn require_schema_version(object: &Map<String, Value>, errors: &mut Vec<ValidationError>) {
    match object.get("schema_version") {
        None => push(errors, "schema_version", "is required"),
        Some(value) => {
            let is_one = value
                .as_number()
                .and_then(u64_number)
                .is_some_and(|version| version == 1);
            if !is_one {
                push(errors, "schema_version", "must be 1");
            }
        }
    }
}

fn optional_tags(object: &Map<String, Value>, errors: &mut Vec<ValidationError>) {
    let Some(value) = optional(object, "tags") else {
        return;
    };
    let Some(items) = expect_array(value, "tags", errors) else {
        return;
    };
    let mut seen = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let path = format!("tags[{index}]");
        let Some(tag) = expect_string(item, &path, errors) else {
            continue;
        };
        if seen.iter().any(|existing: &String| existing == tag) {
            push(errors, path, "duplicate tag");
        } else {
            seen.push(tag.to_owned());
        }
    }
}

fn optional_links(object: &Map<String, Value>, errors: &mut Vec<ValidationError>) {
    let Some(value) = optional(object, "links") else {
        return;
    };
    let Some(items) = expect_array(value, "links", errors) else {
        return;
    };
    for (index, item) in items.iter().enumerate() {
        let path = format!("links[{index}]");
        let Some(link) = expect_object(item, &path, errors) else {
            continue;
        };
        reject_unknown(link, &path, LINK_FIELDS, errors);
        require_string(link, "label", &path, errors);
        require_url(link, "url", &path, errors);
    }
}

fn optional_stack(object: &Map<String, Value>, errors: &mut Vec<ValidationError>) {
    let Some(value) = optional(object, "stack") else {
        return;
    };
    let Some(stack) = expect_object(value, "stack", errors) else {
        return;
    };
    reject_unknown(stack, "stack", STACK_FIELDS, errors);
    match required(stack, "components", "stack", errors) {
        None => {}
        Some(value) => validate_components(value, errors),
    }
    match required(stack, "lockfiles", "stack", errors) {
        None => {}
        Some(value) => validate_lockfiles(value, errors),
    }
}

fn validate_components(value: &Value, errors: &mut Vec<ValidationError>) {
    let Some(items) = expect_array(value, "stack.components", errors) else {
        return;
    };
    for (index, item) in items.iter().enumerate() {
        let path = format!("stack.components[{index}]");
        let Some(component) = expect_object(item, &path, errors) else {
            continue;
        };
        reject_unknown(component, &path, COMPONENT_FIELDS, errors);
        require_choice(component, "kind", &path, COMPONENT_KINDS, errors);
        require_text(component, "name", &path, errors);
        optional_string(component, "version", &path, errors);
        optional_string(component, "note", &path, errors);
    }
}

fn validate_lockfiles(value: &Value, errors: &mut Vec<ValidationError>) {
    let Some(items) = expect_array(value, "stack.lockfiles", errors) else {
        return;
    };
    for (index, item) in items.iter().enumerate() {
        let path = format!("stack.lockfiles[{index}]");
        let Some(lockfile) = expect_string(item, &path, errors) else {
            continue;
        };
        validate_relative_path(lockfile, &path, errors);
    }
}

fn optional_plan(object: &Map<String, Value>, errors: &mut Vec<ValidationError>) {
    let Some(value) = optional(object, "plan") else {
        return;
    };
    let Some(plan) = expect_object(value, "plan", errors) else {
        return;
    };
    reject_unknown(plan, "plan", PLAN_FIELDS, errors);
    let Some(phases) = required(plan, "phases", "plan", errors) else {
        return;
    };
    validate_phases(phases, errors);
}

fn validate_phases(value: &Value, errors: &mut Vec<ValidationError>) {
    let Some(items) = expect_array(value, "plan.phases", errors) else {
        return;
    };
    let mut seen_ids = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let path = format!("plan.phases[{index}]");
        let Some(phase) = expect_object(item, &path, errors) else {
            continue;
        };
        reject_unknown(phase, &path, PHASE_FIELDS, errors);
        if let Some(id) = require_slug(phase, "id", &path, errors) {
            note_duplicate(
                &mut seen_ids,
                id,
                &format!("{path}.id"),
                "duplicate id",
                errors,
            );
        }
        require_text(phase, "title", &path, errors);
        optional_string(phase, "goal", &path, errors);
        if let Some(tasks) = required(phase, "tasks", &path, errors) {
            validate_tasks(tasks, &path, errors);
        }
    }
}

fn validate_tasks(value: &Value, phase_path: &str, errors: &mut Vec<ValidationError>) {
    let path = format!("{phase_path}.tasks");
    let Some(items) = expect_array(value, &path, errors) else {
        return;
    };
    let mut seen_ids = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let task_path = format!("{path}[{index}]");
        let Some(task) = expect_object(item, &task_path, errors) else {
            continue;
        };
        reject_unknown(task, &task_path, TASK_FIELDS, errors);
        if let Some(id) = require_slug(task, "id", &task_path, errors) {
            note_duplicate(
                &mut seen_ids,
                id,
                &format!("{task_path}.id"),
                "duplicate id",
                errors,
            );
        }
        require_text(task, "title", &task_path, errors);
        require_choice(task, "status", &task_path, TASK_STATUSES, errors);
        optional_date(task, "due", &task_path, errors);
        optional_string(task, "note", &task_path, errors);
    }
}

fn optional_issues(object: &Map<String, Value>, errors: &mut Vec<ValidationError>) {
    let Some(value) = optional(object, "issues") else {
        return;
    };
    let Some(items) = expect_array(value, "issues", errors) else {
        return;
    };
    let mut seen_ids = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let path = format!("issues[{index}]");
        let Some(issue) = expect_object(item, &path, errors) else {
            continue;
        };
        reject_unknown(issue, &path, ISSUE_FIELDS, errors);
        if let Some(id) = require_slug(issue, "id", &path, errors) {
            note_duplicate(
                &mut seen_ids,
                id,
                &format!("{path}.id"),
                "duplicate id",
                errors,
            );
        }
        require_text(issue, "title", &path, errors);
        require_choice(issue, "severity", &path, ISSUE_SEVERITIES, errors);
        require_choice(issue, "status", &path, ISSUE_STATUSES, errors);
        optional_url(issue, "url", &path, errors);
        optional_string(issue, "note", &path, errors);
    }
}

fn optional_watch_items(object: &Map<String, Value>, errors: &mut Vec<ValidationError>) {
    let Some(value) = optional(object, "watch_items") else {
        return;
    };
    let Some(items) = expect_array(value, "watch_items", errors) else {
        return;
    };
    let mut seen_ids = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let path = format!("watch_items[{index}]");
        let Some(item) = expect_object(item, &path, errors) else {
            continue;
        };
        reject_unknown(item, &path, WATCH_FIELDS, errors);
        if let Some(id) = require_slug(item, "id", &path, errors) {
            note_duplicate(
                &mut seen_ids,
                id,
                &format!("{path}.id"),
                "duplicate id",
                errors,
            );
        }
        require_choice(item, "kind", &path, WATCH_KINDS, errors);
        require_text(item, "title", &path, errors);
        require_date(item, "due", &path, errors);
        optional_string(item, "note", &path, errors);
    }
}

fn reject_unknown(
    object: &Map<String, Value>,
    parent: &str,
    allowed: &[&str],
    errors: &mut Vec<ValidationError>,
) {
    for key in object.keys() {
        if !allowed.contains(&key.as_str()) {
            push(errors, join(parent, key), "unknown field");
        }
    }
}

fn optional<'a>(object: &'a Map<String, Value>, key: &str) -> Option<&'a Value> {
    match object.get(key) {
        None | Some(Value::Null) => None,
        Some(value) => Some(value),
    }
}

fn required<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    parent: &str,
    errors: &mut Vec<ValidationError>,
) -> Option<&'a Value> {
    match object.get(key) {
        None | Some(Value::Null) => {
            push(errors, join(parent, key), "is required");
            None
        }
        Some(value) => Some(value),
    }
}

fn require_slug<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    parent: &str,
    errors: &mut Vec<ValidationError>,
) -> Option<&'a str> {
    let path = join(parent, key);
    let value = required(object, key, parent, errors)?;
    let text = expect_string(value, &path, errors)?;
    if !is_slug(text) {
        push(errors, path, slug_message());
    }
    Some(text)
}

fn require_text(
    object: &Map<String, Value>,
    key: &str,
    parent: &str,
    errors: &mut Vec<ValidationError>,
) {
    let path = join(parent, key);
    let Some(value) = required(object, key, parent, errors) else {
        return;
    };
    let Some(text) = expect_string(value, &path, errors) else {
        return;
    };
    if text.trim().is_empty() {
        push(errors, path, "must not be empty or whitespace");
    }
}

fn require_string(
    object: &Map<String, Value>,
    key: &str,
    parent: &str,
    errors: &mut Vec<ValidationError>,
) {
    let path = join(parent, key);
    let Some(value) = required(object, key, parent, errors) else {
        return;
    };
    let _ = expect_string(value, &path, errors);
}

fn optional_string(
    object: &Map<String, Value>,
    key: &str,
    parent: &str,
    errors: &mut Vec<ValidationError>,
) {
    let Some(value) = optional(object, key) else {
        return;
    };
    let _ = expect_string(value, &join(parent, key), errors);
}

fn require_choice(
    object: &Map<String, Value>,
    key: &str,
    parent: &str,
    choices: &[&str],
    errors: &mut Vec<ValidationError>,
) {
    let path = join(parent, key);
    let Some(value) = required(object, key, parent, errors) else {
        return;
    };
    let Some(text) = expect_string(value, &path, errors) else {
        return;
    };
    if !choices.contains(&text) {
        push(
            errors,
            path,
            format!("must be one of {}", choices.join(", ")),
        );
    }
}

fn require_url(
    object: &Map<String, Value>,
    key: &str,
    parent: &str,
    errors: &mut Vec<ValidationError>,
) {
    let path = join(parent, key);
    let Some(value) = required(object, key, parent, errors) else {
        return;
    };
    let Some(text) = expect_string(value, &path, errors) else {
        return;
    };
    if !is_http_url(text) {
        push(errors, path, "must start with http:// or https://");
    }
}

fn optional_url(
    object: &Map<String, Value>,
    key: &str,
    parent: &str,
    errors: &mut Vec<ValidationError>,
) {
    let Some(value) = optional(object, key) else {
        return;
    };
    let path = join(parent, key);
    let Some(text) = expect_string(value, &path, errors) else {
        return;
    };
    if !is_http_url(text) {
        push(errors, path, "must start with http:// or https://");
    }
}

fn require_date(
    object: &Map<String, Value>,
    key: &str,
    parent: &str,
    errors: &mut Vec<ValidationError>,
) {
    let path = join(parent, key);
    let Some(value) = required(object, key, parent, errors) else {
        return;
    };
    let Some(text) = expect_string(value, &path, errors) else {
        return;
    };
    check_date(text, &path, errors);
}

fn optional_date(
    object: &Map<String, Value>,
    key: &str,
    parent: &str,
    errors: &mut Vec<ValidationError>,
) {
    let Some(value) = optional(object, key) else {
        return;
    };
    let path = join(parent, key);
    let Some(text) = expect_string(value, &path, errors) else {
        return;
    };
    check_date(text, &path, errors);
}

fn check_date(text: &str, path: &str, errors: &mut Vec<ValidationError>) {
    match validate_date(text) {
        Ok(()) => {}
        Err(DateError::Format) => push(errors, path, "must use YYYY-MM-DD"),
        Err(DateError::Calendar) => push(errors, path, "is not a calendar date"),
    }
}

fn validate_relative_path(text: &str, path: &str, errors: &mut Vec<ValidationError>) {
    if text.is_empty() || is_absolute_path(text) {
        push(errors, path, "must be a relative path");
        return;
    }
    if text.split(['/', '\\']).any(|segment| segment == "..") {
        push(errors, path, "must not contain a '..' path segment");
    }
}

fn is_absolute_path(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes
        .first()
        .is_some_and(|byte| *byte == b'/' || *byte == b'\\')
    {
        return true;
    }
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn is_http_url(text: &str) -> bool {
    text.starts_with("http://") || text.starts_with("https://")
}

fn is_slug(text: &str) -> bool {
    let bytes = text.as_bytes();
    let length = bytes.len();
    if !(1..=64).contains(&length) {
        return false;
    }
    if bytes[0] == b'-' || bytes[length - 1] == b'-' {
        return false;
    }
    bytes
        .iter()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
}

fn slug_message() -> &'static str {
    "must be 1 to 64 characters from a-z, 0-9, and hyphen, and must not start or end with a hyphen"
}

fn note_duplicate(
    seen: &mut Vec<String>,
    id: &str,
    path: &str,
    message: &str,
    errors: &mut Vec<ValidationError>,
) {
    if seen.iter().any(|existing| existing == id) {
        push(errors, path, message);
    } else {
        seen.push(id.to_owned());
    }
}

fn expect_object<'a>(
    value: &'a Value,
    path: &str,
    errors: &mut Vec<ValidationError>,
) -> Option<&'a Map<String, Value>> {
    if let Value::Object(object) = value {
        Some(object)
    } else {
        push(errors, path, "must be an object");
        None
    }
}

fn expect_array<'a>(
    value: &'a Value,
    path: &str,
    errors: &mut Vec<ValidationError>,
) -> Option<&'a Vec<Value>> {
    if let Value::Array(items) = value {
        Some(items)
    } else {
        push(errors, path, "must be an array");
        None
    }
}

fn expect_string<'a>(
    value: &'a Value,
    path: &str,
    errors: &mut Vec<ValidationError>,
) -> Option<&'a str> {
    if let Value::String(text) = value {
        Some(text)
    } else {
        push(errors, path, "must be a string");
        None
    }
}

fn join(parent: &str, field: &str) -> String {
    if parent == "$" {
        field.to_owned()
    } else {
        format!("{parent}.{field}")
    }
}

fn push(errors: &mut Vec<ValidationError>, path: impl Into<String>, message: impl Into<String>) {
    errors.push(ValidationError {
        path: path.into(),
        message: message.into(),
    });
}
