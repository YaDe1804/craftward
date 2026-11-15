//! Shared checks for the hand-written project and workspace validators.
//!
//! Those validators collect every problem with a path. They stay hand-written
//! for that reason. The helpers below are the part both of them used to copy.

use serde_json::{Map, Number, Value};

use crate::error::ValidationError;

pub(crate) fn newer_schema_version(value: &Value) -> Option<u64> {
    let version = value.as_object()?.get("schema_version")?.as_number()?;
    let version = u64_number(version)?;
    (version > 1).then_some(version)
}

pub(crate) fn u64_number(number: &Number) -> Option<u64> {
    if number.is_u64() {
        number.as_u64()
    } else {
        None
    }
}

pub(crate) fn reject_unknown(
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

pub(crate) fn expect_object<'a>(
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

pub(crate) fn expect_array<'a>(
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

pub(crate) fn expect_string<'a>(
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

pub(crate) fn join(parent: &str, field: &str) -> String {
    if parent == "$" {
        field.to_owned()
    } else {
        format!("{parent}.{field}")
    }
}

pub(crate) fn push(
    errors: &mut Vec<ValidationError>,
    path: impl Into<String>,
    message: impl Into<String>,
) {
    errors.push(ValidationError {
        path: path.into(),
        message: message.into(),
    });
}
