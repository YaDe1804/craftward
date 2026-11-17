# ADR 0027: Settings file and data relocation

- Status: Accepted
- Date: 2026-11-17

## Decision

Craftward stores optional path overrides in a versioned `settings.json`
next to the platform-default workspace directory. The desktop app and the
CLI resolve workspace, data, and cache paths in one shared order in
`craftward-core`: command-line option, environment variable, settings
file, then platform default.

First launch (no settings file and no workspace file at the
settings-free resolve path) shows a short desktop setup wizard. Cancel
writes nothing. Finish creates the chosen folders, an empty workspace
file, and `settings.json`.

Changing a location from Settings or `craftward settings set` relocates
existing data before the settings file is updated: workspace copy,
database copy through SQLite's online backup API plus
`PRAGMA integrity_check`, cache create-only. The old copy is removed only
after a successful write. An occupied target is refused (CLI) or confirmed
with the user (app). Paths fixed by CLI or environment variables are not
editable in settings.

CLI destinations use dedicated subcommands with a positional path
(`settings set workspace FILE`, `data-dir DIR`, `cache-dir DIR`). The
global `--workspace`, `--data-dir`, and `--cache-dir` options keep a
single meaning everywhere: override resolution for that invocation.

## Rationale

One resolution path keeps the app and CLI aligned. A strict settings
document matches the other Craftward JSON files. Relocation through the
same backup API used for migrations keeps WAL contents intact and avoids
half-moved databases. Separate set subcommands keep global path flags
unambiguous.
