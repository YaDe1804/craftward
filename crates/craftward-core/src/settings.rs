//! User settings file for storage locations.
//!
//! The file sits next to the platform default workspace directory as
//! `settings.json`. It is versioned JSON and rejected when it has unknown
//! fields or an unsupported schema version.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use crate::error::ValidationError;
use crate::fs_atomic::write_atomic;
use crate::json_check::{
    expect_object, expect_string, join, newer_schema_version, push, reject_unknown, u64_number,
};
use crate::location::{Host, LocateError, default_config_dir};

/// Current settings schema version.
pub const SETTINGS_SCHEMA_VERSION: u64 = 1;

/// File name of the settings document.
pub const SETTINGS_FILE: &str = "settings.json";

/// Optional path overrides stored in the settings file.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Settings {
    /// Absolute or relative path of `workspace.json`.
    pub workspace: Option<PathBuf>,
    /// Directory that holds `craftward.db`.
    pub data_dir: Option<PathBuf>,
    /// Directory for cached HTTP responses.
    pub cache_dir: Option<PathBuf>,
}

/// Failure while reading or writing settings.
#[derive(Debug)]
pub enum SettingsError {
    /// File could not be read.
    Read(std::io::Error),
    /// File could not be written.
    Write(std::io::Error),
    /// JSON was not valid.
    Syntax(String),
    /// Schema or field validation failed.
    Validation(Vec<ValidationError>),
    /// Default settings path could not be built.
    Locate(LocateError),
}

impl std::fmt::Display for SettingsError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read(error) => write!(formatter, "cannot read settings: {error}"),
            Self::Write(error) => write!(formatter, "cannot write settings: {error}"),
            Self::Syntax(message) => write!(formatter, "settings JSON error: {message}"),
            Self::Validation(errors) => {
                let joined = errors
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("; ");
                write!(formatter, "invalid settings: {joined}")
            }
            Self::Locate(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for SettingsError {}

/// Platform-default path of `settings.json` (next to the default workspace).
///
/// # Errors
///
/// Returns [`LocateError`] when the config directory cannot be built.
pub fn default_settings_file(host: &impl Host) -> Result<PathBuf, LocateError> {
    Ok(default_config_dir(host)?.join(SETTINGS_FILE))
}

/// Whether this looks like a first launch: no settings file and no workspace
/// file at the location resolved without settings (CLI → env → default).
///
/// # Errors
///
/// Returns [`LocateError`] when defaults cannot be built.
pub fn is_first_run(host: &impl Host, cli_workspace: Option<&Path>) -> Result<bool, LocateError> {
    let settings_path = default_settings_file(host)?;
    if settings_path.exists() {
        return Ok(false);
    }
    let workspace = crate::location::locate_workspace_with(host, cli_workspace, None)?;
    Ok(!workspace.path.exists())
}

/// Loads settings from the platform-default path.
///
/// A missing file is [`Ok`]`(`[`None`]`)`.
///
/// # Errors
///
/// Returns read, syntax, or validation errors, and [`SettingsError::Locate`]
/// when the default path cannot be built.
pub fn load_settings(host: &impl Host) -> Result<Option<Settings>, SettingsError> {
    let path = default_settings_file(host).map_err(SettingsError::Locate)?;
    load_settings_from(&path)
}

/// Loads settings from `path`.
///
/// A missing file is [`Ok`]`(`[`None`]`)`.
///
/// # Errors
///
/// Returns read, syntax, or validation errors.
pub fn load_settings_from(path: &Path) -> Result<Option<Settings>, SettingsError> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(parse_settings(&text)?)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(SettingsError::Read(error)),
    }
}

/// Writes settings to the platform-default path.
///
/// # Errors
///
/// Returns locate or write errors.
pub fn save_settings(host: &impl Host, settings: &Settings) -> Result<(), SettingsError> {
    let path = default_settings_file(host).map_err(SettingsError::Locate)?;
    save_settings_to(&path, settings)
}

/// Writes settings to `path` as pretty JSON with LF endings.
///
/// # Errors
///
/// Returns write errors.
pub fn save_settings_to(path: &Path, settings: &Settings) -> Result<(), SettingsError> {
    let text = format_settings(settings);
    write_atomic(path, text.as_bytes()).map_err(SettingsError::Write)
}

/// Parses and validates a settings document.
///
/// # Errors
///
/// Returns syntax or validation errors.
pub fn parse_settings(text: &str) -> Result<Settings, SettingsError> {
    let value: Value =
        serde_json::from_str(text).map_err(|error| SettingsError::Syntax(error.to_string()))?;
    let mut errors = Vec::new();
    let Some(object) = expect_object(&value, "$", &mut errors) else {
        return Err(SettingsError::Validation(errors));
    };
    if let Some(version) = newer_schema_version(&value) {
        push(
            &mut errors,
            "schema_version",
            format!("unsupported version {version} (supported: {SETTINGS_SCHEMA_VERSION})"),
        );
    }
    reject_unknown(
        object,
        "$",
        &["schema_version", "workspace", "data_dir", "cache_dir"],
        &mut errors,
    );
    match object.get("schema_version") {
        None | Some(Value::Null) => push(&mut errors, "schema_version", "is required"),
        Some(Value::Number(number)) => {
            if u64_number(number) != Some(SETTINGS_SCHEMA_VERSION)
                && errors.iter().all(|error| {
                    error.path != "schema_version" || !error.message.starts_with("unsupported")
                })
            {
                push(
                    &mut errors,
                    "schema_version",
                    format!("must be {SETTINGS_SCHEMA_VERSION}"),
                );
            }
        }
        Some(_) => push(&mut errors, "schema_version", "must be a number"),
    }
    let workspace = optional_path(object, "workspace", &mut errors);
    let data_dir = optional_path(object, "data_dir", &mut errors);
    let cache_dir = optional_path(object, "cache_dir", &mut errors);
    if !errors.is_empty() {
        return Err(SettingsError::Validation(errors));
    }
    Ok(Settings {
        workspace,
        data_dir,
        cache_dir,
    })
}

fn optional_path(
    object: &Map<String, Value>,
    key: &str,
    errors: &mut Vec<ValidationError>,
) -> Option<PathBuf> {
    let value = object.get(key)?;
    let path = join("$", key);
    let text = expect_string(value, &path, errors)?;
    if text.is_empty() {
        push(errors, path, "must not be empty");
        return None;
    }
    Some(PathBuf::from(text))
}

fn format_settings(settings: &Settings) -> String {
    let mut object = serde_json::Map::new();
    object.insert(
        "schema_version".to_owned(),
        Value::from(SETTINGS_SCHEMA_VERSION),
    );
    if let Some(path) = &settings.workspace {
        object.insert("workspace".to_owned(), Value::String(path_to_string(path)));
    }
    if let Some(path) = &settings.data_dir {
        object.insert("data_dir".to_owned(), Value::String(path_to_string(path)));
    }
    if let Some(path) = &settings.cache_dir {
        object.insert("cache_dir".to_owned(), Value::String(path_to_string(path)));
    }
    let mut text = serde_json::to_string_pretty(&Value::Object(object))
        .unwrap_or_else(|_| format!("{{\n  \"schema_version\": {SETTINGS_SCHEMA_VERSION}\n}}"));
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// Copies a workspace file to a new path without overwriting an existing file.
///
/// When `from` is missing, writes an empty workspace at `to`.
///
/// # Errors
///
/// Returns an error when the target exists or the copy fails.
pub fn relocate_workspace_file(from: &Path, to: &Path) -> Result<(), SettingsError> {
    if to.exists() {
        return Err(SettingsError::Write(std::io::Error::new(
            ErrorKind::AlreadyExists,
            format!("target already exists: {}", to.display()),
        )));
    }
    if let Some(parent) = to.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(SettingsError::Write)?;
    }
    if from.exists() {
        fs::copy(from, to).map_err(SettingsError::Write)?;
    } else {
        crate::workspace::save_workspace(to, &crate::workspace::Workspace::empty())
            .map_err(|error| SettingsError::Write(std::io::Error::other(error.to_string())))?;
    }
    Ok(())
}

/// Removes a relocated source file after a successful move.
///
/// # Errors
///
/// Returns a write error when removal fails.
pub fn remove_relocated_file(path: &Path) -> Result<(), SettingsError> {
    if path.exists() {
        fs::remove_file(path).map_err(SettingsError::Write)?;
    }
    Ok(())
}

/// Finishes first-run setup: creates data and cache directories, an empty
/// workspace file when missing, and writes `settings.json`.
///
/// `settings.workspace`, `data_dir`, and `cache_dir` should already hold the
/// chosen absolute paths. Missing optional fields fall back to the platform
/// defaults for directory creation and the workspace file.
///
/// # Errors
///
/// Returns locate or write errors. Does not remove a partial setup on failure;
/// the caller may leave the machine without a settings file so first-run runs
/// again.
pub fn complete_first_run(host: &impl Host, settings: &Settings) -> Result<(), SettingsError> {
    let data = match &settings.data_dir {
        Some(path) => path.clone(),
        None => crate::location::default_data_dir(host).map_err(SettingsError::Locate)?,
    };
    let cache = match &settings.cache_dir {
        Some(path) => path.clone(),
        None => crate::location::default_cache_dir(host).map_err(SettingsError::Locate)?,
    };
    let workspace = match &settings.workspace {
        Some(path) => path.clone(),
        None => crate::location::default_workspace_file(host).map_err(SettingsError::Locate)?,
    };
    fs::create_dir_all(&data).map_err(SettingsError::Write)?;
    fs::create_dir_all(&cache).map_err(SettingsError::Write)?;
    if !workspace.exists() {
        relocate_workspace_file(Path::new(""), &workspace)?;
    }
    let stored = Settings {
        workspace: Some(workspace),
        data_dir: Some(data),
        cache_dir: Some(cache),
    };
    save_settings(host, &stored)
}

#[cfg(test)]
mod tests {
    use super::{SETTINGS_SCHEMA_VERSION, parse_settings};

    #[test]
    fn parse_accepts_version_one_with_optional_paths() {
        let settings = parse_settings(
            "{\n  \"schema_version\": 1,\n  \"workspace\": \"/tmp/w.json\",\n  \"data_dir\": \"/tmp/data\"\n}\n",
        )
        .expect("parse");
        assert_eq!(
            settings.workspace.as_deref(),
            Some(std::path::Path::new("/tmp/w.json"))
        );
        assert_eq!(
            settings.data_dir.as_deref(),
            Some(std::path::Path::new("/tmp/data"))
        );
        assert!(settings.cache_dir.is_none());
    }

    #[test]
    fn parse_rejects_unknown_fields_and_bad_version() {
        let unknown = parse_settings("{\"schema_version\": 1, \"theme\": \"dark\"}\n");
        assert!(unknown.is_err(), "{unknown:?}");
        let version = parse_settings(&format!(
            "{{\n  \"schema_version\": {}\n}}\n",
            SETTINGS_SCHEMA_VERSION + 1
        ));
        assert!(version.is_err(), "{version:?}");
    }

    #[test]
    fn parse_rejects_empty_path_strings() {
        let empty = parse_settings("{\n  \"schema_version\": 1,\n  \"workspace\": \"\"\n}\n");
        assert!(empty.is_err(), "{empty:?}");
    }
}
