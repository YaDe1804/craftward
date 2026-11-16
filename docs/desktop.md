# Desktop application

Craftward's desktop app (`craftward-app`) shows a portfolio of every project
in the workspace and a detail page for one project. It uses the same files
as the command-line tool. A first set of edits goes through the same core
writers as the CLI.

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

- **Overview:** one card per project, sorted by urgency (open findings by
  severity, then next due date, then name). A summary strip counts
  projects, issues, and findings. The portfolio grid keeps column width on
  a short last row.
- **Detail:** header, stack, phases with tasks and progress, open issues,
  open findings (severity, kind, source), and deadlines. Open a card with
  the mouse or keyboard. Return with **Back** or **Escape**.

**Reload** reloads the workspace, project files, and findings. There is no
file watcher in this release.

## Editing

Supported in this release:

- Change a task status (`open`, `in_progress`, `done`, `dropped`) from the
  status symbol menu on each task row.
- Change an issue status (`open`, `in_progress`, `resolved`, `wont_fix`)
  from the status symbol menu on each open issue.
- Add a task to a phase (**Add task** at the end of the phase): title and
  optional due date (`YYYY-MM-DD`).
- Add an issue (**Add issue** at the end of the issue list): title,
  severity, optional note.

**Save** stays disabled until the form is valid (non-empty title; due date
empty or a calendar day). **Enter** saves a valid form; **Escape** closes
the form without writing. Validation and write errors appear next to the
control that triggered the action; typed input is kept.

Every save calls `craftward-core` the same way the CLI does. After a
successful write the app reloads the project from disk.

### Conflicts

On load the app remembers a fingerprint of each project file's bytes. Before
writing it reads the file again. If the bytes differ, nothing is written.
The UI reports that the file changed outside the app and offers **Reload**.
See [ADR 0024](adr/0024-editing-from-the-desktop-app.md).

## Keyboard

All actions are reachable without a mouse: Tab moves focus, buttons and
menus activate with Enter/Space, Escape closes an open form or leaves the
detail view, Reload and Back are ordinary buttons in the top bar.

## Theme

Colours match the dark HTML roadmaps (dark background, slightly lighter
cards, blue accent). Severity and status always include text, not colour
alone. Status, severity, and kind badges use readable text (and colour for
severity); decorative symbols are omitted when egui's bundled fonts do not
include a clear glyph. Text contrast against surfaces is checked in unit
tests (WCAG AA 4.5:1).

## Known limitations

- No delete; no phase, stack, header, or watch-item editing (CLI only).
- No radar run and no network from the app.
- No file watching; use Reload after CLI changes, or accept the conflict
  prompt if you edit without reloading.
- No window-position persistence (eframe `persistence` is off).
- Findings appear only after a CLI `radar` run has created `craftward.db`.

Architecture: [ADR 0023](adr/0023-desktop-app-with-eframe.md),
[ADR 0024](adr/0024-editing-from-the-desktop-app.md).
