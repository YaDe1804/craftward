# ADR 0012: Workspace registry

- Status: Accepted
- Date: 2026-11-15

## Decision

Craftward keeps a `workspace.json` registry of paths to `craftward.json`
files. The registry has its own schema version, starting at 1, and rejects
unknown fields. It does not copy project data.

The file in use is the `--workspace` option, otherwise the
`CRAFTWARD_WORKSPACE` environment variable, otherwise a suggested directory
for the operating system. Those directories are defaults. Every one of them
can be replaced. Relative paths in the registry are resolved from the
directory that contains the workspace file. Absolute paths are allowed.

Project files stay where the user put them. Adding a project records that
path. Removing a project drops the registry entry and leaves the file in
place.

## Rationale

One `craftward.json` per project is the source of truth for user-maintained
data, and storage locations are never hard-coded. A registry of paths is
enough for a cross-project list. The project file can remain in the
repository or folder the user already keeps.
