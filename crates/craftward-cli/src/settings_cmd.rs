//! `craftward settings` — show and change storage locations.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use craftward_core::{
    DATABASE_FILE, DirLocation, ProcessHost, Settings, SettingsOverrides, StorageKind,
    WorkspaceLocation, default_settings_file, load_settings, locate_cache_with, locate_data_with,
    locate_workspace_with, origin_label, platform_path, relocate_workspace_file,
    remove_relocated_file, save_settings,
};
use craftward_store::{check_database_integrity, copy_database, remove_database_files};

use crate::{Failure, absolute_from_cwd};

/// Loaded settings kept alive for path resolution.
pub(crate) struct LoadedSettings {
    settings: Option<Settings>,
}

impl LoadedSettings {
    pub(crate) fn load() -> Result<Self, Failure> {
        match load_settings(&ProcessHost) {
            Ok(settings) => Ok(Self { settings }),
            Err(error) => Err(Failure::Data(error.to_string())),
        }
    }

    pub(crate) fn overrides(&self) -> SettingsOverrides<'_> {
        SettingsOverrides {
            workspace: self
                .settings
                .as_ref()
                .and_then(|settings| settings.workspace.as_deref()),
            data_dir: self
                .settings
                .as_ref()
                .and_then(|settings| settings.data_dir.as_deref()),
            cache_dir: self
                .settings
                .as_ref()
                .and_then(|settings| settings.cache_dir.as_deref()),
        }
    }
}

enum Cleanup {
    Workspace { path: PathBuf },
    Database { path: PathBuf },
}

pub(crate) fn show(
    cli_workspace: Option<&Path>,
    cli_data: Option<&Path>,
    cli_cache: Option<&Path>,
) -> Result<ExitCode, Failure> {
    let loaded = LoadedSettings::load()?;
    let overrides = loaded.overrides();
    let workspace = locate_workspace_with(&ProcessHost, cli_workspace, overrides.workspace)
        .map_err(|error| error.to_string())?;
    let data = locate_data_with(&ProcessHost, cli_data, overrides.data_dir)
        .map_err(|error| error.to_string())?;
    let cache = locate_cache_with(&ProcessHost, cli_cache, overrides.cache_dir)
        .map_err(|error| error.to_string())?;
    let settings_path = default_settings_file(&ProcessHost).map_err(|error| error.to_string())?;
    println!(
        "settings_file: {} ({})",
        absolute_from_cwd(&settings_path)?.display(),
        if settings_path.exists() {
            "present"
        } else {
            "absent"
        }
    );
    print_workspace(&workspace)?;
    print_dir("data_dir", StorageKind::Data, &data)?;
    print_dir("cache_dir", StorageKind::Cache, &cache)?;
    Ok(ExitCode::SUCCESS)
}

fn print_workspace(location: &WorkspaceLocation) -> Result<(), Failure> {
    let path = absolute_from_cwd(&location.path)?;
    println!(
        "workspace: {} ({})",
        path.display(),
        location.origin.label()
    );
    Ok(())
}

fn print_dir(name: &str, kind: StorageKind, location: &DirLocation) -> Result<(), Failure> {
    let path = absolute_from_cwd(&location.path)?;
    println!(
        "{name}: {} ({})",
        path.display(),
        origin_label(kind, location.origin)
    );
    Ok(())
}

pub(crate) fn set(
    cli_workspace: Option<&Path>,
    cli_data: Option<&Path>,
    cli_cache: Option<&Path>,
    workspace: Option<PathBuf>,
    data_dir: Option<PathBuf>,
    cache_dir: Option<PathBuf>,
) -> Result<ExitCode, Failure> {
    if workspace.is_none() && data_dir.is_none() && cache_dir.is_none() {
        return Err(Failure::Usage(
            "settings set needs workspace, data-dir, or cache-dir".to_owned(),
        ));
    }
    let loaded = LoadedSettings::load()?;
    let overrides = loaded.overrides();
    let mut next = loaded.settings.clone().unwrap_or_default();
    let mut cleanup = Vec::new();
    let mut staged_workspace: Option<PathBuf> = None;
    let mut staged_database: Option<PathBuf> = None;

    if let Some(path) = workspace {
        let current = locate_workspace_with(&ProcessHost, cli_workspace, overrides.workspace)
            .map_err(|error| error.to_string())?;
        if !current.origin.to_path_origin().is_editable() {
            return Err(Failure::Data(format!(
                "workspace is fixed by {} and cannot be changed in settings",
                current.origin.label()
            )));
        }
        let target = absolute_from_cwd(&platform_path(&path))?;
        if current.path != target {
            relocate_workspace_file(&current.path, &target).map_err(|error| error.to_string())?;
            craftward_core::load_or_empty(&target).map_err(|error| error.to_string())?;
            staged_workspace = Some(target.clone());
            if current.path.exists() {
                cleanup.push(Cleanup::Workspace {
                    path: current.path.clone(),
                });
            }
        }
        next.workspace = Some(target);
    }
    if let Some(path) = data_dir {
        let current = locate_data_with(&ProcessHost, cli_data, overrides.data_dir)
            .map_err(|error| error.to_string())?;
        if !current.origin.is_editable() {
            return Err(Failure::Data(format!(
                "data_dir is fixed by {} and cannot be changed in settings",
                origin_label(StorageKind::Data, current.origin)
            )));
        }
        let target = absolute_from_cwd(&platform_path(&path))?;
        if current.path != target {
            std::fs::create_dir_all(&target).map_err(|error| error.to_string())?;
            let from_db = current.path.join(DATABASE_FILE);
            let to_db = target.join(DATABASE_FILE);
            if to_db.exists() {
                return Err(Failure::Data(format!(
                    "target already exists: {}",
                    to_db.display()
                )));
            }
            if from_db.exists() {
                copy_database(&from_db, &to_db).map_err(|error| error.to_string())?;
                check_database_integrity(&to_db).map_err(|error| error.to_string())?;
                staged_database = Some(to_db);
                cleanup.push(Cleanup::Database { path: from_db });
            }
        }
        next.data_dir = Some(target);
    }
    if let Some(path) = cache_dir {
        let current = locate_cache_with(&ProcessHost, cli_cache, overrides.cache_dir)
            .map_err(|error| error.to_string())?;
        if !current.origin.is_editable() {
            return Err(Failure::Data(format!(
                "cache_dir is fixed by {} and cannot be changed in settings",
                origin_label(StorageKind::Cache, current.origin)
            )));
        }
        let target = absolute_from_cwd(&platform_path(&path))?;
        std::fs::create_dir_all(&target).map_err(|error| error.to_string())?;
        next.cache_dir = Some(target);
    }

    if let Err(error) = save_settings(&ProcessHost, &next) {
        // Roll back staged copies; leave sources untouched.
        if let Some(path) = staged_workspace {
            let _ = remove_relocated_file(&path);
        }
        if let Some(path) = staged_database {
            let _ = remove_database_files(&path);
        }
        return Err(Failure::Data(error.to_string()));
    }

    for item in cleanup {
        match item {
            Cleanup::Workspace { path } => {
                remove_relocated_file(&path).map_err(|error| error.to_string())?;
            }
            Cleanup::Database { path } => {
                remove_database_files(&path).map_err(|error| error.to_string())?;
            }
        }
    }
    println!("updated settings");
    Ok(ExitCode::SUCCESS)
}

/// Shared path resolution for other CLI commands.
pub(crate) fn resolve_workspace(
    cli_workspace: Option<&Path>,
) -> Result<WorkspaceLocation, Failure> {
    let loaded = LoadedSettings::load()?;
    locate_workspace_with(&ProcessHost, cli_workspace, loaded.overrides().workspace)
        .map_err(|error| Failure::Data(error.to_string()))
}

pub(crate) fn resolve_data(cli_data: Option<&Path>) -> Result<DirLocation, Failure> {
    let loaded = LoadedSettings::load()?;
    locate_data_with(&ProcessHost, cli_data, loaded.overrides().data_dir)
        .map_err(|error| Failure::Data(error.to_string()))
}

pub(crate) fn resolve_cache(cli_cache: Option<&Path>) -> Result<DirLocation, Failure> {
    let loaded = LoadedSettings::load()?;
    locate_cache_with(&ProcessHost, cli_cache, loaded.overrides().cache_dir)
        .map_err(|error| Failure::Data(error.to_string()))
}
