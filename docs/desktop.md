# Desktop application

Craftward's desktop app (`craftward-app`) shows a portfolio of every project
in the workspace, a detail page for one project, and a cross-project Issues
list. It uses the same files as the command-line tool. Edits go through the
same core writers as the CLI.

## Start

```console
cargo run -p craftward-app --release
```

`--version` prints the binary name and version and exits. Any other
argument is a usage error (exit status 2).

Window title: **Craftward**. Minimum size is 720×480.

## First-run setup

When there is no `settings.json` and no workspace file at the path resolved
without settings, the app opens a short setup wizard instead of the
portfolio:

1. Welcome — what Craftward does.
2. Storage locations — workspace file, data folder, cache folder, each with
   the platform default, a one-line explanation, and Browse. When
   `CRAFTWARD_WORKSPACE`, `CRAFTWARD_DATA_DIR`, or `CRAFTWARD_CACHE_DIR` is
   set, the wizard notes that the variable takes precedence.
3. Operating mode — **Local** only (no shared team folder yet).
4. Finish — creates the folders, an empty `workspace.json`, and
   `settings.json`, then shows the portfolio with a tip to use **New
   project** or **Add existing**.

**Cancel** closes the window without writing files; the next launch shows
the wizard again.

## Data sources

Path resolution matches the CLI (command line is unused in the app):

| Data | Resolution order |
| --- | --- |
| Workspace file | `CRAFTWARD_WORKSPACE`, then `settings.json`, then the platform default |
| Database (`craftward.db`) | `CRAFTWARD_DATA_DIR`, then `settings.json`, then the platform default |
| Cache | `CRAFTWARD_CACHE_DIR`, then `settings.json`, then the platform default |

Project files are the paths listed in `workspace.json`. Findings come from
`craftward.db` when that file exists. Counts and severity badges use only
open findings (same rule as the CLI); actively ignored rows appear as a
muted “N ignored” note and never in the severity chips. An ignore whose
`--until` date is in the past counts as open again. A project with no open
findings shows “No open findings”. The app never creates a database just to
open the window; a missing database is shown as a status message.

See [storage.md](storage.md) and [workspace-format.md](workspace-format.md).

## Navigation

A fixed left sidebar lists **Portfolio**, **Issues**, and **Settings**.
Portfolio expands to indented children: **All (n)**, named categories A–Z
with counts, **Uncategorized (n)** when needed, and **With errors (n)** only
when load failures exist. Load-error projects count under All and With
errors only — never under a category. The category selection stays for the
session, including when opening and returning from a project. The active
entry is highlighted and is reachable by keyboard focus. Workspace and
database paths appear at the bottom of the sidebar.

The project detail page belongs to Portfolio. The top breadcrumb is
**Portfolio › _name_** with ordinary word spacing and Portfolio clickable;
Esc returns there. There is no separate Back button when that breadcrumb is
available.

**Reload** reloads the workspace, project files, and findings. There is no
file watcher in this release.

## Views

- **Portfolio overview:** one card per project, sorted by urgency, filtered
  by the sidebar category plus optional Platform and Tag combos (AND).
  Platforms are derived from the stack (`android` block and `platform`
  components), merged without regard to letter case, and shown with
  readable labels. **New project** and **Add existing** manage the
  workspace list (text fields plus optional native Browse dialogs). New
  project asks for a **Folder for craftward.json**, optional **Source
  folder**, and optional **Category** (suggestions from the portfolio).
  **Remove from list** runs immediately with an undo toast (ADR 0024);
  **Remove and delete craftward.json…** asks for confirmation because file
  deletion cannot be undone. Clicking the ··· menu button opens the card
  menu (Enter does the same when that button is focused); clicking the rest
  of the card opens the project.
- **Detail:** header (including category and **Remove from list** /
  **Delete craftward.json…**), stack, phases with tasks, open issues,
  findings, and **Watch items & deadlines**. Two columns from 900 px width;
  one column below that.
- **Issues:** table with fixed columns Severity, Status, Project, Title
  (title wraps in its column). Filters: status (default open only),
  severity, project, category. Sort: severity descending, then project
  name, then title. Click a row to open that project with the issue marked.
  Status can be changed in the list; undo uses the same path as on the
  project page.
- **Settings:** shows the three storage locations with their origin
  (environment variable, settings file, or default). **Change…** opens a
  folder dialog and relocates existing data as in [storage.md](storage.md).
  A path fixed by an environment variable cannot be changed here; the
  screen explains why. If the target already exists, the app asks instead
  of overwriting.

## Editing

Supported in this release:

- Change task and issue status from the status chip menu (including from
  the Issues list).
- Edit overview header fields (name, status, version, description, tags,
  source folder with Browse and explanation, links).
- Add, edit, and remove phases (removing a phase with tasks is allowed;
  Undo restores the phase and its tasks).
- Add, edit, and remove tasks and issues (task: title, due, note; issue:
  title, severity, URL, note). Due dates follow the OS locale via
  `sys-locale`, with `LANG` / `LC_*` as fallback (`DD.MM.YYYY` for German,
  otherwise `YYYY-MM-DD`); both forms are accepted on input and storage
  stays ISO.
- Stack: add, edit, and remove components (kind, name, version,
  eol_product); add and remove lockfiles (path relative to `source_dir`,
  native Browse dialog, must exist as `Cargo.lock` or `package-lock.json`
  under that folder; picks outside the folder are rejected); edit Android
  with readable labels (Google Play, Phone, and so on).
- Watch items under **Watch items & deadlines**: add (**Add watch item**),
  edit, and remove (kind, title, due, note). Task due dates in that list
  stay display-only.
- Create a project (folder + optional source folder + id + name + status;
  id is suggested from the name until edited by hand), add an existing
  `craftward.json`, remove a project from the workspace list (optionally
  delete only that `craftward.json`).

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
an open form or leaves the detail view for Portfolio, Reload is an ordinary
button in the top bar. Ctrl/Cmd+C, V, X, and A work in text fields; selected
labels can be copied when the OS clipboard feature is enabled.

## Theme

Colours and status glyphs match the dark HTML roadmaps (ADR 0026). Each
project lifecycle status has a named swatch in `theme.rs` (`idea`
`#a371f7`, `in_development` `#d29922`, `released` `#3fb950`, `maintenance`
`#58a6ff`, `paused` `#8b98a7`, `discontinued` `#a5534f`). Project-status
badges use that colour for text, border, and a lightly tinted fill; task and
issue status chips keep the existing open / running / done palette. Status
chips that open a menu are the menu button itself (no double frame).

Portfolio cards and the project Overview header draw a soft radial status
glow: one shared grayscale texture (built once at startup, linear filter),
tinted with the project status colour, covering about the upper half of the
card and fading out toward the centre. Idle / hover / pressed opacities are
named constants (`CARD_GLOW_OPACITY_*`, up to 40 / 50 / 60 %); transitions
use egui’s `animate_value_with_time` (~120 ms). Contrast checks only text
that sits on the glow (title and secondary lines at their row height via the
same falloff as the texture); badges use an opaque fill so the glow does not
change their contrast; text below the glow is checked on the card surface.
Colour is never the only signal — the status badge keeps text and a glyph.

Done and dropped task titles use muted text with WCAG AA contrast. Text
contrast against surfaces, badge fills, on-glow composites, and glyph
coverage are checked in unit tests.

## Known limitations

- Phase status is derived from tasks (no schema field).
- No radar run and no network from the app.
- No file watching; use Reload after CLI changes.
- No window-position persistence (eframe `persistence` is off).
- Findings appear only after a CLI `radar` run has created `craftward.db`.
- No calendar date picker; dates are typed.

Architecture: [ADR 0023](adr/0023-desktop-app-with-eframe.md),
[ADR 0024](adr/0024-editing-from-the-desktop-app.md),
[ADR 0025](adr/0025-native-dialogs-and-clipboard.md),
[ADR 0026](adr/0026-navigation-and-visual-design.md),
[ADR 0027](adr/0027-settings-file-and-data-relocation.md).
