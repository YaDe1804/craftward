# ADR 0024: Editing from the desktop app

- Status: Accepted
- Date: 2026-11-16
- Updated: 2026-11-16 (AP12)

## Decision

The desktop app edits `craftward.json` and `workspace.json` only through the
same `craftward-core` functions the CLI uses. Validation, id formation,
canonical serialization, and atomic replacement stay in the core. The UI does
not invent a second write path.

When the app loads a file it stores a fingerprint of the exact bytes. Before
every write it re-reads the path and compares. If the bytes differ, the core
returns a conflict, nothing is written, and the UI shows a sticky banner with
Reload. After a successful write the app reloads from disk so the view matches
the saved file.

Reversible edits do not use confirmation dialogs. Instead the app shows a
non-blocking toast (about eight seconds) with an Undo control. Undo restores
the previous bytes through the same fingerprint-checked write path. Only the
latest change is undoable; a newer save replaces the toast. If the file
changed outside the app before Undo, the restore is refused and a conflict is
shown. Deleting `craftward.json` is irreversible and therefore asks for
confirmation; see [ADR 0025](0025-native-dialogs-and-clipboard.md).

Every app action has a CLI equivalent. New field editors (`phase set`,
`task set`, `issue set`, and header tags/links) live in the core and CLI as
well as the desktop path.

## Rationale

One writer keeps CLI and app byte-identical for the same change and the same
`updated` day. Fingerprints catch an external edit without a file watcher.
Undo instead of confirm dialogs keeps reversible edits fast while still
recovering from mistakes. Sticky conflict banners survive scrolling so a
failed save is not lost inside a long detail page.
