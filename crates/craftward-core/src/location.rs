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

/// Why a storage path was selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathOrigin {
    /// A command-line option (`--workspace`, `--data-dir`, or `--cache-dir`).
    CommandLine,
    /// An environment variable.
    Environment,
    /// The settings file.
    Settings,
    /// The suggested directory for this operating system.
    Default,
}

impl PathOrigin {
    /// A short English label for UI and command output.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::CommandLine => "command-line option",
            Self::Environment => "environment variable",
            Self::Settings => "settings file",
            Self::Default => "default location",
        }
    }

    /// Whether this origin can be changed from Settings.
    #[must_use]
    pub const fn is_editable(self) -> bool {
        matches!(self, Self::Settings | Self::Default)
    }
}

/// Why a workspace path was selected.
///
/// Prefer [`PathOrigin`] for new code. This type keeps the older labels that
/// name `CRAFTWARD_WORKSPACE` explicitly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceOrigin {
    /// The `--workspace` option.
    Option,
    /// The `CRAFTWARD_WORKSPACE` environment variable.
    Environment,
    /// The settings file.
    Settings,
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
            Self::Settings => "settings file",
            Self::Default => "default location",
        }
    }

    /// Converts to the shared [`PathOrigin`].
    #[must_use]
    pub const fn to_path_origin(self) -> PathOrigin {
        match self {
            Self::Option => PathOrigin::CommandLine,
            Self::Environment => PathOrigin::Environment,
            Self::Settings => PathOrigin::Settings,
            Self::Default => PathOrigin::Default,
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

/// A directory path and the reason it was chosen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirLocation {
    /// Resolved directory path.
    pub path: PathBuf,
    /// Which rule selected [`Self::path`].
    pub origin: PathOrigin,
}

/// Optional path overrides from the settings file.
#[derive(Debug, Clone, Copy, Default)]
pub struct SettingsOverrides<'a> {
    /// `workspace` field from settings.
    pub workspace: Option<&'a Path>,
    /// `data_dir` field from settings.
    pub data_dir: Option<&'a Path>,
    /// `cache_dir` field from settings.
    pub cache_dir: Option<&'a Path>,
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
    locate_workspace_with(host, explicit, None)
}

/// Chooses the workspace file, including an optional settings override.
///
/// Order: command line → `CRAFTWARD_WORKSPACE` → settings → default.
///
/// # Errors
///
/// Returns [`LocateError`] when the default path cannot be built.
pub fn locate_workspace_with(
    host: &impl Host,
    explicit: Option<&Path>,
    from_settings: Option<&Path>,
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
    if let Some(path) = from_settings {
        return Ok(WorkspaceLocation {
            path: path.to_path_buf(),
            origin: WorkspaceOrigin::Settings,
        });
    }
    Ok(WorkspaceLocation {
        path: default_workspace_file(host)?,
        origin: WorkspaceOrigin::Default,
    })
}

/// Platform config directory that holds the default `workspace.json`.
///
/// # Errors
///
/// Returns [`LocateError`] when the directory cannot be built.
pub fn default_config_dir(host: &impl Host) -> Result<PathBuf, LocateError> {
    match host.os() {
        OsFamily::Windows => {
            let app_data = non_empty(host.var("APPDATA")).ok_or(LocateError::NoAppData)?;
            Ok(PathBuf::from(app_data).join("Craftward"))
        }
        OsFamily::Linux => {
            if let Some(config_home) = absolute_xdg(host, "XDG_CONFIG_HOME") {
                Ok(PathBuf::from(config_home).join("craftward"))
            } else {
                Ok(host
                    .home_dir()
                    .ok_or(LocateError::NoHome)?
                    .join(".config")
                    .join("craftward"))
            }
        }
        OsFamily::MacOs => Ok(host
            .home_dir()
            .ok_or(LocateError::NoHome)?
            .join("Library")
            .join("Application Support")
            .join("Craftward")),
    }
}

/// Platform-default workspace file path (ignores env and settings).
///
/// # Errors
///
/// Returns [`LocateError`] when the path cannot be built.
pub fn default_workspace_file(host: &impl Host) -> Result<PathBuf, LocateError> {
    Ok(default_config_dir(host)?.join(WORKSPACE_FILE))
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
    Ok(locate_cache_with(host, explicit, None)?.path)
}

/// Chooses the cache directory, including an optional settings override.
///
/// Order: command line → `CRAFTWARD_CACHE_DIR` → settings → default.
///
/// # Errors
///
/// Returns [`LocateError`] when the default path cannot be built.
pub fn locate_cache_with(
    host: &impl Host,
    explicit: Option<&Path>,
    from_settings: Option<&Path>,
) -> Result<DirLocation, LocateError> {
    if let Some(path) = explicit {
        return Ok(DirLocation {
            path: path.to_path_buf(),
            origin: PathOrigin::CommandLine,
        });
    }
    if let Some(path) = non_empty(host.var(ENV_CACHE)) {
        return Ok(DirLocation {
            path: PathBuf::from(path),
            origin: PathOrigin::Environment,
        });
    }
    if let Some(path) = from_settings {
        return Ok(DirLocation {
            path: path.to_path_buf(),
            origin: PathOrigin::Settings,
        });
    }
    Ok(DirLocation {
        path: default_cache_dir(host)?,
        origin: PathOrigin::Default,
    })
}

/// Platform-default cache directory (ignores env and settings).
///
/// # Errors
///
/// Returns [`LocateError`] when the directory cannot be built.
pub fn default_cache_dir(host: &impl Host) -> Result<PathBuf, LocateError> {
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
    Ok(locate_data_with(host, explicit, None)?.path)
}

/// Chooses the data directory, including an optional settings override.
///
/// Order: command line → `CRAFTWARD_DATA_DIR` → settings → default.
///
/// # Errors
///
/// Returns [`LocateError`] when the default path cannot be built.
pub fn locate_data_with(
    host: &impl Host,
    explicit: Option<&Path>,
    from_settings: Option<&Path>,
) -> Result<DirLocation, LocateError> {
    if let Some(path) = explicit {
        return Ok(DirLocation {
            path: path.to_path_buf(),
            origin: PathOrigin::CommandLine,
        });
    }
    if let Some(path) = non_empty(host.var(ENV_DATA)) {
        return Ok(DirLocation {
            path: PathBuf::from(path),
            origin: PathOrigin::Environment,
        });
    }
    if let Some(path) = from_settings {
        return Ok(DirLocation {
            path: path.to_path_buf(),
            origin: PathOrigin::Settings,
        });
    }
    Ok(DirLocation {
        path: default_data_dir(host)?,
        origin: PathOrigin::Default,
    })
}

/// Resolves workspace, data, and cache together from CLI, env, settings, defaults.
///
/// # Errors
///
/// Returns [`LocateError`] when a default path cannot be built.
pub fn resolve_storage(
    host: &impl Host,
    cli_workspace: Option<&Path>,
    cli_data: Option<&Path>,
    cli_cache: Option<&Path>,
    settings: SettingsOverrides<'_>,
) -> Result<(WorkspaceLocation, DirLocation, DirLocation), LocateError> {
    let workspace = locate_workspace_with(host, cli_workspace, settings.workspace)?;
    let data = locate_data_with(host, cli_data, settings.data_dir)?;
    let cache = locate_cache_with(host, cli_cache, settings.cache_dir)?;
    Ok((workspace, data, cache))
}

/// English label for a data-dir or cache-dir environment origin.
#[must_use]
pub fn environment_label_for(kind: StorageKind) -> &'static str {
    match kind {
        StorageKind::Workspace => "environment variable CRAFTWARD_WORKSPACE",
        StorageKind::Data => "environment variable CRAFTWARD_DATA_DIR",
        StorageKind::Cache => "environment variable CRAFTWARD_CACHE_DIR",
    }
}

/// Which storage location is being described.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageKind {
    /// The workspace file.
    Workspace,
    /// The data directory.
    Data,
    /// The cache directory.
    Cache,
}

/// Human origin label for a resolved path, naming the env var when needed.
#[must_use]
pub fn origin_label(kind: StorageKind, origin: PathOrigin) -> &'static str {
    match origin {
        PathOrigin::CommandLine => "command-line option",
        PathOrigin::Environment => environment_label_for(kind),
        PathOrigin::Settings => "settings file",
        PathOrigin::Default => "default location",
    }
}

/// Platform-default data directory (ignores env and settings).
///
/// # Errors
///
/// Returns [`LocateError`] when the directory cannot be built.
pub fn default_data_dir(host: &impl Host) -> Result<PathBuf, LocateError> {
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
