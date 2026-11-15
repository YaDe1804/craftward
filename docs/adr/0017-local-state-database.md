# ADR 0017: Local state database

- Status: Accepted
- Date: 2026-11-15

## Decision

App-generated state lives in a local SQLite database, `craftward.db`.
`craftward-core` defines the storage operations and does not depend on
SQLite. `craftward-store` is the only crate that links it, through rusqlite
0.43.0 with default features off and `bundled` on. That compiles SQLite
3.54.1 into the program. The command-line tool depends on
`craftward-store`. The desktop application does not, until it reads the
same state. A later team server can implement the same operations.

The journal mode is WAL. Foreign keys are on, and the busy timeout is 5
seconds, on every connection that opens a file. The schema version is
`PRAGMA user_version`. Migrations move only forward, each in a transaction.
A file from a newer Craftward is rejected. Before a non-empty file is
migrated, it is copied to `craftward.db.bak-v<old version>`. An existing
copy of that name is an error and the file is left unchanged.

## Rationale

ADR 0006 keeps user-maintained data in `craftward.json` and app-generated
state behind an interface, so a team server can replace the local database
later. rusqlite's `bundled` feature compiles the SQLite sources with the
C compiler already used for this workspace, which is MSVC on Windows, and
does not need CMake or a SQLite library from the operating system. WAL is
persistent and lets a reader and a writer proceed together. It needs shared
memory next to the file, so the database stays on a local disk. The backup
copy is taken before the migration writes, and it is never replaced, so a
failed upgrade does not destroy the previous file.
