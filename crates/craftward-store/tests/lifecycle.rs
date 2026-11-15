//! Finding lifecycle and schema migration.

use std::fs;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use craftward_core::{
    Finding, FindingState, LockfileReport, NewFinding, ProjectReport, SCHEMA_VERSION, StateFilter,
    StateStore, apply_reports, ignore_finding, list_findings, render_ledger_text, reopen_finding,
    show_finding,
};
use craftward_store::SqliteStore;
use rusqlite::Connection;

const NOW: i64 = 0;
const LATER: i64 = 86_400;

fn scratch(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let path = std::env::temp_dir().join(format!("craftward-store-{name}-{nanos}"));
    fs::create_dir_all(&path).unwrap();
    path
}

fn finding(version: &str) -> Finding {
    Finding {
        ecosystem: "crates.io".to_owned(),
        package: "instant".to_owned(),
        version: version.to_owned(),
        id: "RUSTSEC-2024-0384".to_owned(),
        aliases: vec!["CVE-1".to_owned()],
        severity: "high".to_owned(),
        summary: "unmaintained".to_owned(),
        fixed_versions: vec!["0.1.13".to_owned()],
        url: "https://osv.dev/vulnerability/RUSTSEC-2024-0384".to_owned(),
        kind: "vulnerability".to_owned(),
    }
}

fn eol_hit(version: &str, cycle: &str) -> Finding {
    Finding {
        ecosystem: "endoflife".to_owned(),
        package: "postgresql".to_owned(),
        version: version.to_owned(),
        id: format!("EOL:postgresql:{cycle}"),
        aliases: Vec::new(),
        severity: "high".to_owned(),
        summary: "support ended".to_owned(),
        fixed_versions: Vec::new(),
        url: "https://endoflife.date/postgresql".to_owned(),
        kind: "eol".to_owned(),
    }
}

fn report(findings: Vec<Finding>, error: Option<&str>) -> ProjectReport {
    ProjectReport {
        id: "demo".to_owned(),
        path: "demo/craftward.json".to_owned(),
        lockfiles: vec![LockfileReport {
            path: "Cargo.lock".to_owned(),
            kind: "cargo".to_owned(),
            queried: 1,
            skipped: 0,
        }],
        findings,
        warnings: Vec::new(),
        error: error.map(str::to_owned),
        stale_cache_from: None,
        withdrawn: Vec::new(),
        lockfiles_checked: true,
        eol_checked: false,
        deadlines_checked: false,
        target_sdk_note: None,
    }
}

fn report_without_lockfiles() -> ProjectReport {
    ProjectReport {
        id: "demo".to_owned(),
        path: "demo/craftward.json".to_owned(),
        lockfiles: Vec::new(),
        findings: Vec::new(),
        warnings: Vec::new(),
        error: None,
        stale_cache_from: None,
        withdrawn: Vec::new(),
        lockfiles_checked: false,
        eol_checked: false,
        deadlines_checked: false,
        target_sdk_note: None,
    }
}

fn apply(
    store: &mut SqliteStore,
    findings: Vec<Finding>,
    at: i64,
) -> craftward_core::ProjectLedger {
    apply_reports(store, &[report(findings, None)], at, at, false)
        .unwrap()
        .remove(0)
}

#[test]
fn a_finding_is_discovered_updated_resolved_and_reopened() {
    let mut store = SqliteStore::open_in_memory().unwrap();
    let first = apply(&mut store, vec![finding("0.1.12")], NOW);
    assert_eq!(first.open.len(), 1);
    assert!(first.open[0].new);
    assert_eq!(first.open[0].record_id, 1);
    let second = apply(&mut store, vec![finding("0.1.12")], NOW + 10);
    assert!(!second.open[0].new);
    assert_eq!(second.open[0].record_id, 1);
    let changed = apply(&mut store, vec![finding("0.1.13")], NOW + 20);
    assert_eq!(changed.open[0].version, "0.1.13");
    assert_eq!(changed.open[0].record_id, 1);
    let gone = apply(&mut store, Vec::new(), NOW + 30);
    assert_eq!(gone.open.len(), 0);
    assert_eq!(gone.resolved_count, 1);
    let back = apply(&mut store, vec![finding("0.1.12")], NOW + 40);
    assert_eq!(back.open[0].record_id, 1);
    assert!(!back.open[0].new);
    let (_, events) = show_finding(&mut store, 1).unwrap();
    let kinds: Vec<_> = events.iter().map(|event| event.kind.as_str()).collect();
    assert_eq!(kinds, ["discovered", "resolved", "reappeared"]);
}

#[test]
fn ignore_expiry_reopen_and_a_failed_run() {
    let mut store = SqliteStore::open_in_memory().unwrap();
    apply(&mut store, vec![finding("0.1.12")], NOW);
    ignore_finding(&mut store, 1, "  test  ", Some("1970-01-01"), NOW).unwrap();
    let held = apply(&mut store, vec![finding("0.1.12")], NOW);
    assert_eq!(held.open.len(), 0);
    assert_eq!(held.ignored.len(), 1);
    let expired = apply(&mut store, vec![finding("0.1.12")], LATER);
    assert_eq!(expired.open.len(), 1);
    let (_, events) = show_finding(&mut store, 1).unwrap();
    assert!(
        events
            .iter()
            .any(|event| event.kind == "reopened" && event.reason.contains("expired"))
    );
    ignore_finding(&mut store, 1, "later", None, LATER).unwrap();
    reopen_finding(&mut store, 1, LATER).unwrap();
    let open = list_findings(&mut store, &["demo".to_owned()], StateFilter::Open, None).unwrap();
    assert_eq!(open.len(), 1);
    let error = apply_reports(
        &mut store,
        &[report(Vec::new(), Some("lockfile not found"))],
        LATER,
        LATER,
        true,
    )
    .unwrap();
    assert!(error[0].error.is_some());
    let still = show_finding(&mut store, 1).unwrap().0;
    assert_eq!(still.state, craftward_core::FindingState::Open);
    assert!(reopen_finding(&mut store, 1, LATER).is_err());
    assert!(ignore_finding(&mut store, 9, "x", None, LATER).is_err());
    assert!(ignore_finding(&mut store, 1, "   ", None, LATER).is_err());
}

#[test]
fn removing_lockfiles_leaves_an_open_finding_unevaluated() {
    let root = scratch("unevaluated");
    let path = root.join("craftward.db");
    let mut store = SqliteStore::open(&path).unwrap();
    apply(&mut store, vec![finding("0.1.12")], NOW);
    let ledger = apply_reports(
        &mut store,
        &[report_without_lockfiles()],
        LATER,
        LATER,
        false,
    )
    .unwrap()
    .remove(0);
    assert!(!ledger.evaluated);
    assert!(ledger.error.is_none());
    assert_eq!(ledger.resolved_count, 0);
    assert_eq!(ledger.open.len(), 1);
    assert!(!ledger.open[0].new);
    let (stored, events) = show_finding(&mut store, 1).unwrap();
    assert_eq!(stored.state, FindingState::Open);
    assert_eq!(stored.version, "0.1.12");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, "discovered");
    let text = render_ledger_text(std::slice::from_ref(&ledger), false);
    assert!(text.contains("no lockfiles configured"), "{text}");
    assert!(
        text.contains("existing findings are unchanged until lockfiles are configured"),
        "{text}"
    );
    drop(store);
    let connection = Connection::open(&path).unwrap();
    let (evaluated, error, open_count): (i64, Option<String>, i64) = connection
        .query_row(
            "SELECT evaluated, error, open_count FROM runs ORDER BY id DESC LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(evaluated, 0);
    assert!(error.is_none());
    assert_eq!(open_count, 1);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn file_database_round_trip_and_pragmas() {
    let root = scratch("file");
    let path = root.join("craftward.db");
    {
        let mut store = SqliteStore::open(&path).unwrap();
        apply(&mut store, vec![finding("0.1.12")], NOW);
    }
    let mut store = SqliteStore::open(&path).unwrap();
    let rows = list_findings(&mut store, &[], StateFilter::All, None).unwrap();
    assert_eq!(rows.len(), 1);
    let connection = Connection::open(&path).unwrap();
    let journal: String = connection
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .unwrap();
    let foreign_keys: i64 = connection
        .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
        .unwrap();
    let busy: i64 = connection
        .query_row("PRAGMA busy_timeout", [], |row| row.get(0))
        .unwrap();
    let version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert!(journal.eq_ignore_ascii_case("wal"), "{journal}");
    assert_eq!(foreign_keys, 1);
    assert_eq!(busy, 5000);
    assert_eq!(version, SCHEMA_VERSION);
    assert!(!root.join("craftward.db.bak-v0").exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn migrations_backup_and_reject_a_newer_file() {
    let root = scratch("migrate");
    let path = root.join("craftward.db");
    let connection = Connection::open(&path).unwrap();
    connection
        .execute_batch("CREATE TABLE previous (id INTEGER);")
        .unwrap();
    connection.close().unwrap();
    SqliteStore::open(&path).unwrap();
    assert!(root.join("craftward.db.bak-v0").exists());
    let version: i64 = Connection::open(&path)
        .unwrap()
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, SCHEMA_VERSION);

    let again = root.join("old.db");
    let connection = Connection::open(&again).unwrap();
    connection
        .execute_batch("CREATE TABLE previous (id INTEGER);")
        .unwrap();
    connection.close().unwrap();
    fs::write(root.join("old.db.bak-v0"), "keep").unwrap();
    let Err(error) = SqliteStore::open(&again) else {
        panic!("an existing backup must stop the migration");
    };
    assert!(
        error.to_string().contains("backup already exists"),
        "{error}"
    );
    let still: i64 = Connection::open(&again)
        .unwrap()
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(still, 0);

    let newer = root.join("newer.db");
    let connection = Connection::open(&newer).unwrap();
    connection
        .pragma_update(None, "user_version", SCHEMA_VERSION + 1)
        .unwrap();
    connection.close().unwrap();
    let Err(error) = SqliteStore::open(&newer) else {
        panic!("a newer database must be rejected");
    };
    assert!(error.to_string().contains("newer Craftward"), "{error}");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_version_1_file_records_evaluated_without_replacing_the_backup() {
    let root = scratch("v1");
    let path = root.join("craftward.db");
    let connection = Connection::open(&path).unwrap();
    connection
        .execute_batch(
            "CREATE TABLE runs (
                id INTEGER PRIMARY KEY,
                project_id TEXT NOT NULL,
                started_at TEXT NOT NULL,
                finished_at TEXT NOT NULL,
                offline INTEGER NOT NULL,
                packages_checked INTEGER NOT NULL,
                open_count INTEGER NOT NULL,
                new_count INTEGER NOT NULL,
                resolved_count INTEGER NOT NULL,
                error TEXT
            );
            INSERT INTO runs (
                project_id, started_at, finished_at, offline, packages_checked,
                open_count, new_count, resolved_count, error
            ) VALUES
                ('demo', '1970-01-01T00:00:00Z', '1970-01-01T00:00:00Z', 0, 1, 1, 1, 0, NULL),
                ('demo', '1970-01-01T00:00:00Z', '1970-01-01T00:00:00Z', 0, 0, 0, 0, 0, 'missing');
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
                kind TEXT NOT NULL CHECK (kind IN (
                    'discovered', 'ignored', 'reopened', 'resolved', 'reappeared'
                )),
                at TEXT NOT NULL,
                reason TEXT NOT NULL
            );",
        )
        .unwrap();
    connection
        .pragma_update(None, "user_version", 1_i64)
        .unwrap();
    connection.close().unwrap();
    SqliteStore::open(&path).unwrap();
    assert!(root.join("craftward.db.bak-v1").exists());
    let connection = Connection::open(&path).unwrap();
    let version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, SCHEMA_VERSION);
    let rows: Vec<(i64, Option<String>)> = connection
        .prepare("SELECT evaluated, error FROM runs ORDER BY id")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .map(|row| row.unwrap())
        .collect();
    assert_eq!(rows, vec![(1, None), (0, Some("missing".to_owned()))]);
    connection.close().unwrap();
    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_withdrawn_advisory_resolves_the_stored_finding() {
    let mut store = SqliteStore::open_in_memory().unwrap();
    apply(&mut store, vec![finding("0.1.12")], NOW);
    let mut second = report(Vec::new(), None);
    second.withdrawn.push("RUSTSEC-2024-0384".to_owned());
    let ledger = apply_reports(&mut store, &[second], NOW, NOW, false)
        .unwrap()
        .remove(0);
    assert_eq!(ledger.open.len(), 0);
    assert_eq!(ledger.resolved_count, 1);
    let (stored, events) = show_finding(&mut store, 1).unwrap();
    assert_eq!(stored.state, FindingState::Resolved);
    assert_eq!(events.last().unwrap().kind, "withdrawn");
    assert!(!events.iter().any(|event| event.kind == "resolved"));
}

#[test]
fn an_end_of_life_version_change_leaves_lockfile_findings() {
    let mut store = SqliteStore::open_in_memory().unwrap();
    apply(&mut store, vec![finding("0.1.12")], NOW);
    let mut eol_run = report(vec![eol_hit("9.3.0", "9.3")], None);
    eol_run.lockfiles_checked = false;
    eol_run.eol_checked = true;
    apply_reports(&mut store, &[eol_run], NOW, NOW, false).unwrap();
    let mut moved = report(vec![eol_hit("16.0.0", "16")], None);
    moved.lockfiles_checked = false;
    moved.eol_checked = true;
    let ledger = apply_reports(&mut store, &[moved], LATER, LATER, false)
        .unwrap()
        .remove(0);
    assert_eq!(ledger.resolved_count, 1);
    assert_eq!(ledger.open.len(), 1);
    assert_eq!(ledger.open[0].vulnerability_id, "EOL:postgresql:16");
    let rows = list_findings(&mut store, &[], StateFilter::Open, None).unwrap();
    assert!(
        rows.iter()
            .any(|row| row.vulnerability_id == "RUSTSEC-2024-0384")
    );
}

#[test]
fn a_version_2_file_gains_kind_and_keeps_a_backup() {
    let root = scratch("v2");
    let path = root.join("craftward.db");
    let connection = Connection::open(&path).unwrap();
    connection
        .execute_batch(
            "CREATE TABLE runs (
                id INTEGER PRIMARY KEY,
                project_id TEXT NOT NULL,
                started_at TEXT NOT NULL,
                finished_at TEXT NOT NULL,
                offline INTEGER NOT NULL,
                packages_checked INTEGER NOT NULL,
                open_count INTEGER NOT NULL,
                new_count INTEGER NOT NULL,
                resolved_count INTEGER NOT NULL,
                error TEXT,
                evaluated INTEGER NOT NULL DEFAULT 0
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
                state TEXT NOT NULL,
                first_seen TEXT NOT NULL,
                last_seen TEXT NOT NULL,
                resolved_at TEXT,
                ignore_reason TEXT,
                ignore_until TEXT
            );
            INSERT INTO findings (
                project_id, ecosystem, package_name, vulnerability_id, version, severity,
                summary, aliases, fixed_versions, state, first_seen, last_seen
            ) VALUES (
                'demo', 'crates.io', 'instant', 'RUSTSEC-1', '0.1.12', 'high',
                'old', '[]', '[]', 'open', '1970-01-01T00:00:00Z', '1970-01-01T00:00:00Z'
            );
            CREATE TABLE finding_events (
                id INTEGER PRIMARY KEY,
                finding_id INTEGER NOT NULL REFERENCES findings(id),
                kind TEXT NOT NULL,
                at TEXT NOT NULL,
                reason TEXT NOT NULL
            );
            INSERT INTO finding_events (finding_id, kind, at, reason)
                VALUES (1, 'discovered', '1970-01-01T00:00:00Z', '');",
        )
        .unwrap();
    connection
        .pragma_update(None, "user_version", 2_i64)
        .unwrap();
    connection.close().unwrap();
    SqliteStore::open(&path).unwrap();
    assert!(root.join("craftward.db.bak-v2").exists());
    let connection = Connection::open(&path).unwrap();
    let version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, SCHEMA_VERSION);
    let kind: String = connection
        .query_row("SELECT kind FROM findings WHERE id = 1", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(kind, "vulnerability");
    connection
        .execute(
            "INSERT INTO finding_events (finding_id, kind, at, reason) VALUES (1, 'withdrawn', '1970-01-02T00:00:00Z', '')",
            [],
        )
        .unwrap();
    connection.close().unwrap();
    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_version_3_file_accepts_platform_deadlines_and_keeps_a_backup() {
    let root = scratch("v3");
    let path = root.join("craftward.db");
    let connection = Connection::open(&path).unwrap();
    connection
        .execute_batch(
            "CREATE TABLE runs (
                id INTEGER PRIMARY KEY,
                project_id TEXT NOT NULL,
                started_at TEXT NOT NULL,
                finished_at TEXT NOT NULL,
                offline INTEGER NOT NULL,
                packages_checked INTEGER NOT NULL,
                open_count INTEGER NOT NULL,
                new_count INTEGER NOT NULL,
                resolved_count INTEGER NOT NULL,
                error TEXT,
                evaluated INTEGER NOT NULL DEFAULT 1
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
                state TEXT NOT NULL,
                first_seen TEXT NOT NULL,
                last_seen TEXT NOT NULL,
                resolved_at TEXT,
                ignore_reason TEXT,
                ignore_until TEXT,
                kind TEXT NOT NULL DEFAULT 'vulnerability'
            );
            INSERT INTO findings (
                project_id, ecosystem, package_name, vulnerability_id, version, severity,
                summary, aliases, fixed_versions, state, first_seen, last_seen, kind
            ) VALUES (
                'demo', 'endoflife', 'postgresql', 'EOL:postgresql:16', '16', 'high',
                'old', '[]', '[]', 'open', '1970-01-01T00:00:00Z', '1970-01-01T00:00:00Z', 'eol'
            );
            CREATE TABLE finding_events (
                id INTEGER PRIMARY KEY,
                finding_id INTEGER NOT NULL REFERENCES findings(id),
                kind TEXT NOT NULL,
                at TEXT NOT NULL,
                reason TEXT NOT NULL
            );
            INSERT INTO finding_events (finding_id, kind, at, reason)
                VALUES (1, 'discovered', '1970-01-01T00:00:00Z', '');",
        )
        .unwrap();
    connection
        .pragma_update(None, "user_version", 3_i64)
        .unwrap();
    connection.close().unwrap();
    SqliteStore::open(&path).unwrap();
    assert!(root.join("craftward.db.bak-v3").exists());
    let connection = Connection::open(&path).unwrap();
    let kind: String = connection
        .query_row(
            "SELECT kind FROM findings WHERE vulnerability_id = 'EOL:postgresql:16'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(kind, "eol");
    connection
        .execute(
            "INSERT INTO findings (
                project_id, ecosystem, package_name, vulnerability_id, version, severity,
                summary, aliases, fixed_versions, state, kind, first_seen, last_seen
            ) VALUES (
                'demo', 'platform', 'google-play', 'PLAY:phone:2026-08-31:existing_apps',
                '30', 'high', 'late', '[]', '[]', 'open', 'platform_deadline',
                '1970-01-01T00:00:00Z', '1970-01-01T00:00:00Z'
            )",
            [],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO finding_events (finding_id, kind, at, reason)
             VALUES (1, 'deadline_passed', '1970-01-01T00:00:00Z', '')",
            [],
        )
        .unwrap();
    connection.close().unwrap();
    let _ = fs::remove_dir_all(root);
}

fn play(kind: &str, summary: &str, version: &str) -> Finding {
    Finding {
        ecosystem: "platform".to_owned(),
        package: "google-play".to_owned(),
        version: version.to_owned(),
        id: "PLAY:phone:new_apps_and_updates".to_owned(),
        aliases: Vec::new(),
        severity: if kind == "platform_deadline" {
            "high"
        } else {
            "medium"
        }
        .to_owned(),
        summary: summary.to_owned(),
        fixed_versions: Vec::new(),
        url: "https://example.com/play".to_owned(),
        kind: kind.to_owned(),
    }
}

fn deadline_report(findings: Vec<Finding>) -> ProjectReport {
    let mut report = report(findings, None);
    report.lockfiles.clear();
    report.lockfiles_checked = false;
    report.deadlines_checked = true;
    report
}

#[test]
fn a_deadline_that_arrives_keeps_the_same_finding() {
    let root = scratch("passed");
    let mut store = SqliteStore::open(&root.join("craftward.db")).unwrap();
    let soon = play(
        "platform_deadline_soon",
        "targetSdk 30 is below required 36 for new apps and updates (deadline 2027-02-01)",
        "30",
    );
    let first = apply_reports(&mut store, &[deadline_report(vec![soon])], NOW, NOW, true)
        .unwrap()
        .remove(0);
    let id = first.open[0].record_id;
    assert!(first.open[0].new);
    let passed = play(
        "platform_deadline",
        "targetSdk 30 is below required 36 for new apps and updates (deadline 2027-02-01)",
        "30",
    );
    let second = apply_reports(
        &mut store,
        &[deadline_report(vec![passed])],
        LATER,
        LATER,
        true,
    )
    .unwrap()
    .remove(0);
    assert_eq!(second.open.len(), 1);
    assert_eq!(second.open[0].record_id, id);
    assert!(!second.open[0].new);
    assert_eq!(second.open[0].kind, "platform_deadline");
    assert_eq!(second.open[0].severity, "high");
    let (_finding, events) = show_finding(&mut store, id).unwrap();
    assert!(
        events.iter().any(|event| event.kind == "deadline_passed"),
        "{events:?}"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_higher_requirement_updates_the_finding_until_the_sdk_meets_it() {
    let root = scratch("raised");
    let mut store = SqliteStore::open(&root.join("craftward.db")).unwrap();
    let first = apply_reports(
        &mut store,
        &[deadline_report(vec![play(
            "platform_deadline",
            "required 35 (deadline 2025-08-31)",
            "30",
        )])],
        NOW,
        NOW,
        true,
    )
    .unwrap()
    .remove(0);
    let id = first.open[0].record_id;
    let second = apply_reports(
        &mut store,
        &[deadline_report(vec![play(
            "platform_deadline",
            "required 36 (deadline 2026-08-31)",
            "30",
        )])],
        LATER,
        LATER,
        true,
    )
    .unwrap()
    .remove(0);
    assert_eq!(second.open.len(), 1);
    assert_eq!(second.open[0].record_id, id);
    assert!(!second.open[0].new);
    assert!(second.open[0].summary.contains("required 36"));
    let still = apply_reports(
        &mut store,
        &[deadline_report(vec![play(
            "platform_deadline",
            "required 36 (deadline 2026-08-31)",
            "35",
        )])],
        LATER + 1,
        LATER + 1,
        true,
    )
    .unwrap()
    .remove(0);
    assert_eq!(still.open[0].record_id, id);
    assert_eq!(still.resolved_count, 0);
    let cleared = apply_reports(
        &mut store,
        &[deadline_report(Vec::new())],
        LATER + 2,
        LATER + 2,
        true,
    )
    .unwrap()
    .remove(0);
    assert_eq!(cleared.open.len(), 0);
    assert_eq!(cleared.resolved_count, 1);
    let rows = list_findings(&mut store, &[], StateFilter::All, None).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].state, FindingState::Resolved);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_backup_includes_committed_wal_frames() {
    let root = scratch("wal");
    let path = root.join("craftward.db");
    let writer = Connection::open(&path).unwrap();
    writer.pragma_update(None, "journal_mode", "WAL").unwrap();
    writer.pragma_update(None, "user_version", 1_i64).unwrap();
    writer
        .execute_batch(
            "CREATE TABLE runs (
                id INTEGER PRIMARY KEY,
                project_id TEXT NOT NULL,
                started_at TEXT NOT NULL,
                finished_at TEXT NOT NULL,
                offline INTEGER NOT NULL,
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
                state TEXT NOT NULL,
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
                kind TEXT NOT NULL,
                at TEXT NOT NULL,
                reason TEXT NOT NULL
            );",
        )
        .unwrap();
    writer
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .unwrap();
    writer
        .pragma_update(None, "wal_autocheckpoint", 0_i64)
        .unwrap();
    writer
        .execute(
            "INSERT INTO findings (
                project_id, ecosystem, package_name, vulnerability_id, version, severity,
                summary, aliases, fixed_versions, state, first_seen, last_seen
            ) VALUES (
                'demo', 'crates.io', 'instant', 'RUSTSEC-WAL', '1', 'high',
                'wal-only-marker', '[]', '[]', 'open',
                '1970-01-01T00:00:00Z', '1970-01-01T00:00:00Z'
            )",
            [],
        )
        .unwrap();
    let main = fs::read(&path).unwrap();
    assert!(
        !main
            .windows(b"wal-only-marker".len())
            .any(|window| window == b"wal-only-marker"),
        "the marker must still live in the WAL, not the main file"
    );
    let wal = sibling(&path, "-wal");
    assert!(fs::metadata(&wal).unwrap().len() > 0);
    SqliteStore::open(&path).unwrap();
    let backup = sibling(&path, ".bak-v1");
    let saved =
        Connection::open_with_flags(&backup, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let integrity: String = saved
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .unwrap();
    assert_eq!(integrity, "ok");
    let summary: String = saved
        .query_row(
            "SELECT summary FROM findings WHERE vulnerability_id = 'RUSTSEC-WAL'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(summary, "wal-only-marker");
    drop(writer);
    let _ = fs::remove_dir_all(root);
}

fn sibling(path: &std::path::Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().unwrap().to_os_string();
    name.push(suffix);
    path.with_file_name(name)
}

#[test]
fn two_writers_on_one_file_both_finish() {
    let root = scratch("parallel");
    let path = root.join("craftward.db");
    SqliteStore::open(&path).unwrap();
    let (started, waiting) = mpsc::channel();
    let held = path.clone();
    let holder = thread::spawn(move || {
        let mut store = SqliteStore::open(&held).unwrap();
        store
            .in_transaction(|session| {
                started.send(()).unwrap();
                thread::sleep(Duration::from_millis(400));
                session.insert_finding(&NewFinding {
                    project_id: "demo".to_owned(),
                    ecosystem: "crates.io".to_owned(),
                    package: "held".to_owned(),
                    vulnerability_id: "RUSTSEC-HELD".to_owned(),
                    version: "1".to_owned(),
                    severity: "low".to_owned(),
                    summary: "held".to_owned(),
                    aliases: Vec::new(),
                    fixed_versions: Vec::new(),
                    kind: "notice".to_owned(),
                    seen_at: "1970-01-01T00:00:00Z".to_owned(),
                })?;
                Ok(())
            })
            .unwrap();
    });
    waiting.recv().unwrap();
    let began = Instant::now();
    let mut store = SqliteStore::open(&path).unwrap();
    apply(&mut store, vec![finding("0.1.12")], NOW);
    let waited = began.elapsed();
    holder.join().unwrap();
    assert!(
        waited >= Duration::from_millis(250),
        "the second writer returned in {waited:?}"
    );
    let rows = list_findings(&mut store, &[], StateFilter::All, None).unwrap();
    assert_eq!(rows.len(), 2, "{rows:?}");
    let _ = fs::remove_dir_all(root);
}
