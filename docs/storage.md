# Local database

`craftward radar` stores findings, their history, and one row per project
check in `craftward.db`. `craftward findings` reads the same file. The
OSV cache stays a separate directory of JSON files, described in
[radar.md](radar.md).

## Location

The directory is `--data-dir`, then `CRAFTWARD_DATA_DIR`, then:

| System | Default |
| --- | --- |
| Windows | `%LOCALAPPDATA%\Craftward` |
| Linux | `$XDG_DATA_HOME/craftward`, or `~/.local/share/craftward` |
| macOS | `~/Library/Application Support/Craftward` |

An empty environment variable is treated as unset. The directory is created
when the database is opened. The file name is `craftward.db`.

Do not put this directory on a network drive. The database uses SQLite's
WAL journal, and WAL needs a small shared-memory file next to the database.
SQLite requires every process using a WAL database to be on the same
computer. A network filesystem does not provide that.

## Opening

Each connection turns foreign keys on and sets a busy timeout of 5 seconds.
The journal mode is WAL. WAL stays in effect after the connection closes.
An in-memory database cannot use WAL. SQLite keeps that one in memory mode.
The file database does not.

## Schema

The schema version is `PRAGMA user_version`. This build writes version 2.
Migrations only move forward, one version at a time, each in a transaction.
If the file's version is newer, Craftward stops and says that a newer
Craftward version created the database.

Before migrating a database file that already has contents, Craftward copies
it to `craftward.db.bak-v<old version>` in the same directory. If that copy
is already there, Craftward stops and leaves the database unchanged. A
missing file or a file of zero bytes is a new database and is not copied.

A failed project check is a row in `runs` with `error` set, `evaluated` 0,
and the counters at zero. The findings of that project are not changed. A
check that compared lockfiles leaves `error` null and `evaluated` 1. A
project with no lockfiles also leaves `error` null, and sets `evaluated`
to 0: nothing failed, and nothing was compared. Version 2 adds
`evaluated`. A version 1 row whose `error` is null is marked evaluated.
A version 1 row with `error` set stays unevaluated.

### runs

| Column | Meaning |
| --- | --- |
| `id` | Row id. |
| `project_id` | Project id. |
| `started_at`, `finished_at` | UTC timestamps, `YYYY-MM-DDTHH:MM:SSZ`. |
| `offline` | `1` when the check was not allowed to use the network. |
| `packages_checked` | Packages sent to OSV. Zero when the check failed. |
| `open_count`, `new_count`, `resolved_count` | Counts after a check that compared lockfiles. On a failed check every counter is zero. When nothing was evaluated, `open_count` is the number of findings already open and the other two counters are zero. |
| `evaluated` | `1` when lockfile results were compared with stored findings. `0` when they were not. |
| `error` | Why the check failed, or null. Null together with `evaluated` 0 means no lockfiles were configured. |

### findings

One row is unique on project id, ecosystem, package name, and vulnerability
id. The version is the last version that was still affected. It is not part
of the identity, so a new affected version updates the same row.

`state` is `open`, `ignored`, or `resolved`. `aliases` and `fixed_versions`
are JSON arrays of strings. `ignore_until` is `YYYY-MM-DD` or null. The
other times use the same UTC timestamp as `runs`.

### finding_events

One row per change: `discovered`, `ignored`, `reopened`, `resolved`, or
`reappeared`. `reason` is the ignore text, `ignore expired on YYYY-MM-DD`
when an ignore lapses, or an empty string.
