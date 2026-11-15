//! SQLite database for Craftward findings.
//!
//! This is the only crate that links SQLite. The schema and the lifecycle
//! live in `craftward-core`.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use craftward_core::{
    FindingState, NewFinding, NewRun, SCHEMA_VERSION, StateFilter, StateStore, StoreError,
    StoreSession, StoredEvent, StoredFinding,
};
use rusqlite::{Connection, OpenFlags, Transaction};

const BUSY_TIMEOUT_MS: u64 = 5000;

const MIGRATION_V1: &str = "
CREATE TABLE runs (
    id INTEGER PRIMARY KEY,
    project_id TEXT NOT NULL,
    started_at TEXT NOT NULL,
    finished_at TEXT NOT NULL,
    offline INTEGER NOT NULL CHECK (offline IN (0, 1)),
    packages_checked INTEGER NOT NULL,
    open_count INTEGER NOT NULL,
    new_count INTEGER NOT NULL,
    resolved_count INTEGER NOT NULL,
    error TEXT
);
CREATE TABLE findings (
    id INTEGER PRIMARY KEY,
    project_id TEXT NOT NULL,
    ecosystem TEXT NOT NULL,
    package_name TEXT NOT NULL,
    vulnerability_id TEXT NOT NULL,
    version TEXT NOT NULL,
    severity TEXT NOT NULL,
    summary TEXT NOT NULL,
    aliases TEXT NOT NULL,
    fixed_versions TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('open', 'ignored', 'resolved')),
    first_seen TEXT NOT NULL,
    last_seen TEXT NOT NULL,
    resolved_at TEXT,
    ignore_reason TEXT,
    ignore_until TEXT,
    UNIQUE (project_id, ecosystem, package_name, vulnerability_id)
);
CREATE TABLE finding_events (
    id INTEGER PRIMARY KEY,
    finding_id INTEGER NOT NULL REFERENCES findings(id),
    kind TEXT NOT NULL CHECK (kind IN ('discovered', 'ignored', 'reopened', 'resolved', 'reappeared')),
    at TEXT NOT NULL,
    reason TEXT NOT NULL
);
CREATE INDEX finding_events_finding ON finding_events(finding_id);
";

const MIGRATION_V2: &str = "
ALTER TABLE runs ADD COLUMN evaluated INTEGER NOT NULL DEFAULT 0 CHECK (evaluated IN (0, 1));
UPDATE runs SET evaluated = 1 WHERE error IS NULL;
";

const FINDING_COLUMNS: &str = "
id, project_id, ecosystem, package_name, vulnerability_id, version, severity,
summary, aliases, fixed_versions, state, first_seen, last_seen, resolved_at,
ignore_reason, ignore_until
";

/// A SQLite file, or an in-memory database for tests.
pub struct SqliteStore {
    conn: Connection,
}

impl SqliteStore {
    /// Opens or creates the database file and applies pending migrations.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::NewerDatabase`] when the file is newer than this
    /// build, [`StoreError::BackupExists`] when the migration backup is
    /// already present, and [`StoreError::Failed`] when the file cannot be
    /// read or written.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).map_err(|error| StoreError::Failed(error.to_string()))?;
        }
        let current = stored_version(path)?;
        if let Some(version) = current
            && version > SCHEMA_VERSION
        {
            return Err(StoreError::NewerDatabase {
                found: version,
                supported: SCHEMA_VERSION,
            });
        }
        if let Some(version) = current
            && version < SCHEMA_VERSION
        {
            copy_backup(path, version)?;
        }
        let mut conn = Connection::open(path).map_err(|error| failed(&error))?;
        configure(&conn, true)?;
        if current.unwrap_or(0) < SCHEMA_VERSION {
            migrate(&mut conn, current.unwrap_or(0))?;
        }
        Ok(Self { conn })
    }

    /// Opens a private in-memory database at schema version 1.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Failed`] when SQLite cannot create it.
    pub fn open_in_memory() -> Result<Self, StoreError> {
        let mut conn = Connection::open_in_memory().map_err(|error| failed(&error))?;
        configure(&conn, false)?;
        migrate(&mut conn, 0)?;
        Ok(Self { conn })
    }
}

impl StateStore for SqliteStore {
    fn in_transaction<T>(
        &mut self,
        body: impl FnOnce(&mut dyn StoreSession) -> Result<T, StoreError>,
    ) -> Result<T, StoreError> {
        let transaction = self.conn.transaction().map_err(|error| failed(&error))?;
        let value = {
            let mut session = Session {
                transaction: &transaction,
            };
            body(&mut session)?
        };
        transaction.commit().map_err(|error| failed(&error))?;
        Ok(value)
    }
}

struct Session<'a> {
    transaction: &'a Transaction<'a>,
}

impl StoreSession for Session<'_> {
    fn findings_for_project(&mut self, project_id: &str) -> Result<Vec<StoredFinding>, StoreError> {
        query_findings(
            self.transaction,
            &format!("SELECT {FINDING_COLUMNS} FROM findings WHERE project_id = ?1 ORDER BY id"),
            Some(project_id),
        )
    }

    fn insert_finding(&mut self, finding: &NewFinding) -> Result<i64, StoreError> {
        self.transaction
            .execute(
                "INSERT INTO findings (
                    project_id, ecosystem, package_name, vulnerability_id, version, severity,
                    summary, aliases, fixed_versions, state, first_seen, last_seen
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'open', ?10, ?10)",
                (
                    &finding.project_id,
                    &finding.ecosystem,
                    &finding.package,
                    &finding.vulnerability_id,
                    &finding.version,
                    &finding.severity,
                    &finding.summary,
                    encode_list(&finding.aliases)?,
                    encode_list(&finding.fixed_versions)?,
                    &finding.seen_at,
                ),
            )
            .map_err(|error| failed(&error))?;
        Ok(self.transaction.last_insert_rowid())
    }

    fn update_finding(&mut self, finding: &StoredFinding) -> Result<(), StoreError> {
        self.transaction
            .execute(
                "UPDATE findings SET
                    version = ?1, severity = ?2, summary = ?3, aliases = ?4, fixed_versions = ?5,
                    state = ?6, last_seen = ?7, resolved_at = ?8, ignore_reason = ?9, ignore_until = ?10
                 WHERE id = ?11",
                (
                    &finding.version,
                    &finding.severity,
                    &finding.summary,
                    encode_list(&finding.aliases)?,
                    encode_list(&finding.fixed_versions)?,
                    finding.state.as_str(),
                    &finding.last_seen,
                    &finding.resolved_at,
                    &finding.ignore_reason,
                    &finding.ignore_until,
                    finding.id,
                ),
            )
            .map_err(|error| failed(&error))?;
        Ok(())
    }

    fn insert_event(
        &mut self,
        finding_id: i64,
        kind: &str,
        at: &str,
        reason: &str,
    ) -> Result<(), StoreError> {
        self.transaction
            .execute(
                "INSERT INTO finding_events (finding_id, kind, at, reason) VALUES (?1, ?2, ?3, ?4)",
                (finding_id, kind, at, reason),
            )
            .map_err(|error| failed(&error))?;
        Ok(())
    }

    fn insert_run(&mut self, run: &NewRun) -> Result<(), StoreError> {
        self.transaction
            .execute(
                "INSERT INTO runs (
                    project_id, started_at, finished_at, offline, packages_checked,
                    open_count, new_count, resolved_count, error, evaluated
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                (
                    &run.project_id,
                    &run.started_at,
                    &run.finished_at,
                    i64::from(u8::from(run.offline)),
                    run.packages_checked,
                    run.open_count,
                    run.new_count,
                    run.resolved_count,
                    &run.error,
                    i64::from(u8::from(run.evaluated)),
                ),
            )
            .map_err(|error| failed(&error))?;
        Ok(())
    }

    fn list_findings(
        &mut self,
        projects: &[String],
        filter: StateFilter,
    ) -> Result<Vec<StoredFinding>, StoreError> {
        let mut rows = query_findings(
            self.transaction,
            &format!("SELECT {FINDING_COLUMNS} FROM findings ORDER BY id"),
            None,
        )?;
        if let Some(state) = filter_state(filter) {
            rows.retain(|row| row.state == state);
        }
        if !projects.is_empty() {
            rows.retain(|row| projects.iter().any(|project| project == &row.project_id));
        }
        Ok(rows)
    }

    fn finding(&mut self, id: i64) -> Result<Option<StoredFinding>, StoreError> {
        let sql = format!("SELECT {FINDING_COLUMNS} FROM findings WHERE id = ?1");
        let mut statement = self
            .transaction
            .prepare(&sql)
            .map_err(|error| failed(&error))?;
        let mut rows = statement.query([id]).map_err(|error| failed(&error))?;
        rows.next()
            .map_err(|error| failed(&error))?
            .map(read_finding)
            .transpose()
    }

    fn events(&mut self, finding_id: i64) -> Result<Vec<StoredEvent>, StoreError> {
        let mut statement = self
            .transaction
            .prepare(
                "SELECT kind, at, reason FROM finding_events WHERE finding_id = ?1 ORDER BY id",
            )
            .map_err(|error| failed(&error))?;
        let mut rows = statement
            .query([finding_id])
            .map_err(|error| failed(&error))?;
        let mut events = Vec::new();
        while let Some(row) = rows.next().map_err(|error| failed(&error))? {
            events.push(StoredEvent {
                kind: row.get(0).map_err(|error| failed(&error))?,
                at: row.get(1).map_err(|error| failed(&error))?,
                reason: row.get(2).map_err(|error| failed(&error))?,
            });
        }
        Ok(events)
    }
}

fn query_findings(
    connection: &Connection,
    sql: &str,
    project_id: Option<&str>,
) -> Result<Vec<StoredFinding>, StoreError> {
    let mut statement = connection.prepare(sql).map_err(|error| failed(&error))?;
    let mut rows = if let Some(project_id) = project_id {
        statement
            .query([project_id])
            .map_err(|error| failed(&error))?
    } else {
        statement.query([]).map_err(|error| failed(&error))?
    };
    let mut findings = Vec::new();
    while let Some(row) = rows.next().map_err(|error| failed(&error))? {
        findings.push(read_finding(row)?);
    }
    Ok(findings)
}

fn read_finding(row: &rusqlite::Row<'_>) -> Result<StoredFinding, StoreError> {
    let state_text: String = row.get(10).map_err(|error| failed(&error))?;
    let state = FindingState::parse(&state_text)
        .ok_or_else(|| StoreError::Failed(format!("unknown finding state: {state_text}")))?;
    let aliases: String = row.get(8).map_err(|error| failed(&error))?;
    let fixed: String = row.get(9).map_err(|error| failed(&error))?;
    Ok(StoredFinding {
        id: row.get(0).map_err(|error| failed(&error))?,
        project_id: row.get(1).map_err(|error| failed(&error))?,
        ecosystem: row.get(2).map_err(|error| failed(&error))?,
        package: row.get(3).map_err(|error| failed(&error))?,
        vulnerability_id: row.get(4).map_err(|error| failed(&error))?,
        version: row.get(5).map_err(|error| failed(&error))?,
        severity: row.get(6).map_err(|error| failed(&error))?,
        summary: row.get(7).map_err(|error| failed(&error))?,
        aliases: decode_list(&aliases)?,
        fixed_versions: decode_list(&fixed)?,
        state,
        first_seen: row.get(11).map_err(|error| failed(&error))?,
        last_seen: row.get(12).map_err(|error| failed(&error))?,
        resolved_at: row.get(13).map_err(|error| failed(&error))?,
        ignore_reason: row.get(14).map_err(|error| failed(&error))?,
        ignore_until: row.get(15).map_err(|error| failed(&error))?,
    })
}

fn filter_state(filter: StateFilter) -> Option<FindingState> {
    match filter {
        StateFilter::Open => Some(FindingState::Open),
        StateFilter::Ignored => Some(FindingState::Ignored),
        StateFilter::Resolved => Some(FindingState::Resolved),
        StateFilter::All => None,
    }
}

fn configure(connection: &Connection, require_wal: bool) -> Result<(), StoreError> {
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .map_err(|error| failed(&error))?;
    connection
        .busy_timeout(Duration::from_millis(BUSY_TIMEOUT_MS))
        .map_err(|error| failed(&error))?;
    connection
        .pragma_update(
            None,
            "busy_timeout",
            i64::try_from(BUSY_TIMEOUT_MS).unwrap_or(5000),
        )
        .map_err(|error| failed(&error))?;
    let mode: String = connection
        .pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get(0))
        .map_err(|error| failed(&error))?;
    if require_wal && !mode.eq_ignore_ascii_case("wal") {
        return Err(StoreError::Failed(format!(
            "journal_mode is {mode}, expected wal"
        )));
    }
    Ok(())
}

fn stored_version(path: &Path) -> Result<Option<i64>, StoreError> {
    if !path.exists() {
        return Ok(None);
    }
    let length = fs::metadata(path)
        .map_err(|error| StoreError::Failed(error.to_string()))?
        .len();
    if length == 0 {
        return Ok(None);
    }
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|error| failed(&error))?;
    let version = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|error| failed(&error))?;
    Ok(Some(version))
}

fn copy_backup(path: &Path, version: i64) -> Result<(), StoreError> {
    let backup = backup_path(path, version)?;
    if backup.exists() {
        return Err(StoreError::BackupExists(backup));
    }
    fs::copy(path, &backup).map_err(|error| StoreError::Failed(error.to_string()))?;
    Ok(())
}

fn backup_path(path: &Path, version: i64) -> Result<PathBuf, StoreError> {
    let Some(name) = path.file_name() else {
        return Err(StoreError::Failed(
            "database path has no file name".to_owned(),
        ));
    };
    let mut backed = name.to_os_string();
    backed.push(format!(".bak-v{version}"));
    Ok(path.with_file_name(backed))
}

fn migrate(connection: &mut Connection, from: i64) -> Result<(), StoreError> {
    if from < 1 {
        apply_migration(connection, MIGRATION_V1, 1)?;
    }
    if from < 2 {
        apply_migration(connection, MIGRATION_V2, 2)?;
    }
    Ok(())
}

fn apply_migration(connection: &mut Connection, sql: &str, version: i64) -> Result<(), StoreError> {
    let transaction = connection.transaction().map_err(|error| failed(&error))?;
    transaction
        .execute_batch(sql)
        .map_err(|error| failed(&error))?;
    transaction
        .pragma_update(None, "user_version", version)
        .map_err(|error| failed(&error))?;
    transaction.commit().map_err(|error| failed(&error))?;
    Ok(())
}

fn encode_list(values: &[String]) -> Result<String, StoreError> {
    serde_json::to_string(values).map_err(|error| StoreError::Failed(error.to_string()))
}

fn decode_list(text: &str) -> Result<Vec<String>, StoreError> {
    serde_json::from_str(text).map_err(|error| StoreError::Failed(error.to_string()))
}

fn failed(error: &rusqlite::Error) -> StoreError {
    StoreError::Failed(error.to_string())
}
