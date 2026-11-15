//! How a radar run changes stored findings.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use serde::Serialize;

use crate::date::{format_iso8601, utc_day};
use crate::osv::severity_rank;
use crate::radar::{Finding, LockfileReport, ProjectReport};
use crate::store::{
    FindingState, NewFinding, NewRun, StateFilter, StateStore, StoreError, StoreSession,
    StoredEvent, StoredFinding,
};

/// One project after its findings have been stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectLedger {
    /// Project id.
    pub id: String,
    /// Project file.
    pub path: String,
    /// Lockfiles that were read.
    pub lockfiles: Vec<LockfileReport>,
    /// Warnings that did not fail the project.
    pub warnings: Vec<String>,
    /// Why this project could not be checked.
    pub error: Option<String>,
    /// Cache time used after a network failure.
    pub stale_cache_from: Option<String>,
    /// Open findings after the run, most severe first.
    pub open: Vec<LedgerFinding>,
    /// Ignored findings that were still present.
    pub ignored: Vec<LedgerFinding>,
    /// Findings marked resolved by this run.
    pub resolved_count: u32,
    /// Whether this run compared lockfile results with stored findings.
    pub evaluated: bool,
}

/// One finding in a ledger, with its database id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerFinding {
    /// Database id.
    pub record_id: i64,
    /// Whether this run inserted the row.
    pub new: bool,
    /// State after the run.
    pub state: FindingState,
    /// OSV ecosystem.
    pub ecosystem: String,
    /// Package name.
    pub package: String,
    /// Last seen version.
    pub version: String,
    /// OSV id.
    pub vulnerability_id: String,
    /// Aliases.
    pub aliases: Vec<String>,
    /// Severity word.
    pub severity: String,
    /// Short description.
    pub summary: String,
    /// Fixed versions.
    pub fixed_versions: Vec<String>,
    /// Page on osv.dev.
    pub url: String,
}

/// Applies every project report.
///
/// A failed project, and a project with no lockfiles, do not change findings.
///
/// # Errors
///
/// Returns [`StoreError`] when a transaction cannot be stored. Earlier
/// projects in `reports` stay committed.
pub fn apply_reports(
    store: &mut impl StateStore,
    reports: &[ProjectReport],
    started_at: i64,
    finished_at: i64,
    offline: bool,
) -> Result<Vec<ProjectLedger>, StoreError> {
    let started = format_iso8601(started_at)
        .ok_or_else(|| StoreError::Invalid("time is out of range".to_owned()))?;
    let finished = format_iso8601(finished_at)
        .ok_or_else(|| StoreError::Invalid("time is out of range".to_owned()))?;
    let today = utc_day(finished_at)
        .ok_or_else(|| StoreError::Invalid("time is out of range".to_owned()))?;
    let mut ledgers = Vec::new();
    for report in reports {
        ledgers.push(store.in_transaction(|session| {
            apply_one(session, report, &started, &finished, &today, offline)
        })?);
    }
    Ok(ledgers)
}

/// Ignores one finding. `reason` must contain text that is not only whitespace.
///
/// `until` is the last UTC day the ignore stays in force. `None` does not expire.
///
/// # Errors
///
/// Returns [`StoreError::NotFound`] when the id is unknown, and
/// [`StoreError::Invalid`] when the reason is blank.
pub fn ignore_finding(
    store: &mut impl StateStore,
    id: i64,
    reason: &str,
    until: Option<&str>,
    now: i64,
) -> Result<(), StoreError> {
    let reason = reason.trim();
    if reason.is_empty() {
        return Err(StoreError::Invalid(
            "ignore reason must not be empty".to_owned(),
        ));
    }
    let at = stamp(now)?;
    store.in_transaction(|session| {
        let mut finding = required(session, id)?;
        finding.state = FindingState::Ignored;
        finding.ignore_reason = Some(reason.to_owned());
        finding.ignore_until = until.map(str::to_owned);
        finding.resolved_at = None;
        session.update_finding(&finding)?;
        session.insert_event(id, "ignored", &at, reason)?;
        Ok(())
    })
}

/// Makes an ignored or resolved finding open again.
///
/// # Errors
///
/// Returns [`StoreError::NotFound`] when the id is unknown, and
/// [`StoreError::Invalid`] when it is already open.
pub fn reopen_finding(store: &mut impl StateStore, id: i64, now: i64) -> Result<(), StoreError> {
    let at = stamp(now)?;
    store.in_transaction(|session| {
        let mut finding = required(session, id)?;
        if finding.state == FindingState::Open {
            return Err(StoreError::Invalid(format!("finding {id} is already open")));
        }
        finding.state = FindingState::Open;
        finding.ignore_reason = None;
        finding.ignore_until = None;
        finding.resolved_at = None;
        session.update_finding(&finding)?;
        session.insert_event(id, "reopened", &at, "")?;
        Ok(())
    })
}

/// Lists findings. An empty `projects` slice means every project.
///
/// # Errors
///
/// Returns [`StoreError`] when the rows cannot be read.
pub fn list_findings(
    store: &mut impl StateStore,
    projects: &[String],
    filter: StateFilter,
) -> Result<Vec<StoredFinding>, StoreError> {
    let mut rows = store.in_transaction(|session| session.list_findings(projects, filter))?;
    rows.sort_by(|left, right| {
        left.project_id
            .cmp(&right.project_id)
            .then(severity_rank(&left.severity).cmp(&severity_rank(&right.severity)))
            .then(left.package.cmp(&right.package))
            .then(left.version.cmp(&right.version))
            .then(left.id.cmp(&right.id))
    });
    Ok(rows)
}

/// One finding and its history.
///
/// # Errors
///
/// Returns [`StoreError::NotFound`] when the id is unknown.
pub fn show_finding(
    store: &mut impl StateStore,
    id: i64,
) -> Result<(StoredFinding, Vec<StoredEvent>), StoreError> {
    store.in_transaction(|session| {
        let finding = required(session, id)?;
        let events = session.events(id)?;
        Ok((finding, events))
    })
}

fn apply_one(
    session: &mut dyn StoreSession,
    report: &ProjectReport,
    started: &str,
    finished: &str,
    today: &str,
    offline: bool,
) -> Result<ProjectLedger, StoreError> {
    if let Some(error) = &report.error {
        session.insert_run(&NewRun {
            project_id: report.id.clone(),
            started_at: started.to_owned(),
            finished_at: finished.to_owned(),
            offline,
            packages_checked: 0,
            open_count: 0,
            new_count: 0,
            resolved_count: 0,
            evaluated: false,
            error: Some(error.clone()),
        })?;
        return Ok(empty_ledger(report));
    }
    if report.lockfiles.is_empty() && report.warnings.is_empty() {
        return record_unevaluated(session, report, started, finished, offline);
    }
    let mut rows = session.findings_for_project(&report.id)?;
    let mut index = index_rows(&rows);
    let mut seen = BTreeSet::new();
    let mut new_ids = BTreeSet::new();
    for finding in unique_observed(&report.findings) {
        let key = observed_key(finding);
        seen.insert(key.clone());
        if let Some(position) = index.get(&key).copied() {
            touch_known(session, &mut rows[position], finding, finished, today)?;
        } else {
            let id = insert_new(session, &report.id, finding, finished)?;
            new_ids.insert(id);
            let stored = session
                .finding(id)?
                .ok_or_else(|| StoreError::Failed(format!("finding {id} was not stored")))?;
            index.insert(key, rows.len());
            rows.push(stored);
        }
    }
    let resolved_count = resolve_missing(session, &mut rows, &seen, finished)?;
    let open_count = rows
        .iter()
        .filter(|row| row.state == FindingState::Open)
        .count();
    session.insert_run(&NewRun {
        project_id: report.id.clone(),
        started_at: started.to_owned(),
        finished_at: finished.to_owned(),
        offline,
        packages_checked: i64::try_from(packages_checked(report)).unwrap_or(i64::MAX),
        open_count: i64::try_from(open_count).unwrap_or(i64::MAX),
        new_count: i64::try_from(new_ids.len()).unwrap_or(i64::MAX),
        resolved_count: i64::from(resolved_count),
        evaluated: true,
        error: None,
    })?;
    Ok(ledger_from(report, &rows, &seen, &new_ids, resolved_count))
}

fn touch_known(
    session: &mut dyn StoreSession,
    row: &mut StoredFinding,
    finding: &Finding,
    finished: &str,
    today: &str,
) -> Result<(), StoreError> {
    row.version.clone_from(&finding.version);
    row.severity.clone_from(&finding.severity);
    row.summary.clone_from(&finding.summary);
    row.aliases.clone_from(&finding.aliases);
    row.fixed_versions.clone_from(&finding.fixed_versions);
    finished.clone_into(&mut row.last_seen);
    if row.state == FindingState::Resolved {
        row.state = FindingState::Open;
        row.resolved_at = None;
        row.ignore_reason = None;
        row.ignore_until = None;
        session.update_finding(row)?;
        return session.insert_event(row.id, "reappeared", finished, "");
    }
    if let Some(until) = row.ignore_until.clone()
        && row.state == FindingState::Ignored
        && until.as_str() < today
    {
        row.state = FindingState::Open;
        row.ignore_reason = None;
        row.ignore_until = None;
        session.update_finding(row)?;
        return session.insert_event(
            row.id,
            "reopened",
            finished,
            &format!("ignore expired on {until}"),
        );
    }
    session.update_finding(row)
}

fn insert_new(
    session: &mut dyn StoreSession,
    project_id: &str,
    finding: &Finding,
    finished: &str,
) -> Result<i64, StoreError> {
    let id = session.insert_finding(&NewFinding {
        project_id: project_id.to_owned(),
        ecosystem: finding.ecosystem.clone(),
        package: finding.package.clone(),
        vulnerability_id: finding.id.clone(),
        version: finding.version.clone(),
        severity: finding.severity.clone(),
        summary: finding.summary.clone(),
        aliases: finding.aliases.clone(),
        fixed_versions: finding.fixed_versions.clone(),
        seen_at: finished.to_owned(),
    })?;
    session.insert_event(id, "discovered", finished, "")?;
    Ok(id)
}

fn resolve_missing(
    session: &mut dyn StoreSession,
    rows: &mut [StoredFinding],
    seen: &BTreeSet<(String, String, String)>,
    finished: &str,
) -> Result<u32, StoreError> {
    let mut resolved = 0_u32;
    for row in rows {
        if seen.contains(&stored_key(row)) || row.state == FindingState::Resolved {
            continue;
        }
        row.state = FindingState::Resolved;
        row.resolved_at = Some(finished.to_owned());
        session.update_finding(row)?;
        session.insert_event(row.id, "resolved", finished, "")?;
        resolved += 1;
    }
    Ok(resolved)
}

fn ledger_from(
    report: &ProjectReport,
    rows: &[StoredFinding],
    seen: &BTreeSet<(String, String, String)>,
    new_ids: &BTreeSet<i64>,
    resolved_count: u32,
) -> ProjectLedger {
    let mut open = Vec::new();
    let mut ignored = Vec::new();
    for row in rows {
        if !seen.contains(&stored_key(row)) {
            continue;
        }
        let listed = listed(row, new_ids.contains(&row.id));
        match row.state {
            FindingState::Open => open.push(listed),
            FindingState::Ignored => ignored.push(listed),
            FindingState::Resolved => {}
        }
    }
    sort_listed(&mut open);
    sort_listed(&mut ignored);
    ProjectLedger {
        id: report.id.clone(),
        path: report.path.clone(),
        lockfiles: report.lockfiles.clone(),
        warnings: report.warnings.clone(),
        error: None,
        stale_cache_from: report.stale_cache_from.clone(),
        open,
        ignored,
        resolved_count,
        evaluated: true,
    }
}

fn record_unevaluated(
    session: &mut dyn StoreSession,
    report: &ProjectReport,
    started: &str,
    finished: &str,
    offline: bool,
) -> Result<ProjectLedger, StoreError> {
    let rows = session.findings_for_project(&report.id)?;
    let open_count = rows
        .iter()
        .filter(|row| row.state == FindingState::Open)
        .count();
    session.insert_run(&NewRun {
        project_id: report.id.clone(),
        started_at: started.to_owned(),
        finished_at: finished.to_owned(),
        offline,
        packages_checked: 0,
        open_count: i64::try_from(open_count).unwrap_or(i64::MAX),
        new_count: 0,
        resolved_count: 0,
        evaluated: false,
        error: None,
    })?;
    let mut open = Vec::new();
    let mut ignored = Vec::new();
    for row in &rows {
        let listed = listed(row, false);
        match row.state {
            FindingState::Open => open.push(listed),
            FindingState::Ignored => ignored.push(listed),
            FindingState::Resolved => {}
        }
    }
    sort_listed(&mut open);
    sort_listed(&mut ignored);
    Ok(ProjectLedger {
        id: report.id.clone(),
        path: report.path.clone(),
        lockfiles: report.lockfiles.clone(),
        warnings: report.warnings.clone(),
        error: None,
        stale_cache_from: report.stale_cache_from.clone(),
        open,
        ignored,
        resolved_count: 0,
        evaluated: false,
    })
}

fn listed(row: &StoredFinding, new: bool) -> LedgerFinding {
    LedgerFinding {
        record_id: row.id,
        new,
        state: row.state,
        ecosystem: row.ecosystem.clone(),
        package: row.package.clone(),
        version: row.version.clone(),
        vulnerability_id: row.vulnerability_id.clone(),
        aliases: row.aliases.clone(),
        severity: row.severity.clone(),
        summary: row.summary.clone(),
        fixed_versions: row.fixed_versions.clone(),
        url: format!("https://osv.dev/vulnerability/{}", row.vulnerability_id),
    }
}

fn sort_listed(findings: &mut [LedgerFinding]) {
    findings.sort_by(|left, right| {
        severity_rank(&left.severity)
            .cmp(&severity_rank(&right.severity))
            .then(left.package.cmp(&right.package))
            .then(left.version.cmp(&right.version))
            .then(left.vulnerability_id.cmp(&right.vulnerability_id))
    });
}

fn empty_ledger(report: &ProjectReport) -> ProjectLedger {
    ProjectLedger {
        id: report.id.clone(),
        path: report.path.clone(),
        lockfiles: report.lockfiles.clone(),
        warnings: report.warnings.clone(),
        error: report.error.clone(),
        stale_cache_from: report.stale_cache_from.clone(),
        open: Vec::new(),
        ignored: Vec::new(),
        resolved_count: 0,
        evaluated: false,
    }
}

fn unique_observed(findings: &[Finding]) -> Vec<&Finding> {
    let mut seen = BTreeSet::new();
    let mut unique = Vec::new();
    for finding in findings {
        if seen.insert(observed_key(finding)) {
            unique.push(finding);
        }
    }
    unique
}

fn observed_key(finding: &Finding) -> (String, String, String) {
    (
        finding.ecosystem.clone(),
        finding.package.clone(),
        finding.id.clone(),
    )
}

fn stored_key(row: &StoredFinding) -> (String, String, String) {
    (
        row.ecosystem.clone(),
        row.package.clone(),
        row.vulnerability_id.clone(),
    )
}

fn index_rows(rows: &[StoredFinding]) -> BTreeMap<(String, String, String), usize> {
    rows.iter()
        .enumerate()
        .map(|(index, row)| (stored_key(row), index))
        .collect()
}

fn packages_checked(report: &ProjectReport) -> usize {
    report
        .lockfiles
        .iter()
        .map(|lockfile| lockfile.queried)
        .sum()
}

fn required(session: &mut dyn StoreSession, id: i64) -> Result<StoredFinding, StoreError> {
    session.finding(id)?.ok_or(StoreError::NotFound(id))
}

fn stamp(now: i64) -> Result<String, StoreError> {
    format_iso8601(now).ok_or_else(|| StoreError::Invalid("time is out of range".to_owned()))
}

/// Text report after findings have been stored.
///
/// `show_ignored` includes ignored findings in the list. They are always counted.
#[must_use]
pub fn render_ledger_text(projects: &[ProjectLedger], show_ignored: bool) -> String {
    if projects.is_empty() {
        return "no projects\n".to_owned();
    }
    let mut out = String::new();
    for project in projects {
        out.push_str(&project.id);
        out.push('\n');
        write_project(&mut out, project, show_ignored);
    }
    out
}

fn write_project(out: &mut String, project: &ProjectLedger, show_ignored: bool) {
    if project.lockfiles.is_empty() && project.error.is_none() && project.warnings.is_empty() {
        out.push_str("  no lockfiles configured\n");
    }
    for lockfile in &project.lockfiles {
        let _ = writeln!(
            out,
            "  lockfile {}: queried {}, skipped {}",
            lockfile.path, lockfile.queried, lockfile.skipped
        );
    }
    for warning in &project.warnings {
        let _ = writeln!(out, "  warning: {warning}");
    }
    if let Some(when) = &project.stale_cache_from {
        let _ = writeln!(
            out,
            "  warning: using cached results from {when} after a network error"
        );
    }
    for finding in &project.open {
        write_finding(out, finding, if finding.new { "new" } else { "open" });
    }
    if show_ignored {
        for finding in &project.ignored {
            write_finding(out, finding, "ignored");
        }
    }
    if project.error.is_none() {
        let _ = writeln!(out, "  ignored: {}", project.ignored.len());
        let _ = writeln!(out, "  resolved: {}", project.resolved_count);
        if project.open.is_empty() && project.ignored.is_empty() && project.resolved_count == 0 {
            out.push_str("  no open findings\n");
        }
        if !project.evaluated && (!project.open.is_empty() || !project.ignored.is_empty()) {
            out.push_str("  existing findings are unchanged until lockfiles are configured\n");
        }
    }
    if let Some(error) = &project.error {
        for line in error.lines() {
            let _ = writeln!(out, "  error: {line}");
        }
    }
}

fn write_finding(out: &mut String, finding: &LedgerFinding, mark: &str) {
    let _ = writeln!(out, "  {} {} {mark}", finding.package, finding.version);
    let aliases = if finding.aliases.is_empty() {
        String::new()
    } else {
        format!(" ({})", finding.aliases.join(", "))
    };
    let _ = writeln!(
        out,
        "    #{} {}{aliases} {}",
        finding.record_id, finding.vulnerability_id, finding.severity
    );
    if !finding.summary.is_empty() {
        let _ = writeln!(out, "    {}", finding.summary);
    }
    if !finding.fixed_versions.is_empty() {
        let _ = writeln!(out, "    fixed in {}", finding.fixed_versions.join(", "));
    }
    let _ = writeln!(out, "    {}", finding.url);
}

/// JSON report after findings have been stored.
///
/// # Errors
///
/// Returns a message when the report cannot be encoded.
pub fn render_ledger_json(
    projects: &[ProjectLedger],
    show_ignored: bool,
) -> Result<String, String> {
    let mut text = serde_json::to_string_pretty(&LedgerDocument {
        projects: projects
            .iter()
            .map(|project| project_json(project, show_ignored))
            .collect(),
    })
    .map_err(|error| error.to_string())?;
    text.push('\n');
    Ok(text)
}

/// Text for `findings` without `show`.
#[must_use]
pub fn render_finding_list(findings: &[StoredFinding]) -> String {
    if findings.is_empty() {
        return "no findings\n".to_owned();
    }
    let mut out = String::new();
    for finding in findings {
        let _ = writeln!(
            out,
            "#{} {} {} {} {} {} {}",
            finding.id,
            finding.project_id,
            finding.package,
            finding.version,
            finding.vulnerability_id,
            finding.severity,
            finding.state.as_str()
        );
    }
    out
}

/// Text for `findings show`.
#[must_use]
pub fn render_finding_show(finding: &StoredFinding, events: &[StoredEvent]) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "#{} {} {}",
        finding.id, finding.package, finding.version
    );
    let _ = writeln!(out, "  project: {}", finding.project_id);
    let _ = writeln!(out, "  ecosystem: {}", finding.ecosystem);
    let _ = writeln!(out, "  vulnerability: {}", finding.vulnerability_id);
    let _ = writeln!(out, "  aliases: {}", finding.aliases.join(", "));
    let _ = writeln!(out, "  severity: {}", finding.severity);
    let _ = writeln!(out, "  state: {}", finding.state.as_str());
    let _ = writeln!(out, "  summary: {}", finding.summary);
    let _ = writeln!(out, "  fixed in: {}", finding.fixed_versions.join(", "));
    let _ = writeln!(
        out,
        "  url: https://osv.dev/vulnerability/{}",
        finding.vulnerability_id
    );
    let _ = writeln!(out, "  first seen: {}", finding.first_seen);
    let _ = writeln!(out, "  last seen: {}", finding.last_seen);
    let _ = writeln!(
        out,
        "  resolved at: {}",
        optional(finding.resolved_at.as_ref())
    );
    let _ = writeln!(
        out,
        "  ignore reason: {}",
        optional(finding.ignore_reason.as_ref())
    );
    let _ = writeln!(
        out,
        "  ignore until: {}",
        optional(finding.ignore_until.as_ref())
    );
    out.push_str("  events:\n");
    for event in events {
        if event.reason.is_empty() {
            let _ = writeln!(out, "    {} {}", event.at, event.kind);
        } else {
            let _ = writeln!(out, "    {} {} {}", event.at, event.kind, event.reason);
        }
    }
    out
}

/// JSON for `findings` and `findings show`.
///
/// # Errors
///
/// Returns a message when the value cannot be encoded.
pub fn render_findings_json(findings: &[StoredFinding]) -> Result<String, String> {
    encode(&FindingsDocument {
        findings: findings.iter().map(finding_json).collect(),
    })
}

/// JSON for one finding and its events.
///
/// # Errors
///
/// Returns a message when the value cannot be encoded.
pub fn render_show_json(finding: &StoredFinding, events: &[StoredEvent]) -> Result<String, String> {
    encode(&ShowDocument {
        finding: finding_json(finding),
        events,
    })
}

fn optional(value: Option<&String>) -> &str {
    value.map_or("-", String::as_str)
}

fn encode(value: &impl Serialize) -> Result<String, String> {
    let mut text = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
    text.push('\n');
    Ok(text)
}

fn project_json(project: &ProjectLedger, show_ignored: bool) -> ProjectJson<'_> {
    let mut findings: Vec<&LedgerFinding> = project.open.iter().collect();
    if show_ignored {
        findings.extend(project.ignored.iter());
    }
    ProjectJson {
        id: &project.id,
        path: &project.path,
        lockfiles: &project.lockfiles,
        findings: findings.into_iter().map(hit_json).collect(),
        ignored: project.ignored.len(),
        resolved: project.resolved_count,
        evaluated: project.evaluated,
        warnings: &project.warnings,
        error: project.error.as_deref(),
        stale_cache_from: project.stale_cache_from.as_deref(),
    }
}

fn hit_json(finding: &LedgerFinding) -> HitJson<'_> {
    HitJson {
        record_id: finding.record_id,
        new: finding.new,
        state: finding.state.as_str(),
        ecosystem: &finding.ecosystem,
        package: &finding.package,
        version: &finding.version,
        id: &finding.vulnerability_id,
        aliases: &finding.aliases,
        severity: &finding.severity,
        summary: &finding.summary,
        fixed_versions: &finding.fixed_versions,
        url: &finding.url,
    }
}

fn finding_json(finding: &StoredFinding) -> FindingJson<'_> {
    FindingJson {
        id: finding.id,
        project_id: &finding.project_id,
        ecosystem: &finding.ecosystem,
        package: &finding.package,
        version: &finding.version,
        vulnerability: &finding.vulnerability_id,
        aliases: &finding.aliases,
        severity: &finding.severity,
        summary: &finding.summary,
        fixed_versions: &finding.fixed_versions,
        state: finding.state.as_str(),
        first_seen: &finding.first_seen,
        last_seen: &finding.last_seen,
        resolved_at: finding.resolved_at.as_deref(),
        ignore_reason: finding.ignore_reason.as_deref(),
        ignore_until: finding.ignore_until.as_deref(),
        url: format!("https://osv.dev/vulnerability/{}", finding.vulnerability_id),
    }
}

#[derive(Serialize)]
struct LedgerDocument<'a> {
    projects: Vec<ProjectJson<'a>>,
}

#[derive(Serialize)]
struct ProjectJson<'a> {
    id: &'a str,
    path: &'a str,
    lockfiles: &'a [LockfileReport],
    findings: Vec<HitJson<'a>>,
    ignored: usize,
    resolved: u32,
    evaluated: bool,
    warnings: &'a [String],
    error: Option<&'a str>,
    stale_cache_from: Option<&'a str>,
}

#[derive(Serialize)]
struct HitJson<'a> {
    record_id: i64,
    new: bool,
    state: &'a str,
    ecosystem: &'a str,
    package: &'a str,
    version: &'a str,
    id: &'a str,
    aliases: &'a [String],
    severity: &'a str,
    summary: &'a str,
    fixed_versions: &'a [String],
    url: &'a str,
}

#[derive(Serialize)]
struct FindingsDocument<'a> {
    findings: Vec<FindingJson<'a>>,
}

#[derive(Serialize)]
struct ShowDocument<'a> {
    finding: FindingJson<'a>,
    events: &'a [StoredEvent],
}

#[derive(Serialize)]
struct FindingJson<'a> {
    id: i64,
    project_id: &'a str,
    ecosystem: &'a str,
    package: &'a str,
    version: &'a str,
    vulnerability: &'a str,
    aliases: &'a [String],
    severity: &'a str,
    summary: &'a str,
    fixed_versions: &'a [String],
    state: &'a str,
    first_seen: &'a str,
    last_seen: &'a str,
    resolved_at: Option<&'a str>,
    ignore_reason: Option<&'a str>,
    ignore_until: Option<&'a str>,
    url: String,
}
