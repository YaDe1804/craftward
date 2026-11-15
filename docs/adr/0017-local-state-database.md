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
migrated, SQLite's online backup API copies it to
`craftward.db.bak-v<old version>`. An existing copy of that name is an
error and the file is left unchanged. The new copy is opened read-only and
must pass `PRAGMA integrity_check` with the single result `ok`. A copy that
fails that check is removed, so a later open can try again; a copy from an
earlier attempt is never replaced. Writing transactions use
`BEGIN IMMEDIATE`, so a second writer waits on the busy timeout instead of
doing work it cannot commit. Schema version 6 stores finding `state`,
finding `kind`, and event `kind` as text without `CHECK` lists. Those words
are checked when a row is read. A new word is a change to the Rust enums,
not a schema migration. The version 6 rebuild turns foreign keys off before
its transaction, runs `PRAGMA foreign_key_check` before the commit, and
turns foreign keys on again afterwards.

## Rationale

ADR 0006 keeps user-maintained data in `craftward.json` and app-generated
state behind an interface, so a team server can replace the local database
later. rusqlite's `bundled` feature compiles the SQLite sources with the
C compiler already used for this workspace, which is MSVC on Windows, and
does not need CMake or a SQLite library from the operating system. WAL is
persistent and lets a reader and a writer proceed together. It needs shared
memory next to the file, so the database stays on a local disk. The backup
is taken before the migration writes, and it is never replaced, so a
failed upgrade does not destroy the previous file. The online backup API
(`sqlite3_backup_init`, exposed by rusqlite's `backup` feature) is the
SQLite interface for a consistent snapshot of a live database. It reads
through the pager, so committed frames that are still in the WAL, and not
yet checkpointed into the main file, are part of the copy. Copying the
main file with the operating system leaves those frames behind. `VACUUM
INTO` can also snapshot a live database, but it is a maintenance command
and fails when the same connection has an open transaction. The backup API
is built for this copy. `BEGIN IMMEDIATE` takes the reserved lock at the
start of the transaction. Two writers then queue, and the busy timeout is
what makes the second one wait. Checking the closed vocabularies in Rust
means a new finding kind, severity, state, or event does not require
rebuilding the tables. The version 6 rebuild follows SQLite's steps for a
schema change that `ALTER TABLE` cannot express, so foreign-key references
stay valid after the tables are replaced.
