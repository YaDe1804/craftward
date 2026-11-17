//! Storage path resolution, first-run setup, and relocation for the desktop app.
//!
//! Path order matches the CLI: command line (unused here), environment,
//! settings file, then platform default. Database moves use the store backup
//! API the same way CLI `settings set` does.

use std::fs;
use std::path::{Path, PathBuf};

use craftward_core::{
    DATABASE_FILE, DirLocation, PathOrigin, ProcessHost, Settings, SettingsOverrides, StorageKind,
    WorkspaceLocation, WorkspaceOrigin, complete_first_run, default_settings_file, is_first_run,
    load_settings, locate_cache_with, locate_data_with, locate_workspace_with, origin_label,
    platform_path, relocate_workspace_file, remove_relocated_file, save_settings,
};
use craftward_store::{check_database_integrity, copy_database, remove_database_files};

/// Resolved storage locations with origin labels for the Settings screen.
#[derive(Debug, Clone)]
pub struct StorageSnapshot {
    /// Absolute settings.json path when it could be built.
    pub settings_file: Option<PathBuf>,
    /// Whether settings.json exists.
    pub settings_present: bool,
    /// Workspace file location.
    pub workspace: Result<WorkspaceLocation, String>,
    /// Data directory.
    pub data: Result<DirLocation, String>,
    /// Cache directory.
    pub cache: Result<DirLocation, String>,
    /// Loaded settings document, when present and valid.
    pub settings: Option<Settings>,
}

impl StorageSnapshot {
    /// Loads settings and resolves all three storage paths.
    #[must_use]
    pub fn load() -> Self {
        let settings_file = default_settings_file(&ProcessHost).ok();
        let settings_present = settings_file.as_ref().is_some_and(|path| path.exists());
        let settings = match load_settings(&ProcessHost) {
            Ok(value) => value,
            Err(error) => {
                return Self {
                    settings_file,
                    settings_present,
                    workspace: Err(error.to_string()),
                    data: Err(error.to_string()),
                    cache: Err(error.to_string()),
                    settings: None,
                };
            }
        };
        let overrides = overrides_of(settings.as_ref());
        let workspace = locate_workspace_with(&ProcessHost, None, overrides.workspace)
            .map_err(|e| e.to_string());
        let data =
            locate_data_with(&ProcessHost, None, overrides.data_dir).map_err(|e| e.to_string());
        let cache =
            locate_cache_with(&ProcessHost, None, overrides.cache_dir).map_err(|e| e.to_string());
        Self {
            settings_file,
            settings_present,
            workspace,
            data,
            cache,
            settings,
        }
    }
}

fn overrides_of(settings: Option<&Settings>) -> SettingsOverrides<'_> {
    SettingsOverrides {
        workspace: settings.and_then(|value| value.workspace.as_deref()),
        data_dir: settings.and_then(|value| value.data_dir.as_deref()),
        cache_dir: settings.and_then(|value| value.cache_dir.as_deref()),
    }
}

/// Whether Craftward should show the first-run wizard.
#[must_use]
pub fn needs_onboarding() -> bool {
    is_first_run(&ProcessHost, None).unwrap_or(false)
}

/// Default paths suggested during onboarding (ignores settings; respects env).
#[derive(Debug, Clone)]
pub struct OnboardingDefaults {
    pub workspace: PathBuf,
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub workspace_env: bool,
    pub data_env: bool,
    pub cache_env: bool,
}

impl OnboardingDefaults {
    /// Builds platform defaults and notes which env vars are set.
    pub fn load() -> Result<Self, String> {
        let workspace_env =
            std::env::var_os("CRAFTWARD_WORKSPACE").is_some_and(|value| !value.is_empty());
        let data_env =
            std::env::var_os("CRAFTWARD_DATA_DIR").is_some_and(|value| !value.is_empty());
        let cache_env =
            std::env::var_os("CRAFTWARD_CACHE_DIR").is_some_and(|value| !value.is_empty());
        let workspace = locate_workspace_with(&ProcessHost, None, None)
            .map_err(|error| error.to_string())?
            .path;
        let data_dir = locate_data_with(&ProcessHost, None, None)
            .map_err(|error| error.to_string())?
            .path;
        let cache_dir = locate_cache_with(&ProcessHost, None, None)
            .map_err(|error| error.to_string())?
            .path;
        Ok(Self {
            workspace: absolute_or_as_is(&workspace),
            data_dir: absolute_or_as_is(&data_dir),
            cache_dir: absolute_or_as_is(&cache_dir),
            workspace_env,
            data_env,
            cache_env,
        })
    }
}

fn absolute_or_as_is(path: &Path) -> PathBuf {
    let path = platform_path(path);
    if path.is_absolute() {
        path
    } else if let Ok(cwd) = std::env::current_dir() {
        platform_path(&cwd.join(&path))
    } else {
        path
    }
}

/// Draft paths collected by the onboarding wizard.
#[derive(Debug, Clone)]
pub struct OnboardingDraft {
    pub workspace: String,
    pub data_dir: String,
    pub cache_dir: String,
}

impl OnboardingDraft {
    #[must_use]
    pub fn from_defaults(defaults: &OnboardingDefaults) -> Self {
        Self {
            workspace: defaults.workspace.display().to_string(),
            data_dir: defaults.data_dir.display().to_string(),
            cache_dir: defaults.cache_dir.display().to_string(),
        }
    }
}

/// Creates directories, an empty workspace, and settings.json.
pub fn finish_onboarding(draft: &OnboardingDraft) -> Result<(), String> {
    let settings = Settings {
        workspace: Some(absolute_or_as_is(Path::new(&draft.workspace))),
        data_dir: Some(absolute_or_as_is(Path::new(&draft.data_dir))),
        cache_dir: Some(absolute_or_as_is(Path::new(&draft.cache_dir))),
    };
    complete_first_run(&ProcessHost, &settings).map_err(|error| error.to_string())
}

/// Human label for a workspace origin.
#[must_use]
pub fn workspace_origin_label(origin: WorkspaceOrigin) -> &'static str {
    origin.label()
}

/// Human label for a data or cache origin.
#[must_use]
pub fn dir_origin_label(kind: StorageKind, origin: PathOrigin) -> &'static str {
    origin_label(kind, origin)
}

/// Whether a path may be changed from Settings.
#[must_use]
pub fn workspace_editable(origin: WorkspaceOrigin) -> bool {
    origin.to_path_origin().is_editable()
}

/// Whether a directory may be changed from Settings.
#[must_use]
pub fn dir_editable(origin: PathOrigin) -> bool {
    origin.is_editable()
}

/// Relocates the workspace file and updates settings. Refuses an occupied target.
pub fn relocate_workspace(
    current: &Path,
    target: &Path,
    settings: &mut Settings,
) -> Result<(), String> {
    let target = absolute_or_as_is(target);
    if current == target.as_path() {
        settings.workspace = Some(target);
        save_settings(&ProcessHost, settings).map_err(|error| error.to_string())?;
        return Ok(());
    }
    if target.exists() {
        return Err(format!("target already exists: {}", target.display()));
    }
    relocate_workspace_file(current, &target).map_err(|error| error.to_string())?;
    craftward_core::load_or_empty(&target).map_err(|error| error.to_string())?;
    let previous = settings.workspace.replace(target);
    if let Err(error) = save_settings(&ProcessHost, settings) {
        if let Some(path) = settings.workspace.take() {
            let _ = remove_relocated_file(&path);
        }
        settings.workspace = previous;
        return Err(error.to_string());
    }
    if current.exists() {
        remove_relocated_file(current).map_err(|error| error.to_string())?;
    }
    Ok(())
}

/// Relocates craftward.db via the backup API, then updates settings.
pub fn relocate_data_dir(
    current: &Path,
    target: &Path,
    settings: &mut Settings,
) -> Result<(), String> {
    let target = absolute_or_as_is(target);
    if current == target.as_path() {
        settings.data_dir = Some(target);
        save_settings(&ProcessHost, settings).map_err(|error| error.to_string())?;
        return Ok(());
    }
    fs::create_dir_all(&target).map_err(|error| error.to_string())?;
    let from_db = current.join(DATABASE_FILE);
    let to_db = target.join(DATABASE_FILE);
    if to_db.exists() {
        return Err(format!("target already exists: {}", to_db.display()));
    }
    if from_db.exists() {
        copy_database(&from_db, &to_db).map_err(|error| error.to_string())?;
        check_database_integrity(&to_db).map_err(|error| error.to_string())?;
    }
    let previous = settings.data_dir.replace(target);
    if let Err(error) = save_settings(&ProcessHost, settings) {
        if from_db.exists() {
            let _ = remove_database_files(&to_db);
        }
        settings.data_dir = previous;
        return Err(error.to_string());
    }
    if from_db.exists() {
        remove_database_files(&from_db).map_err(|error| error.to_string())?;
    }
    Ok(())
}

/// Creates a new cache directory and records it in settings (cache is not copied).
pub fn relocate_cache_dir(target: &Path, settings: &mut Settings) -> Result<(), String> {
    let target = absolute_or_as_is(target);
    fs::create_dir_all(&target).map_err(|error| error.to_string())?;
    settings.cache_dir = Some(target);
    save_settings(&ProcessHost, settings).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::needs_onboarding;
    use craftward_core::{Host, OsFamily, is_first_run};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct FakeHost {
        config: PathBuf,
        home: PathBuf,
    }

    impl Host for FakeHost {
        fn var(&self, name: &str) -> Option<String> {
            match name {
                "XDG_CONFIG_HOME" => Some(self.config.display().to_string()),
                "XDG_DATA_HOME" => Some(self.home.join("share").display().to_string()),
                "XDG_CACHE_HOME" => Some(self.home.join("cache").display().to_string()),
                _ => None,
            }
        }

        fn home_dir(&self) -> Option<PathBuf> {
            Some(self.home.clone())
        }

        fn os(&self) -> OsFamily {
            OsFamily::Linux
        }
    }

    fn scratch(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let root = std::env::temp_dir().join(format!("craftward-app-storage-{name}-{nanos}"));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn fake_host_first_run_matches_core() {
        let root = scratch("first");
        let host = FakeHost {
            config: root.join("config"),
            home: root.join("home"),
        };
        fs::create_dir_all(&host.config).unwrap();
        assert!(is_first_run(&host, None).unwrap());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn needs_onboarding_is_deterministic() {
        let _ = needs_onboarding();
    }
}
