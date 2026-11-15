//! Where the workspace file is found.
//!
//! Nothing here is a fixed installation path. Callers pass the command-line
//! path and a [`Host`] that supplies environment variables, the home directory,
//! and the operating system. Tests use their own host.

use std::env;
use std::path::{Path, PathBuf};

/// Operating system rules for the suggested workspace directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OsFamily {
    /// `%APPDATA%\Craftward`.
    Windows,
    /// Absolute `$XDG_CONFIG_HOME/craftward`, or `~/.config/craftward` when unset or relative.
    Linux,
    /// `~/Library/Application Support/Craftward`.
    MacOs,
}

/// Facts about the machine, isolated so tests do not read the real environment.
pub trait Host {
    /// Returns an environment variable when it is set.
    ///
    /// An empty value is still returned. Callers decide whether empty means unset.
    fn var(&self, name: &str) -> Option<String>;

    /// Returns the current user's home directory when it is known.
    fn home_dir(&self) -> Option<PathBuf>;

    /// Returns which default-directory rule to apply.
    fn os(&self) -> OsFamily;
}

/// The process environment.
///
/// [`Host::home_dir`] uses [`std::env::home_dir`]. As of Rust 1.100 that function
/// is not deprecated: on Unix it reads `HOME`, and on Windows it reads
/// `USERPROFILE` rather than the non-standard `HOME` variable. Windows
/// configuration for Craftward does not use it; it uses `APPDATA`.
#[derive(Debug, Clone, Copy)]
pub struct ProcessHost;

impl Host for ProcessHost {
    fn var(&self, name: &str) -> Option<String> {
        env::var(name).ok()
    }

    fn home_dir(&self) -> Option<PathBuf> {
        env::home_dir()
    }

    fn os(&self) -> OsFamily {
        if cfg!(windows) {
            OsFamily::Windows
        } else if cfg!(target_os = "macos") {
            OsFamily::MacOs
        } else {
            OsFamily::Linux
        }
    }
}

/// Why a workspace path was selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceOrigin {
    /// The `--workspace` option.
    Option,
    /// The `CRAFTWARD_WORKSPACE` environment variable.
    Environment,
    /// The suggested directory for this operating system.
    Default,
}

impl WorkspaceOrigin {
    /// A short English label for command output.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Option => "command-line option",
            Self::Environment => "environment variable CRAFTWARD_WORKSPACE",
            Self::Default => "default location",
        }
    }
}

/// A workspace file and the reason it was chosen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceLocation {
    /// Path of `workspace.json`. It may be relative when it came from the
    /// command line or the environment variable.
    pub path: PathBuf,
    /// Which rule selected [`Self::path`].
    pub origin: WorkspaceOrigin,
}

/// Failure to suggest a default workspace path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocateError {
    /// Windows has no `APPDATA` value.
    NoAppData,
    /// Linux or macOS has no home directory, and Linux has no `XDG_CONFIG_HOME`.
    NoHome,
    /// Windows has no `LOCALAPPDATA` value.
    NoLocalAppData,
    /// Linux or macOS has no home directory, and Linux has no `XDG_CACHE_HOME`.
    NoCacheHome,
    /// Windows has no `LOCALAPPDATA` value for the data directory.
    NoLocalData,
    /// Linux or macOS has no home directory for the data directory.
    NoDataHome,
}

impl std::fmt::Display for LocateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoAppData => {
                formatter.write_str("APPDATA is not set, so there is no default workspace location")
            }
            Self::NoHome => formatter.write_str(
                "the home directory is not known, so there is no default workspace location",
            ),
            Self::NoLocalAppData => formatter
                .write_str("LOCALAPPDATA is not set, so there is no default cache location"),
            Self::NoCacheHome => formatter.write_str(
                "the home directory is not known, so there is no default cache location",
            ),
            Self::NoLocalData => {
                formatter.write_str("LOCALAPPDATA is not set, so there is no default data location")
            }
            Self::NoDataHome => formatter
                .write_str("the home directory is not known, so there is no default data location"),
        }
    }
}

impl std::error::Error for LocateError {}

const WORKSPACE_FILE: &str = "workspace.json";
const ENV_WORKSPACE: &str = "CRAFTWARD_WORKSPACE";
const ENV_CACHE: &str = "CRAFTWARD_CACHE_DIR";
const ENV_DATA: &str = "CRAFTWARD_DATA_DIR";

/// Chooses the workspace file.
///
/// The order is the explicit path, then `CRAFTWARD_WORKSPACE`, then the
/// suggested directory. An empty environment variable is treated as unset.
///
/// # Errors
///
/// Returns [`LocateError`] when no explicit path and no environment variable
/// are set and the suggested directory cannot be built.
pub fn locate_workspace(
    host: &impl Host,
    explicit: Option<&Path>,
) -> Result<WorkspaceLocation, LocateError> {
    if let Some(path) = explicit {
        return Ok(WorkspaceLocation {
            path: path.to_path_buf(),
            origin: WorkspaceOrigin::Option,
        });
    }
    if let Some(path) = non_empty(host.var(ENV_WORKSPACE)) {
        return Ok(WorkspaceLocation {
            path: PathBuf::from(path),
            origin: WorkspaceOrigin::Environment,
        });
    }
    Ok(WorkspaceLocation {
        path: default_workspace_file(host)?,
        origin: WorkspaceOrigin::Default,
    })
}

fn default_workspace_file(host: &impl Host) -> Result<PathBuf, LocateError> {
    let directory = match host.os() {
        OsFamily::Windows => {
            let app_data = non_empty(host.var("APPDATA")).ok_or(LocateError::NoAppData)?;
            PathBuf::from(app_data).join("Craftward")
        }
        OsFamily::Linux => {
            if let Some(config_home) = absolute_xdg(host, "XDG_CONFIG_HOME") {
                PathBuf::from(config_home).join("craftward")
            } else {
                host.home_dir()
                    .ok_or(LocateError::NoHome)?
                    .join(".config")
                    .join("craftward")
            }
        }
        OsFamily::MacOs => host
            .home_dir()
            .ok_or(LocateError::NoHome)?
            .join("Library")
            .join("Application Support")
            .join("Craftward"),
    };
    Ok(directory.join(WORKSPACE_FILE))
}

/// Chooses the directory that stores cached OSV responses.
///
/// The order is the explicit path, then `CRAFTWARD_CACHE_DIR`, then the
/// suggested directory. An empty environment variable is treated as unset.
///
/// # Errors
///
/// Returns [`LocateError`] when no explicit path and no environment variable
/// are set and the suggested directory cannot be built.
pub fn locate_cache(host: &impl Host, explicit: Option<&Path>) -> Result<PathBuf, LocateError> {
    if let Some(path) = explicit {
        return Ok(path.to_path_buf());
    }
    if let Some(path) = non_empty(host.var(ENV_CACHE)) {
        return Ok(PathBuf::from(path));
    }
    default_cache_dir(host)
}

fn default_cache_dir(host: &impl Host) -> Result<PathBuf, LocateError> {
    let directory = match host.os() {
        OsFamily::Windows => {
            let local = non_empty(host.var("LOCALAPPDATA")).ok_or(LocateError::NoLocalAppData)?;
            PathBuf::from(local).join("Craftward").join("cache")
        }
        OsFamily::Linux => {
            if let Some(cache_home) = absolute_xdg(host, "XDG_CACHE_HOME") {
                PathBuf::from(cache_home).join("craftward")
            } else {
                host.home_dir()
                    .ok_or(LocateError::NoCacheHome)?
                    .join(".cache")
                    .join("craftward")
            }
        }
        OsFamily::MacOs => host
            .home_dir()
            .ok_or(LocateError::NoCacheHome)?
            .join("Library")
            .join("Caches")
            .join("Craftward"),
    };
    Ok(directory)
}

/// Chooses the directory that stores `craftward.db`.
///
/// The order is the explicit path, then `CRAFTWARD_DATA_DIR`, then the
/// suggested directory. An empty environment variable is treated as unset.
///
/// # Errors
///
/// Returns [`LocateError`] when no explicit path and no environment variable
/// are set and the suggested directory cannot be built.
pub fn locate_data(host: &impl Host, explicit: Option<&Path>) -> Result<PathBuf, LocateError> {
    if let Some(path) = explicit {
        return Ok(path.to_path_buf());
    }
    if let Some(path) = non_empty(host.var(ENV_DATA)) {
        return Ok(PathBuf::from(path));
    }
    default_data_dir(host)
}

fn default_data_dir(host: &impl Host) -> Result<PathBuf, LocateError> {
    let directory = match host.os() {
        OsFamily::Windows => {
            let local = non_empty(host.var("LOCALAPPDATA")).ok_or(LocateError::NoLocalData)?;
            PathBuf::from(local).join("Craftward")
        }
        OsFamily::Linux => {
            if let Some(data_home) = absolute_xdg(host, "XDG_DATA_HOME") {
                PathBuf::from(data_home).join("craftward")
            } else {
                host.home_dir()
                    .ok_or(LocateError::NoDataHome)?
                    .join(".local")
                    .join("share")
                    .join("craftward")
            }
        }
        OsFamily::MacOs => host
            .home_dir()
            .ok_or(LocateError::NoDataHome)?
            .join("Library")
            .join("Application Support")
            .join("Craftward"),
    };
    Ok(directory)
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|text| !text.is_empty())
}

/// XDG Base Directory values must be absolute; a relative value is ignored.
fn absolute_xdg(host: &impl Host, name: &str) -> Option<String> {
    let value = non_empty(host.var(name))?;
    let absolute = match host.os() {
        OsFamily::Windows => Path::new(&value).is_absolute(),
        OsFamily::Linux | OsFamily::MacOs => value.starts_with('/'),
    };
    absolute.then_some(value)
}
