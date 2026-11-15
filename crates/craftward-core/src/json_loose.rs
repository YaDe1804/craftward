//! Lenient and strict deserializers for foreign JSON.
//!
//! Serde's `#[serde(default)]` and `Option<T>` cover missing fields. They do
//! not cover a field that is present with the wrong JSON type: that fails the
//! whole value. Display-only fields may stay lenient (wrong type becomes
//! absent or empty, bad list items are skipped). Fields that decide whether a
//! finding is raised or suppressed must be strict.

use serde::Deserialize;
use serde::de::{DeserializeOwned, Deserializer, Error};
use serde_json::Value;

/// A list that may be missing, the wrong type, or contain unusable items.
///
/// Missing or a non-array becomes an empty `Vec`. Bad items are skipped.
pub(crate) fn lenient_vec<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    Ok(items_lenient(Value::deserialize(deserializer)?))
}

/// A list that must be a JSON array; unusable items are still skipped.
pub(crate) fn lenient_array<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    let value = Value::deserialize(deserializer)?;
    let Value::Array(_) = &value else {
        return Err(Error::custom("expected array"));
    };
    Ok(items_lenient(value))
}

/// A list that must be a JSON array; every item must deserialize.
pub(crate) fn strict_vec<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    let value = Value::deserialize(deserializer)?;
    let Value::Array(items) = value else {
        return Err(Error::custom("expected array"));
    };
    items
        .into_iter()
        .map(|item| serde_json::from_value(item).map_err(Error::custom))
        .collect()
}

/// `null` becomes `None`. An array requires every item. Any other type fails.
pub(crate) fn optional_strict_vec<'de, D, T>(deserializer: D) -> Result<Option<Vec<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    let value = Value::deserialize(deserializer)?;
    match value {
        Value::Null => Ok(None),
        Value::Array(items) => items
            .into_iter()
            .map(|item| serde_json::from_value(item).map_err(Error::custom))
            .collect::<Result<Vec<_>, _>>()
            .map(Some),
        _ => Err(Error::custom("expected array")),
    }
}

/// A string, or `None` when the value is missing, null, or another type.
pub(crate) fn optional_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(match Value::deserialize(deserializer)? {
        Value::String(text) => Some(text),
        _ => None,
    })
}

/// Like [`optional_string`], but a wrong type fails instead of becoming `None`.
pub(crate) fn optional_strict_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    match Value::deserialize(deserializer)? {
        Value::Null => Ok(None),
        Value::String(text) => Ok(Some(text)),
        _ => Err(Error::custom("expected string")),
    }
}

/// Like [`optional_string`], but a missing or wrong value becomes `""`.
pub(crate) fn string_or_empty<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(optional_string(deserializer)?.unwrap_or_default())
}

/// A boolean, or `None` for null. Any other type fails.
pub(crate) fn optional_strict_bool<'de, D>(deserializer: D) -> Result<Option<bool>, D::Error>
where
    D: Deserializer<'de>,
{
    match Value::deserialize(deserializer)? {
        Value::Null => Ok(None),
        Value::Bool(flag) => Ok(Some(flag)),
        _ => Err(Error::custom("expected boolean")),
    }
}

fn items_lenient<T: DeserializeOwned>(value: Value) -> Vec<T> {
    let Value::Array(items) = value else {
        return Vec::new();
    };
    items
        .into_iter()
        .filter_map(|item| serde_json::from_value(item).ok())
        .collect()
}
