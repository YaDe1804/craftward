//! `craftward stack`.

use std::process::ExitCode;
use std::time::SystemTime;

use craftward_core::{
    AndroidChanges, ComponentChanges, EditError, NewComponent, ProjectUpdate, add_component,
    add_lockfile, edit_file, remove_component, remove_lockfile, set_android, set_component,
    utc_date,
};

use crate::{Failure, resolve_project};

/// One stack command.
pub(crate) enum Request {
    /// Append a component.
    Add(AddRequest),
    /// Change one component.
    Set(SetRequest),
    /// Remove one component.
    Remove(NameRequest),
    /// Append a lockfile path.
    AddLockfile(PathRequest),
    /// Remove a lockfile path.
    RemoveLockfile(PathRequest),
    /// Create or change `stack.android`.
    Android(AndroidRequest),
}

/// Arguments of `stack android`.
pub(crate) struct AndroidRequest {
    /// Project id or path.
    pub project: String,
    /// `target_sdk` update. An empty string clears it.
    pub target_sdk: Update,
    /// `min_sdk` update. An empty string clears it.
    pub min_sdk: Update,
    /// Form factor, when the flag was present.
    pub form_factor: Option<String>,
    /// Store, when the flag was present.
    pub store: Option<String>,
    /// Paths to add to `detect_from`.
    pub detect_from: Vec<String>,
    /// Drop the current `detect_from` list before adding paths.
    pub clear_detect_from: bool,
}

/// Arguments of `stack add`.
pub(crate) struct AddRequest {
    /// Project id or path.
    pub project: String,
    /// Component role.
    pub kind: String,
    /// Component name.
    pub name: String,
    /// Version, when given.
    pub version: Option<String>,
    /// Note, when given.
    pub note: Option<String>,
    /// endoflife.date product id, when given.
    pub eol_product: Option<String>,
}

/// Arguments of `stack set`.
pub(crate) struct SetRequest {
    /// Project id or path.
    pub project: String,
    /// Component name.
    pub name: String,
    /// Version update.
    pub version: Update,
    /// Note update.
    pub note: Update,
    /// Product id update.
    pub eol_product: Update,
}

/// Arguments of `stack remove`.
pub(crate) struct NameRequest {
    /// Project id or path.
    pub project: String,
    /// Component name.
    pub name: String,
}

/// Arguments of a lockfile command.
pub(crate) struct PathRequest {
    /// Project id or path.
    pub project: String,
    /// Lockfile path.
    pub path: String,
}

/// Whether an optional field stays, changes, or is removed.
#[derive(Clone)]
pub(crate) enum Update {
    /// The flag was absent.
    Keep,
    /// Store this text.
    Set(String),
    /// The flag was an empty string.
    Clear,
}

pub(crate) fn run(
    request: &Request,
    workspace: Option<&std::path::Path>,
) -> Result<ExitCode, Failure> {
    let today = utc_date(SystemTime::now())
        .ok_or_else(|| Failure::Data("cannot read the system clock".to_owned()))?;
    match request {
        Request::Add(request) => {
            let path = resolve_project(&request.project, workspace)?;
            edit_file(&path, &today, |project, updated| {
                add_component(
                    project,
                    NewComponent {
                        kind: &request.kind,
                        name: &request.name,
                        version: request.version.as_deref(),
                        note: request.note.as_deref(),
                        eol_product: request.eol_product.as_deref(),
                    },
                    updated,
                )
            })
            .map_err(|error| map_edit(&error))?;
            println!("added component {}", request.name);
        }
        Request::Set(request) => {
            let path = resolve_project(&request.project, workspace)?;
            edit_file(&path, &today, |project, updated| {
                set_component(
                    project,
                    &request.name,
                    ComponentChanges {
                        version: to_update(&request.version),
                        note: to_update(&request.note),
                        eol_product: to_update(&request.eol_product),
                    },
                    updated,
                )
            })
            .map_err(|error| map_edit(&error))?;
            println!("updated component {}", request.name);
        }
        Request::Remove(request) => {
            let path = resolve_project(&request.project, workspace)?;
            edit_file(&path, &today, |project, updated| {
                remove_component(project, &request.name, updated)
            })
            .map_err(|error| map_edit(&error))?;
            println!("removed component {}", request.name);
        }
        Request::AddLockfile(request) => {
            let path = resolve_project(&request.project, workspace)?;
            edit_file(&path, &today, |project, updated| {
                add_lockfile(project, &request.path, updated)
            })
            .map_err(|error| map_edit(&error))?;
            println!("added lockfile {}", request.path);
        }
        Request::RemoveLockfile(request) => {
            let path = resolve_project(&request.project, workspace)?;
            edit_file(&path, &today, |project, updated| {
                remove_lockfile(project, &request.path, updated)
            })
            .map_err(|error| map_edit(&error))?;
            println!("removed lockfile {}", request.path);
        }
        Request::Android(request) => {
            let path = resolve_project(&request.project, workspace)?;
            edit_file(&path, &today, |project, updated| {
                set_android(
                    project,
                    AndroidChanges {
                        target_sdk: to_update(&request.target_sdk),
                        min_sdk: to_update(&request.min_sdk),
                        form_factor: request.form_factor.as_deref(),
                        store: request.store.as_deref(),
                        detect_from: &request.detect_from,
                        clear_detect_from: request.clear_detect_from,
                    },
                    updated,
                )
            })
            .map_err(|error| map_edit(&error))?;
            println!("updated android settings");
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn to_update(update: &Update) -> ProjectUpdate<'_> {
    match update {
        Update::Keep => ProjectUpdate::Keep,
        Update::Set(text) => ProjectUpdate::Set(text),
        Update::Clear => ProjectUpdate::Clear,
    }
}

fn map_edit(error: &EditError) -> Failure {
    if error.is_usage() {
        Failure::Usage(error.to_string())
    } else {
        Failure::Data(error.to_string())
    }
}
