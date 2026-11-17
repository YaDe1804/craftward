//! Native folder and file pickers for the desktop forms.
//!
//! Dialogs run on a background thread so the egui surface stays responsive.
//! Results arrive through a channel that the UI drains each frame.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use eframe::egui::Context;

/// What the form asked the picker to choose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickKind {
    /// Existing directory for a new project's `craftward.json`.
    Folder,
    /// Optional source directory when code lives outside the project folder.
    SourceFolder,
    /// Existing `craftward.json` file.
    ProjectFile,
    /// Existing directory that contains `craftward.json`.
    ProjectFolder,
    /// Existing lockfile (`Cargo.lock` or `package-lock.json`).
    Lockfile,
    /// Source directory while editing project header fields.
    HeaderSourceFolder,
    /// Parent folder for the workspace file (onboarding or settings).
    WorkspaceFolder,
    /// Data directory for craftward.db.
    DataFolder,
    /// Cache directory for HTTP responses.
    CacheFolder,
}

/// One completed picker result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickResult {
    /// Which form field the path fills.
    pub kind: PickKind,
    /// Chosen path, or [`None`] when the user cancelled.
    pub path: Option<PathBuf>,
}

/// Starts native dialogs without blocking the UI thread.
pub trait PathPicker: Send {
    /// Opens a folder dialog and eventually sends the result on `tx`.
    fn pick_folder(&self, ctx: Context, tx: Sender<PickResult>);

    /// Opens a folder dialog for an optional source tree.
    fn pick_source_folder(&self, ctx: Context, tx: Sender<PickResult>);

    /// Opens a folder dialog for the header form's source directory.
    fn pick_header_source_folder(&self, ctx: Context, tx: Sender<PickResult>);

    /// Opens a file dialog filtered to JSON and eventually sends the result.
    fn pick_project_file(&self, ctx: Context, tx: Sender<PickResult>);

    /// Opens a folder dialog for an existing project directory.
    fn pick_project_folder(&self, ctx: Context, tx: Sender<PickResult>);

    /// Opens a file dialog for a supported lockfile.
    fn pick_lockfile(&self, ctx: Context, tx: Sender<PickResult>);

    /// Opens a folder dialog for the workspace file's parent directory.
    fn pick_workspace_folder(&self, ctx: Context, tx: Sender<PickResult>);

    /// Opens a folder dialog for the data directory.
    fn pick_data_folder(&self, ctx: Context, tx: Sender<PickResult>);

    /// Opens a folder dialog for the cache directory.
    fn pick_cache_folder(&self, ctx: Context, tx: Sender<PickResult>);
}

/// Production picker that uses `rfd` on a worker thread.
#[derive(Debug, Default, Clone, Copy)]
pub struct RfdPicker;

impl PathPicker for RfdPicker {
    fn pick_folder(&self, ctx: Context, tx: Sender<PickResult>) {
        spawn_pick(ctx, tx, PickKind::Folder, || {
            rfd::FileDialog::new()
                .set_title("Folder for craftward.json")
                .pick_folder()
        });
    }

    fn pick_source_folder(&self, ctx: Context, tx: Sender<PickResult>) {
        spawn_pick(ctx, tx, PickKind::SourceFolder, || {
            rfd::FileDialog::new()
                .set_title("Source folder (if the code lives elsewhere)")
                .pick_folder()
        });
    }

    fn pick_header_source_folder(&self, ctx: Context, tx: Sender<PickResult>) {
        spawn_pick(ctx, tx, PickKind::HeaderSourceFolder, || {
            rfd::FileDialog::new()
                .set_title("Source folder (if the code lives elsewhere)")
                .pick_folder()
        });
    }

    fn pick_project_file(&self, ctx: Context, tx: Sender<PickResult>) {
        spawn_pick(ctx, tx, PickKind::ProjectFile, || {
            rfd::FileDialog::new()
                .set_title("Choose craftward.json")
                .add_filter("craftward.json", &["json"])
                .pick_file()
        });
    }

    fn pick_project_folder(&self, ctx: Context, tx: Sender<PickResult>) {
        spawn_pick(ctx, tx, PickKind::ProjectFolder, || {
            rfd::FileDialog::new()
                .set_title("Choose folder that contains craftward.json")
                .pick_folder()
        });
    }

    fn pick_lockfile(&self, ctx: Context, tx: Sender<PickResult>) {
        spawn_pick(ctx, tx, PickKind::Lockfile, || {
            rfd::FileDialog::new()
                .set_title("Choose lockfile")
                .add_filter("Lockfiles", &["lock", "json"])
                .pick_file()
        });
    }

    fn pick_workspace_folder(&self, ctx: Context, tx: Sender<PickResult>) {
        spawn_pick(ctx, tx, PickKind::WorkspaceFolder, || {
            rfd::FileDialog::new()
                .set_title("Folder for workspace.json")
                .pick_folder()
        });
    }

    fn pick_data_folder(&self, ctx: Context, tx: Sender<PickResult>) {
        spawn_pick(ctx, tx, PickKind::DataFolder, || {
            rfd::FileDialog::new()
                .set_title("Folder for craftward.db")
                .pick_folder()
        });
    }

    fn pick_cache_folder(&self, ctx: Context, tx: Sender<PickResult>) {
        spawn_pick(ctx, tx, PickKind::CacheFolder, || {
            rfd::FileDialog::new()
                .set_title("Folder for the HTTP cache")
                .pick_folder()
        });
    }
}

fn spawn_pick(
    ctx: Context,
    tx: Sender<PickResult>,
    kind: PickKind,
    open: impl FnOnce() -> Option<PathBuf> + Send + 'static,
) {
    thread::spawn(move || {
        let path = open();
        let _ = tx.send(PickResult { kind, path });
        ctx.request_repaint();
    });
}

/// Next value a [`FakePicker`] should return.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FakePick {
    /// Dialog cancelled.
    Cancelled,
    /// Dialog chose this path.
    Path(PathBuf),
}

/// Test double that returns a prepared path without opening a dialog.
#[derive(Debug, Default)]
#[allow(dead_code)] // Used from unit tests; kept on the bin target for the trait.
pub struct FakePicker {
    /// Next pick outcome.
    pub next: std::sync::Mutex<Option<FakePick>>,
}

#[allow(dead_code)]
impl FakePicker {
    /// Builds a fake that yields `path` on the next pick.
    #[must_use]
    pub fn with_path(path: Option<PathBuf>) -> Self {
        let next = match path {
            Some(path) => FakePick::Path(path),
            None => FakePick::Cancelled,
        };
        Self {
            next: std::sync::Mutex::new(Some(next)),
        }
    }

    fn take_next(&self) -> Option<PathBuf> {
        match self
            .next
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
        {
            Some(FakePick::Path(path)) => Some(path),
            Some(FakePick::Cancelled) | None => None,
        }
    }
}

impl PathPicker for FakePicker {
    fn pick_folder(&self, ctx: Context, tx: Sender<PickResult>) {
        let _ = tx.send(PickResult {
            kind: PickKind::Folder,
            path: self.take_next(),
        });
        ctx.request_repaint();
    }

    fn pick_source_folder(&self, ctx: Context, tx: Sender<PickResult>) {
        let _ = tx.send(PickResult {
            kind: PickKind::SourceFolder,
            path: self.take_next(),
        });
        ctx.request_repaint();
    }

    fn pick_header_source_folder(&self, ctx: Context, tx: Sender<PickResult>) {
        let _ = tx.send(PickResult {
            kind: PickKind::HeaderSourceFolder,
            path: self.take_next(),
        });
        ctx.request_repaint();
    }

    fn pick_project_file(&self, ctx: Context, tx: Sender<PickResult>) {
        let _ = tx.send(PickResult {
            kind: PickKind::ProjectFile,
            path: self.take_next(),
        });
        ctx.request_repaint();
    }

    fn pick_project_folder(&self, ctx: Context, tx: Sender<PickResult>) {
        let _ = tx.send(PickResult {
            kind: PickKind::ProjectFolder,
            path: self.take_next(),
        });
        ctx.request_repaint();
    }

    fn pick_lockfile(&self, ctx: Context, tx: Sender<PickResult>) {
        let _ = tx.send(PickResult {
            kind: PickKind::Lockfile,
            path: self.take_next(),
        });
        ctx.request_repaint();
    }

    fn pick_workspace_folder(&self, ctx: Context, tx: Sender<PickResult>) {
        let _ = tx.send(PickResult {
            kind: PickKind::WorkspaceFolder,
            path: self.take_next(),
        });
        ctx.request_repaint();
    }

    fn pick_data_folder(&self, ctx: Context, tx: Sender<PickResult>) {
        let _ = tx.send(PickResult {
            kind: PickKind::DataFolder,
            path: self.take_next(),
        });
        ctx.request_repaint();
    }

    fn pick_cache_folder(&self, ctx: Context, tx: Sender<PickResult>) {
        let _ = tx.send(PickResult {
            kind: PickKind::CacheFolder,
            path: self.take_next(),
        });
        ctx.request_repaint();
    }
}

/// Holds the channel end the UI drains each frame.
pub struct PickerInbox {
    tx: Sender<PickResult>,
    rx: Receiver<PickResult>,
}

impl Default for PickerInbox {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self { tx, rx }
    }
}

impl PickerInbox {
    /// Cloneable sender for a background pick.
    #[must_use]
    pub fn sender(&self) -> Sender<PickResult> {
        self.tx.clone()
    }

    /// Takes one pending result, if any.
    pub fn try_recv(&self) -> Option<PickResult> {
        self.rx.try_recv().ok()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use eframe::egui::Context;

    use super::{FakePicker, PathPicker, PickKind, PickerInbox};

    #[test]
    fn fake_picker_delivers_path_into_inbox() {
        let inbox = PickerInbox::default();
        let picker = FakePicker::with_path(Some(PathBuf::from("/tmp/demo")));
        let ctx = Context::default();
        picker.pick_folder(ctx, inbox.sender());
        let result = inbox.try_recv().expect("result");
        assert_eq!(result.kind, PickKind::Folder);
        assert_eq!(result.path, Some(PathBuf::from("/tmp/demo")));
    }
}
