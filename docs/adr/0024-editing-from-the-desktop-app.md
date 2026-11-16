# ADR 0024: Editing from the desktop app

- Status: Accepted
- Date: 2026-11-16

## Decision

The desktop app edits `craftward.json` only through the same `craftward-core`
functions the CLI uses (`add_task`, `set_task_status`, `add_issue`,
`set_issue_status`, and the shared `edit_file` / `edit_file_if_unchanged`
writers). Validation, id formation, canonical serialization, and atomic
replacement stay in the core. The UI does not invent a second write path.

When the app loads a project file it stores a fingerprint of the exact file
bytes. Before every write it re-reads the path and compares. If the bytes
differ, the core returns `EditError::Conflict`, nothing is written, and the
UI shows that the file changed outside the app and offers Reload. After a
successful write the app reloads from disk so the view matches the saved
file, not an in-memory draft.

This package covers task status, issue status, adding a task to a phase, and
adding an issue. Deleting items, editing phases, stack, header fields, and
watch items stay CLI-only for now so the first editable surface stays small
and reviewable.

## Rationale

One writer keeps CLI and app byte-identical for the same change and the same
`updated` day. Fingerprints catch an external edit without a file watcher.
Leaving delete and structural edits for a later package avoids shipping a
large destructive surface before the conflict and reload behaviour is proven.
