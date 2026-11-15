//! Google Play target-API deadlines from a curated data file.
//!
//! Google publishes no API for these dates. The file shipped with Craftward
//! is [`EMBEDDED`]. A `platform-deadlines.json` in the data directory replaces
//! it when that file is valid and its `data_version` is newer.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::date::{days_between, is_calendar_date};
use crate::model::FormFactor;

const EMBEDDED: &str = include_str!("../../../data/platform-deadlines.json");
const FILE_NAME: &str = "platform-deadlines.json";
/// Official Play Console Help page these deadlines were copied from.
pub const PLAY_HELP: &str =
    "https://support.google.com/googleplay/android-developer/answer/11926878";

/// Where the deadlines in use came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogOrigin {
    /// The file compiled into this build.
    Builtin,
    /// `platform-deadlines.json` in the data directory.
    DataDirectory,
}

/// One published requirement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deadline {
    /// Store. This build only accepts `google_play`.
    pub platform: String,
    /// `phone`, `automotive`, `wear`, `tv`, or `xr`.
    pub form_factor: String,
    /// Stable requirement id, such as `new_apps_and_updates`.
    pub requirement: String,
    /// Minimum `targetSdk` that meets the requirement.
    pub api_level: u32,
    /// First day the requirement is in force, `YYYY-MM-DD`.
    pub deadline: String,
    /// Page the date was copied from.
    pub source: String,
    /// Day the row was checked against that page.
    pub checked_on: String,
    /// Short explanation of the row.
    pub note: String,
}

/// Deadlines selected for one run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Catalog {
    /// `data_version` of the file in use.
    pub data_version: String,
    /// Which file supplied [`Self::entries`].
    pub origin: CatalogOrigin,
    /// Rows from that file, in file order.
    pub entries: Vec<Deadline>,
    /// The data-directory file was unreadable. The built-in file is in use.
    pub warning: Option<String>,
    /// A data-directory file existed and was older than the built-in file.
    pub ignored_older: Option<String>,
}

/// One deadline that the current target SDK does not meet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeadlineHit {
    /// `PLAY:<form factor>:<requirement>`.
    pub id: String,
    /// `platform_deadline` or `platform_deadline_soon`.
    pub kind: &'static str,
    /// `high` or `medium`.
    pub severity: &'static str,
    /// Required level, current level, deadline, and where targetSdk was found.
    pub summary: String,
    /// Source page for this row.
    pub url: String,
}

/// Loads the built-in file, or a newer valid file from `data_dir`.
///
/// # Errors
///
/// Returns a message when the built-in file itself is invalid.
pub fn load(data_dir: &Path) -> Result<Catalog, String> {
    let builtin = parse_document(EMBEDDED)?;
    let path = data_dir.join(FILE_NAME);
    if !path.is_file() {
        return Ok(from_parsed(builtin, CatalogOrigin::Builtin, None, None));
    }
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => {
            return Ok(from_parsed(
                builtin,
                CatalogOrigin::Builtin,
                Some(format!(
                    "platform-deadlines.json in the data directory is invalid: {error}"
                )),
                None,
            ));
        }
    };
    let parsed = match parse_document(&text) {
        Ok(parsed) => parsed,
        Err(error) => {
            return Ok(from_parsed(
                builtin,
                CatalogOrigin::Builtin,
                Some(format!(
                    "platform-deadlines.json in the data directory is invalid: {error}"
                )),
                None,
            ));
        }
    };
    if parsed.data_version.as_str() > builtin.data_version.as_str() {
        return Ok(from_parsed(
            parsed,
            CatalogOrigin::DataDirectory,
            None,
            None,
        ));
    }
    let older = parsed.data_version;
    Ok(from_parsed(
        builtin,
        CatalogOrigin::Builtin,
        None,
        Some(older),
    ))
}

/// Checks the text of a deadline file.
///
/// # Errors
///
/// Returns every problem, separated by newlines.
pub fn parse_document(text: &str) -> Result<Parsed, String> {
    let file: File = serde_json::from_str(text).map_err(|error| error.to_string())?;
    let mut errors = Vec::new();
    if file.schema_version != 1 {
        errors.push("schema_version must be 1".to_owned());
    }
    if !is_calendar_date(&file.data_version) {
        errors.push("data_version must use YYYY-MM-DD".to_owned());
    }
    let mut seen = BTreeSet::new();
    for (index, row) in file.deadlines.iter().enumerate() {
        check_row(index, row, &mut seen, &mut errors);
    }
    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }
    Ok(Parsed {
        data_version: file.data_version,
        entries: file.deadlines.into_iter().map(Deadline::from).collect(),
    })
}

/// A file that passed validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parsed {
    /// `data_version`.
    pub data_version: String,
    /// Rows, in file order.
    pub entries: Vec<Deadline>,
}

/// Warning when `data_version` is more than 365 days before `today`.
#[must_use]
pub fn stale_warning(data_version: &str, today: &str) -> Option<String> {
    let age = days_between(data_version, today)?;
    if age > 365 {
        Some(format!(
            "platform deadline data from {data_version} may be out of date"
        ))
    } else {
        None
    }
}

/// Requirements the installed SDK does not meet.
///
/// For one form factor and requirement, the past row is the one with the
/// latest deadline that is today or earlier. Older rows of that pair are
/// ignored. When that row is already met, the next future row counts if it
/// falls inside `warning_days`.
#[must_use]
pub fn compare(
    catalog: &Catalog,
    form_factor: &str,
    target_sdk: u32,
    detected_at: &str,
    today: &str,
    warning_days: u32,
) -> Vec<DeadlineHit> {
    let mut grouped: BTreeMap<&str, Vec<&Deadline>> = BTreeMap::new();
    for entry in &catalog.entries {
        if entry.platform == "google_play" && entry.form_factor == form_factor {
            grouped
                .entry(entry.requirement.as_str())
                .or_default()
                .push(entry);
        }
    }
    let mut hits = Vec::new();
    for entries in grouped.values() {
        if let Some(hit) = governing(entries, target_sdk, detected_at, today, warning_days) {
            hits.push(hit);
        }
    }
    hits
}

fn governing(
    entries: &[&Deadline],
    target_sdk: u32,
    detected_at: &str,
    today: &str,
    warning_days: u32,
) -> Option<DeadlineHit> {
    if let Some(row) = newest_reached(entries, today)
        && target_sdk < row.api_level
    {
        return Some(hit(
            row,
            target_sdk,
            detected_at,
            "platform_deadline",
            "high",
        ));
    }
    let row = next_future(entries, today)?;
    let distance = days_between(today, &row.deadline)?;
    if distance <= i64::from(warning_days) && target_sdk < row.api_level {
        return Some(hit(
            row,
            target_sdk,
            detected_at,
            "platform_deadline_soon",
            "medium",
        ));
    }
    None
}

fn newest_reached<'a>(entries: &[&'a Deadline], today: &str) -> Option<&'a Deadline> {
    entries
        .iter()
        .copied()
        .filter(|entry| days_between(today, &entry.deadline).is_some_and(|days| days <= 0))
        .max_by(|left, right| left.deadline.cmp(&right.deadline))
}

fn next_future<'a>(entries: &[&'a Deadline], today: &str) -> Option<&'a Deadline> {
    entries
        .iter()
        .copied()
        .filter(|entry| days_between(today, &entry.deadline).is_some_and(|days| days > 0))
        .min_by(|left, right| left.deadline.cmp(&right.deadline))
}

fn hit(
    row: &Deadline,
    target_sdk: u32,
    detected_at: &str,
    kind: &'static str,
    severity: &'static str,
) -> DeadlineHit {
    let label = row.requirement.replace('_', " ");
    DeadlineHit {
        id: format!("PLAY:{}:{}", row.form_factor, row.requirement),
        kind,
        severity,
        summary: format!(
            "targetSdk {target_sdk} is below required {} for {label} (deadline {}); detected at {detected_at}",
            row.api_level, row.deadline
        ),
        url: row.source.clone(),
    }
}

/// Page linked from a stored finding.
#[must_use]
pub fn page_url(ecosystem: &str, package: &str, vulnerability_id: &str) -> String {
    if ecosystem == "endoflife" {
        return format!("https://endoflife.date/{package}");
    }
    if ecosystem == "platform" {
        return source_for(vulnerability_id).unwrap_or_else(|| PLAY_HELP.to_owned());
    }
    format!("https://osv.dev/vulnerability/{vulnerability_id}")
}

/// Text for `craftward deadlines`.
#[must_use]
pub fn render(catalog: &Catalog, today: &str) -> String {
    let mut out = String::new();
    match catalog.origin {
        CatalogOrigin::Builtin => {
            let _ = writeln!(
                out,
                "platform deadlines from the built-in data (data_version {})",
                catalog.data_version
            );
        }
        CatalogOrigin::DataDirectory => {
            let _ = writeln!(
                out,
                "platform deadlines from the data directory (data_version {})",
                catalog.data_version
            );
        }
    }
    if let Some(warning) = &catalog.warning {
        let _ = writeln!(out, "warning: {warning}");
    }
    if let Some(older) = &catalog.ignored_older {
        let _ = writeln!(
            out,
            "data directory file data_version {older} is older and was not used"
        );
    }
    if let Some(stale) = stale_warning(&catalog.data_version, today) {
        let _ = writeln!(out, "warning: {stale}");
    }
    for entry in &catalog.entries {
        let _ = writeln!(
            out,
            "{} {} API {} deadline {}\n  {}\n  {}\n  checked {}",
            entry.form_factor,
            entry.requirement,
            entry.api_level,
            entry.deadline,
            entry.note,
            entry.source,
            entry.checked_on
        );
    }
    out
}

fn source_for(vulnerability_id: &str) -> Option<String> {
    let mut parts = vulnerability_id.split(':');
    if parts.next() != Some("PLAY") {
        return None;
    }
    let form_factor = parts.next()?;
    let requirement = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    let parsed = parse_document(EMBEDDED).ok()?;
    parsed
        .entries
        .into_iter()
        .filter(|entry| entry.form_factor == form_factor && entry.requirement == requirement)
        .max_by(|left, right| left.deadline.cmp(&right.deadline))
        .map(|entry| entry.source)
}

fn from_parsed(
    parsed: Parsed,
    origin: CatalogOrigin,
    warning: Option<String>,
    ignored_older: Option<String>,
) -> Catalog {
    Catalog {
        data_version: parsed.data_version,
        origin,
        entries: parsed.entries,
        warning,
        ignored_older,
    }
}

fn check_row(index: usize, row: &Row, seen: &mut BTreeSet<String>, errors: &mut Vec<String>) {
    let path = format!("deadlines[{index}]");
    if row.platform != "google_play" {
        errors.push(format!("{path}.platform must be google_play"));
    }
    if FormFactor::parse(&row.form_factor).is_none() {
        errors.push(format!(
            "{path}.form_factor must be phone, automotive, wear, tv, or xr"
        ));
    }
    if !is_requirement(&row.requirement) {
        errors.push(format!(
            "{path}.requirement must be a short id of a-z, 0-9, and underscore"
        ));
    }
    if row.api_level == 0 {
        errors.push(format!("{path}.api_level must be a positive integer"));
    }
    if !is_calendar_date(&row.deadline) {
        errors.push(format!("{path}.deadline must use YYYY-MM-DD"));
    }
    if !is_https(&row.source) {
        errors.push(format!("{path}.source must be an https URL"));
    }
    if !is_calendar_date(&row.checked_on) {
        errors.push(format!("{path}.checked_on must use YYYY-MM-DD"));
    }
    if row.note.trim().is_empty() {
        errors.push(format!("{path}.note must not be empty"));
    }
    let key = format!(
        "{}|{}|{}|{}",
        row.platform, row.form_factor, row.requirement, row.deadline
    );
    if !seen.insert(key) {
        errors.push(format!(
            "{path} duplicates {} {} {} {}",
            row.platform, row.form_factor, row.requirement, row.deadline
        ));
    }
}

fn is_requirement(text: &str) -> bool {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_lowercase() || text.len() > 64 {
        return false;
    }
    chars.all(|char| char.is_ascii_lowercase() || char.is_ascii_digit() || char == '_')
}

fn is_https(text: &str) -> bool {
    let Some(rest) = text.strip_prefix("https://") else {
        return false;
    };
    !rest.is_empty() && !rest.chars().any(char::is_whitespace)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    schema_version: u32,
    data_version: String,
    deadlines: Vec<Row>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    platform: String,
    form_factor: String,
    requirement: String,
    api_level: u32,
    deadline: String,
    source: String,
    checked_on: String,
    note: String,
}

impl From<Row> for Deadline {
    fn from(row: Row) -> Self {
        Self {
            platform: row.platform,
            form_factor: row.form_factor,
            requirement: row.requirement,
            api_level: row.api_level,
            deadline: row.deadline,
            source: row.source,
            checked_on: row.checked_on,
            note: row.note,
        }
    }
}
