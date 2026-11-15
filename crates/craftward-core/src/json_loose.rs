//! Lenient deserializers for foreign JSON.
//!
//! Serde's `#[serde(default)]` and `Option<T>` cover missing fields. They do
//! not cover a field that is present with the wrong JSON type: that fails the
//! whole value. OSV and endoflife.date sometimes send a number or object where
//! a string or array is documented. The helpers below keep the previous
//! Value-based behaviour: a wrong type is treated as absent or empty, and a
//! list item that does not match is skipped.

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
    Ok(items(Value::deserialize(deserializer)?))
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
    Ok(items(value))
}

/// `null` or a non-array becomes `None`. An array skips unusable items.
pub(crate) fn optional_lenient_vec<'de, D, T>(deserializer: D) -> Result<Option<Vec<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    let value = Value::deserialize(deserializer)?;
    Ok(match value {
        Value::Array(_) => Some(items(value)),
        _ => None,
    })
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

/// Like [`optional_string`], but a missing or wrong value becomes `""`.
pub(crate) fn string_or_empty<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(optional_string(deserializer)?.unwrap_or_default())
}

/// A boolean, or `None` when the value is missing, null, or another type.
pub(crate) fn optional_bool<'de, D>(deserializer: D) -> Result<Option<bool>, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Value::deserialize(deserializer)?.as_bool())
}

fn items<T: DeserializeOwned>(value: Value) -> Vec<T> {
    let Value::Array(items) = value else {
        return Vec::new();
    };
    items
        .into_iter()
        .filter_map(|item| serde_json::from_value(item).ok())
        .collect()
}
