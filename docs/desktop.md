# Desktop application

Craftward's desktop app (`craftward-app`) shows a read-only portfolio of
every project in the workspace and a detail page for one project. It uses
the same files as the command-line tool.

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
  projects, issues, and findings.
- **Detail:** header, stack, phases with tasks and progress, open issues,
  open findings (severity, kind, source), and deadlines. Open a card with
  the mouse or the Open button. Return with **Back** or **Escape**.

**Reload** reloads the workspace, project files, and findings. There is no
file watcher in this release.

## Keyboard

All actions are reachable without a mouse: Tab moves focus, buttons
activate with Enter/Space, Escape leaves the detail view, Reload and Back
are ordinary buttons in the top bar.

## Theme

Colours match the dark HTML roadmaps (dark background, slightly lighter
cards, blue accent). Severity and status always include text, not colour
alone. Text contrast against surfaces is checked in unit tests (WCAG AA
4.5:1).

## Known limitations

- Read-only: no edits, no radar run, no network.
- No file watching; use Reload after CLI changes.
- No window-position persistence (eframe `persistence` is off).
- Findings appear only after a CLI `radar` run has created `craftward.db`.

Architecture: [ADR 0023](adr/0023-desktop-app-with-eframe.md).
