# Desktop application

Craftward's desktop app (`craftward-app`) shows a portfolio of every project
in the workspace and a detail page for one project. It uses the same files
as the command-line tool. Edits go through the same core writers as the CLI.

## Start

```console
cargo run -p craftward-app --release
```

`--version` prints the binary name and version and exits. Any other
argument is a usage error (exit status 2).

Window title: **Craftward**. Minimum size is 720×480.

## Data sources

Path resolution matches the CLI:

| Data | Resolution order |
| --- | --- |
| Workspace file | `CRAFTWARD_WORKSPACE`, then the platform default directory |
| Database (`craftward.db`) | `CRAFTWARD_DATA_DIR`, then the platform default data directory |

Project files are the paths listed in `workspace.json`. Open findings come
from `craftward.db` when that file exists. The app never creates a database
just to open the window; a missing database is shown as a status message.

See [storage.md](storage.md) and [workspace-format.md](workspace-format.md).

## Views

- **Overview:** one card per project, sorted by urgency. **New project** and
  **Add existing** manage the workspace list (text fields plus optional
  native Browse dialogs). **Remove from list** runs immediately with an
  undo toast (ADR 0024); **Remove and delete craftward.json…** asks for
  confirmation because file deletion cannot be undone. Clicking the ···
  menu button opens the card menu (Enter does the same when that button is
  focused); clicking the rest of the card opens the project.
- **Detail:** header (including **Remove from list** and **Delete
  craftward.json…**), stack, phases with tasks, open issues, findings, and
  deadlines. Two columns from 900 px width; one column below that.

**Reload** reloads the workspace, project files, and findings. There is no
file watcher in this release.

## Editing

Supported in this release:

- Change task and issue status from the status chip menu.
- Edit overview header fields (name, status, version, description, tags,
  source directory, links).
- Add, edit, and remove phases (removing a phase with tasks is allowed;
  Undo restores the phase and its tasks).
- Add, edit, and remove tasks and issues (task: title, due, note; issue:
  title, severity, URL, note).
- Stack: add, edit, and remove components (kind, name, version,
  eol_product); add and remove lockfiles (path relative to `source_dir`,
  native Browse dialog, must exist as `Cargo.lock` or `package-lock.json`);
  edit Android (`target_sdk`, `min_sdk`, `form_factor`, `store`,
  `detect_from`).
- Watch items in the Deadlines section: add, edit, and remove (kind,
  title, due, note). Task due dates in that list stay display-only.
- Create a project (folder + id + name + status; id is suggested from the
  name until edited by hand), add an existing `craftward.json`, remove a
  project from the workspace list (optionally delete only that
  `craftward.json`).

**Save** stays disabled until the form is valid. Field errors stay next to
the input; drafts keep their text after a failed save. **Escape** closes an
open form. Every save calls `craftward-core` the same way the CLI does.

### Undo

After a successful save a toast appears in the footer for about eight
seconds: the change summary and **Undo**. Undo restores the previous file
bytes (or deletes a newly created `craftward.json`) through the fingerprint
check against the post-save state. Only the latest change can be undone. A
newer save replaces the toast. No confirmation dialog is shown for
undoable edits.

### Conflicts

On load the app remembers a fingerprint of each project file and of
`workspace.json`. Before writing it reads again. If the bytes differ,
nothing is written. A sticky banner stays visible while scrolling until
**Reload** or **Dismiss**. The same banner appears when Undo finds an
external change. See [ADR 0024](adr/0024-editing-from-the-desktop-app.md).

## Keyboard

Tab moves focus, buttons and menus activate with Enter/Space, Escape closes
an open form or leaves the detail view, Reload and Back are ordinary
buttons in the top bar. Ctrl/Cmd+C, V, X, and A work in text fields; selected
labels can be copied when the OS clipboard feature is enabled.

## Theme

Colours match the dark HTML roadmaps. Status chips are the menu button
itself (no double frame). Done and dropped task titles use muted text with
WCAG AA contrast. Text contrast against surfaces is checked in unit tests.

## Known limitations

- Phase status is derived from tasks (no schema field).
- No radar run and no network from the app.
- No file watching; use Reload after CLI changes.
- No window-position persistence (eframe `persistence` is off).
- Findings appear only after a CLI `radar` run has created `craftward.db`.

Architecture: [ADR 0023](adr/0023-desktop-app-with-eframe.md),
[ADR 0024](adr/0024-editing-from-the-desktop-app.md),
[ADR 0025](adr/0025-native-dialogs-and-clipboard.md).
