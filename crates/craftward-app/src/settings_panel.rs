//! Settings screen: show storage locations and relocate them.

use std::path::{Path, PathBuf};

use eframe::egui::{Button, RichText, Ui};

use craftward_core::StorageKind;

use crate::picker::{PathPicker, PickKind, PickerInbox};
use crate::storage::{
    StorageSnapshot, dir_editable, dir_origin_label, relocate_cache_dir, relocate_data_dir,
    relocate_workspace, workspace_editable, workspace_origin_label,
};
use crate::theme::{ACCENT, HEADING, MUTED, SEVERITY_CRITICAL, TEXT, TYPE};

/// Which location is being changed with a pending folder pick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTarget {
    Workspace,
    Data,
    Cache,
}

/// Pending conflict when the target already exists.
#[derive(Debug, Clone)]
pub struct OccupiedPrompt {
    /// Which location the user tried to change.
    pub target: SettingsTarget,
    /// Absolute path that already exists.
    pub path: PathBuf,
}

/// Mutable Settings screen state.
#[derive(Debug, Default)]
pub struct SettingsEdit {
    pub message: Option<String>,
    pub error: Option<String>,
    pub pending_pick: Option<SettingsTarget>,
    pub occupied: Option<OccupiedPrompt>,
}

/// Draws the Settings screen.
#[allow(clippy::too_many_lines)]
pub fn draw_settings(
    ui: &mut Ui,
    snapshot: &StorageSnapshot,
    edit: &mut SettingsEdit,
    picker: &dyn PathPicker,
    inbox: &PickerInbox,
) {
    ui.label(
        RichText::new("Settings")
            .font(TYPE.title_id())
            .color(HEADING)
            .strong(),
    );
    ui.add_space(4.0);
    ui.label(
        RichText::new(
            "Storage locations resolve in this order: command-line option, \
             environment variable, settings file, then the platform default.",
        )
        .font(TYPE.secondary_id())
        .color(MUTED),
    );
    if let Some(path) = &snapshot.settings_file {
        ui.label(
            RichText::new(format!(
                "Settings file: {} ({})",
                path.display(),
                if snapshot.settings_present {
                    "present"
                } else {
                    "absent"
                }
            ))
            .font(TYPE.secondary_id())
            .color(MUTED),
        );
    }
    ui.add_space(12.0);

    if let Some(error) = &edit.error {
        ui.colored_label(SEVERITY_CRITICAL, error);
        ui.add_space(8.0);
    }
    if let Some(message) = &edit.message {
        ui.label(RichText::new(message).font(TYPE.body_id()).color(TEXT));
        ui.add_space(8.0);
    }

    if let Some(prompt) = edit.occupied.clone() {
        draw_occupied(ui, edit, &prompt);
        return;
    }

    let ctx = ui.ctx().clone();
    match &snapshot.workspace {
        Ok(location) => {
            let editable = workspace_editable(location.origin);
            let origin = workspace_origin_label(location.origin);
            let path_text = location.path.display().to_string();
            draw_location(
                ui,
                "Workspace file",
                &path_text,
                origin,
                editable,
                "The list of projects (workspace.json).",
                || {
                    edit.pending_pick = Some(SettingsTarget::Workspace);
                    picker.pick_workspace_folder(ctx.clone(), inbox.sender());
                },
            );
        }
        Err(error) => {
            ui.colored_label(SEVERITY_CRITICAL, format!("Workspace: {error}"));
        }
    }
    ui.add_space(10.0);
    match &snapshot.data {
        Ok(location) => {
            let editable = dir_editable(location.origin);
            let origin = dir_origin_label(StorageKind::Data, location.origin);
            let path_text = location.path.display().to_string();
            draw_location(
                ui,
                "Data folder",
                &path_text,
                origin,
                editable,
                "craftward.db and related data.",
                || {
                    edit.pending_pick = Some(SettingsTarget::Data);
                    picker.pick_data_folder(ctx.clone(), inbox.sender());
                },
            );
        }
        Err(error) => {
            ui.colored_label(SEVERITY_CRITICAL, format!("Data folder: {error}"));
        }
    }
    ui.add_space(10.0);
    match &snapshot.cache {
        Ok(location) => {
            let editable = dir_editable(location.origin);
            let origin = dir_origin_label(StorageKind::Cache, location.origin);
            let path_text = location.path.display().to_string();
            draw_location(
                ui,
                "Cache folder",
                &path_text,
                origin,
                editable,
                "Cached HTTP responses (safe to rebuild).",
                || {
                    edit.pending_pick = Some(SettingsTarget::Cache);
                    picker.pick_cache_folder(ctx, inbox.sender());
                },
            );
        }
        Err(error) => {
            ui.colored_label(SEVERITY_CRITICAL, format!("Cache folder: {error}"));
        }
    }
}

fn draw_location(
    ui: &mut Ui,
    title: &str,
    path: &str,
    origin: &str,
    editable: bool,
    help: &str,
    on_change: impl FnOnce(),
) {
    ui.label(
        RichText::new(title)
            .font(TYPE.section_id())
            .color(HEADING)
            .strong(),
    );
    ui.label(RichText::new(help).font(TYPE.secondary_id()).color(MUTED));
    ui.label(RichText::new(path).font(TYPE.body_id()).color(TEXT));
    ui.label(
        RichText::new(format!("Source: {origin}"))
            .font(TYPE.secondary_id())
            .color(MUTED),
    );
    if editable {
        if ui
            .add(Button::new(
                RichText::new("Change…").font(TYPE.body_id()).color(ACCENT),
            ))
            .clicked()
        {
            on_change();
        }
    } else {
        ui.label(
            RichText::new(format!(
                "Fixed by {origin}; unset that override to change this location in Settings."
            ))
            .font(TYPE.secondary_id())
            .color(MUTED),
        );
    }
}

fn draw_occupied(ui: &mut Ui, edit: &mut SettingsEdit, prompt: &OccupiedPrompt) {
    let kind = match prompt.target {
        SettingsTarget::Workspace => "workspace file",
        SettingsTarget::Data => "database",
        SettingsTarget::Cache => "cache folder",
    };
    ui.label(
        RichText::new(format!(
            "A {kind} already exists at {}.\nCraftward will not overwrite it. \
             Choose another folder, or cancel.",
            prompt.path.display()
        ))
        .font(TYPE.body_id())
        .color(TEXT),
    );
    ui.add_space(8.0);
    if ui
        .add(Button::new(
            RichText::new("Cancel").font(TYPE.body_id()).color(MUTED),
        ))
        .clicked()
    {
        edit.occupied = None;
        edit.pending_pick = None;
    }
}

/// Applies a completed folder pick to relocate storage.
pub fn apply_settings_pick(
    snapshot: &StorageSnapshot,
    edit: &mut SettingsEdit,
    kind: PickKind,
    folder: &Path,
) -> bool {
    let Some(target) = edit.pending_pick.take() else {
        return false;
    };
    let expected = match kind {
        PickKind::WorkspaceFolder => SettingsTarget::Workspace,
        PickKind::DataFolder => SettingsTarget::Data,
        PickKind::CacheFolder => SettingsTarget::Cache,
        _ => return false,
    };
    if expected != target {
        edit.pending_pick = Some(target);
        return false;
    }

    let mut settings = snapshot.settings.clone().unwrap_or_default();
    edit.error = None;
    edit.message = None;

    let result = match target {
        SettingsTarget::Workspace => {
            let Ok(current) = &snapshot.workspace else {
                edit.error = Some("workspace path is unavailable".to_owned());
                return true;
            };
            let new_file = folder.join("workspace.json");
            if new_file.exists() && new_file != current.path {
                edit.occupied = Some(OccupiedPrompt {
                    target,
                    path: new_file,
                });
                edit.pending_pick = Some(target);
                return true;
            }
            relocate_workspace(&current.path, &new_file, &mut settings)
        }
        SettingsTarget::Data => {
            let Ok(current) = &snapshot.data else {
                edit.error = Some("data path is unavailable".to_owned());
                return true;
            };
            let new_db = folder.join(craftward_core::DATABASE_FILE);
            if new_db.exists() && folder != current.path.as_path() {
                edit.occupied = Some(OccupiedPrompt {
                    target,
                    path: new_db,
                });
                edit.pending_pick = Some(target);
                return true;
            }
            relocate_data_dir(&current.path, folder, &mut settings)
        }
        SettingsTarget::Cache => relocate_cache_dir(folder, &mut settings),
    };

    match result {
        Ok(()) => {
            edit.message = Some("Location updated.".to_owned());
            true
        }
        Err(error) => {
            if error.contains("already exists") {
                edit.occupied = Some(OccupiedPrompt {
                    target,
                    path: PathBuf::from(folder),
                });
            } else {
                edit.error = Some(error);
            }
            true
        }
    }
}
