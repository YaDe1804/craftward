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
| Linux | `$XDG_DATA_HOME/craftward` when absolute, or `~/.local/share/craftward` |
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
A writing transaction starts with `BEGIN IMMEDIATE`. The second writer
blocks until the first commits, or until the busy timeout expires.
An in-memory database cannot use WAL. SQLite keeps that one in memory mode.
The file database does not.

## Schema

The schema version is `PRAGMA user_version`. This build writes version 6.
Migrations only move forward, one version at a time, each in a transaction.
If the file's version is newer, Craftward stops and says that a newer
Craftward version created the database.

Before migrating a database file that already has contents, Craftward
writes `craftward.db.bak-v<old version>` in the same directory through
SQLite's online backup API. That copy includes committed WAL frames that
have not been checkpointed into the main file. The backup is then opened
read-only and must pass `PRAGMA integrity_check` with the result `ok`.
Anything else aborts the migration. A backup created by that failed check
is removed. If a file of the backup name is already there, Craftward stops
and leaves the database unchanged. A missing file or a file of zero bytes
is a new database and is not backed up.

A failed project check is a row in `runs` with `error` set, `evaluated` 0,
and the counters at zero. The findings of that project are not changed. A
check that compared lockfiles leaves `error` null and `evaluated` 1. A
project with no lockfiles also leaves `error` null, and sets `evaluated`
to 0: nothing failed, and nothing was compared. Version 2 adds
`evaluated`. A version 1 row whose `error` is null is marked evaluated.
A version 1 row with `error` set stays unevaluated. Version 3 adds `kind`
on `findings`. Existing rows become `vulnerability`. The event table is
rebuilt in the same migration so `withdrawn` is allowed. Version 4 rebuilds
`findings` so `kind` may also be `platform_deadline` or
`platform_deadline_soon`. Existing rows keep their kind. The event table is
rebuilt again with the same event kinds, because a child table cannot stay
in place while its parent is replaced. Version 5 rebuilds only the event
table, so `kind` may also be `deadline_passed`. That event is recorded
when a Play finding changes from `platform_deadline_soon` to
`platform_deadline`. Version 6 rebuilds `findings` and `finding_events`
again and drops the `CHECK` lists on `state`, finding `kind`, and event
`kind`. Those words are checked when a row is read: an unknown word is an
error that names the column and the value. A new word is a change to the
Rust enums, not a schema change. The rebuild follows SQLite's procedure for
other kinds of table schema changes. Foreign keys are turned off before the
transaction, the new tables are filled, the old tables are dropped, and the
new tables are renamed. `PRAGMA foreign_key_check` must return no rows
before the commit. Foreign keys are turned on again afterwards, including
when the rebuild fails. The copy is still named for the version the file had
before the migration started.

### runs

| Column | Meaning |
| --- | --- |
| `id` | Row id. |
| `project_id` | Project id. |
| `started_at`, `finished_at` | UTC timestamps, `YYYY-MM-DDTHH:MM:SSZ`. |
| `offline` | `1` when the check was not allowed to use the network. |
| `packages_checked` | Packages sent to OSV. Zero when the check failed. |
| `open_count`, `new_count`, `resolved_count` | Counts after a check that compared lockfiles. On a failed check every counter is zero. When nothing was evaluated, `open_count` is the number of findings already open and the other two counters are zero. |
| `evaluated` | `1` when lockfile results, end-of-life data, or Play deadlines were compared with stored findings. `0` when none of them was. |
| `error` | Why the check failed, or null. Null together with `evaluated` 0 means nothing was compared. |

### findings

One row is unique on project id, ecosystem, package name, and vulnerability
id. The version is the last version that was still affected. It is not part
of the identity, so a new affected version updates the same row.

`state` is `open`, `ignored`, or `resolved`. `kind` is `vulnerability`,
`malicious`, `unmaintained`, `unsound`, `notice`, `eol`, `eol_soon`,
`platform_deadline`, or `platform_deadline_soon`. The database does not
enforce those lists. Craftward rejects any other word when it reads the row.
`aliases` and `fixed_versions`
are JSON arrays of strings. `ignore_until` is `YYYY-MM-DD` or null. The
other times use the same UTC timestamp as `runs`.

### finding_events

One row per change: `discovered`, `ignored`, `reopened`, `resolved`,
`reappeared`, `withdrawn`, or `deadline_passed`. `reason` is the ignore
text, `ignore expired on YYYY-MM-DD` when an ignore lapses, or an empty
string. `deadline_passed` has an empty reason.
