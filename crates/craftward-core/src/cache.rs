//! JSON cache for OSV query results and vulnerability records.

use std::fs;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::date::format_unix_timestamp;
use crate::fs_atomic::write_atomic;
use crate::lockfile::PackageQuery;
use crate::osv::Advisory;
use crate::percent::encode;
use crate::store::{FindingKind, Severity};

/// A stored package query.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CachedQuery {
    /// POSIX seconds when this file was written.
    pub stored_at: i64,
    /// Vulnerability ids returned for the package.
    pub ids: Vec<String>,
}

/// A stored vulnerability record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CachedAdvisory {
    /// POSIX seconds when this file was written.
    pub stored_at: i64,
    /// The record itself.
    pub advisory: StoredAdvisory,
}

/// The advisory fields that are written to disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredAdvisory {
    /// OSV id.
    pub id: String,
    /// Other ids.
    pub aliases: Vec<String>,
    /// Short description.
    pub summary: String,
    /// Radar severity word.
    pub severity: Severity,
    /// Affected packages and fixed versions.
    pub affected: Vec<StoredAffected>,
    /// Whether the record was withdrawn. Absent in caches written before this field.
    #[serde(default)]
    pub withdrawn: bool,
    /// Advisory kind. Absent caches are read as `vulnerability`.
    #[serde(default)]
    pub kind: FindingKind,
}

/// One affected package in the cache.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredAffected {
    /// OSV ecosystem name.
    pub ecosystem: String,
    /// Package name.
    pub name: String,
    /// Fixed versions.
    pub fixed: Vec<String>,
}

impl From<&Advisory> for StoredAdvisory {
    fn from(advisory: &Advisory) -> Self {
        Self {
            id: advisory.id.clone(),
            aliases: advisory.aliases.clone(),
            summary: advisory.summary.clone(),
            severity: advisory.severity,
            withdrawn: advisory.withdrawn,
            kind: advisory.kind,
            affected: advisory
                .affected
                .iter()
                .map(|package| StoredAffected {
                    ecosystem: package.ecosystem.clone(),
                    name: package.name.clone(),
                    fixed: package.fixed.clone(),
                })
                .collect(),
        }
    }
}

impl From<StoredAdvisory> for Advisory {
    fn from(stored: StoredAdvisory) -> Self {
        Self {
            id: stored.id,
            aliases: stored.aliases,
            summary: stored.summary,
            severity: stored.severity,
            withdrawn: stored.withdrawn,
            kind: stored.kind,
            affected: stored
                .affected
                .into_iter()
                .map(|package| crate::osv::AffectedPackage {
                    ecosystem: package.ecosystem,
                    name: package.name,
                    fixed: package.fixed,
                })
                .collect(),
        }
    }
}

/// Whether `stored_at` is still inside `ttl_seconds` at `now`.
#[must_use]
pub fn is_fresh(stored_at: i64, now: i64, ttl_seconds: i64) -> bool {
    now.saturating_sub(stored_at) < ttl_seconds
}

/// Reads a package query from the cache.
#[must_use]
pub fn read_query(cache_dir: &Path, package: &PackageQuery) -> Option<CachedQuery> {
    read_json(&query_path(cache_dir, package))
}

/// Writes a package query. A write failure leaves the previous file in place.
pub fn write_query(cache_dir: &Path, package: &PackageQuery, cached: &CachedQuery) {
    let _ = write_stored(&query_path(cache_dir, package), cached, true);
}

/// Reads a vulnerability record from the cache.
#[must_use]
pub fn read_advisory(cache_dir: &Path, id: &str) -> Option<CachedAdvisory> {
    read_json(&advisory_path(cache_dir, id))
}

/// Writes a vulnerability record. A write failure leaves the previous file in place.
pub fn write_advisory(cache_dir: &Path, cached: &CachedAdvisory) {
    let _ = write_stored(&advisory_path(cache_dir, &cached.advisory.id), cached, true);
}

fn query_path(cache_dir: &Path, package: &PackageQuery) -> PathBuf {
    cache_dir
        .join("queries")
        .join(encode(&package.ecosystem))
        .join(encode(&package.name))
        .join(format!("{}.json", encode(&package.version)))
}

fn advisory_path(cache_dir: &Path, id: &str) -> PathBuf {
    cache_dir
        .join("advisories")
        .join(format!("{}.json", encode(id)))
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Option<T> {
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// How a caller wants a timed JSON file to be used.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CachePolicy {
    /// POSIX seconds of this check.
    pub now: i64,
    /// Maximum age that still counts as fresh.
    pub ttl_seconds: i64,
    /// Ignore a fresh file and fetch again.
    pub refresh: bool,
    /// Do not contact the network.
    pub offline: bool,
}

/// A stored file, or the reason the caller should fetch or stop.
#[derive(Debug)]
pub(crate) enum CacheChoice<T> {
    /// `stored_at` is inside the lifetime.
    Fresh(T),
    /// The file is older than the lifetime, and the caller is offline.
    Stale(T),
    /// The caller should ask the network.
    Fetch,
    /// Offline, and there is no readable file.
    Unavailable,
}

/// A value that records when it was written.
pub(crate) trait TimedCache {
    /// POSIX seconds, when the file has that field.
    fn stored_at(&self) -> Option<i64>;
}

impl TimedCache for CachedQuery {
    fn stored_at(&self) -> Option<i64> {
        Some(self.stored_at)
    }
}

impl TimedCache for CachedAdvisory {
    fn stored_at(&self) -> Option<i64> {
        Some(self.stored_at)
    }
}

/// What to do when a cache file is JSON of the wrong shape.
#[derive(Debug, Clone, Copy)]
pub(crate) enum ShapeFault {
    /// Treat it as a missing file. OSV records are fetched again.
    Absent,
    /// Report `message`. End-of-life caches use this.
    Error(&'static str),
}

/// Reads `path` and decides whether it is fresh, stale, or should be fetched.
///
/// # Errors
///
/// Returns [`ShapeFault::Error`] when the file is JSON of the wrong shape.
pub(crate) fn choose<T>(
    path: &Path,
    policy: CachePolicy,
    fault: ShapeFault,
) -> Result<CacheChoice<T>, String>
where
    T: DeserializeOwned + TimedCache,
{
    if policy.refresh && !policy.offline {
        return Ok(CacheChoice::Fetch);
    }
    match read_shaped::<T>(path, fault)? {
        Some(value) if fresh_at(value.stored_at(), policy.now, policy.ttl_seconds) => {
            Ok(CacheChoice::Fresh(value))
        }
        Some(value) if policy.offline => Ok(CacheChoice::Stale(value)),
        Some(_) if !policy.offline => Ok(CacheChoice::Fetch),
        None if policy.offline => Ok(CacheChoice::Unavailable),
        _ => Ok(CacheChoice::Fetch),
    }
}

pub(crate) fn choose_query(
    cache_dir: &Path,
    package: &PackageQuery,
    policy: CachePolicy,
) -> Result<CacheChoice<CachedQuery>, String> {
    choose(&query_path(cache_dir, package), policy, ShapeFault::Absent)
}

pub(crate) fn choose_advisory(
    cache_dir: &Path,
    id: &str,
    policy: CachePolicy,
) -> Result<CacheChoice<CachedAdvisory>, String> {
    choose(&advisory_path(cache_dir, id), policy, ShapeFault::Absent)
}

/// Reads one JSON file.
///
/// # Errors
///
/// Returns the message in [`ShapeFault::Error`] when the bytes are JSON of the
/// wrong shape.
pub(crate) fn read_shaped<T: DeserializeOwned>(
    path: &Path,
    fault: ShapeFault,
) -> Result<Option<T>, String> {
    let Ok(bytes) = fs::read(path) else {
        return Ok(None);
    };
    match serde_json::from_slice(&bytes) {
        Ok(value) => Ok(Some(value)),
        Err(error) if error.is_data() => match fault {
            ShapeFault::Absent => Ok(None),
            ShapeFault::Error(message) => Err(message.to_owned()),
        },
        Err(_) => Ok(None),
    }
}

/// Writes pretty JSON. `trailing_newline` keeps the OSV cache files unchanged.
///
/// A serialization error is returned and nothing is written. An IO error is
/// also returned. The previous file stays in place either way.
///
/// # Errors
///
/// Returns the serializer or filesystem error.
pub(crate) fn write_stored<T: Serialize>(
    path: &Path,
    value: &T,
    trailing_newline: bool,
) -> Result<(), String> {
    let mut text = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
    if trailing_newline {
        text.push('\n');
    }
    write_atomic(path, text.as_bytes()).map_err(|error| error.to_string())
}

fn fresh_at(stored_at: Option<i64>, now: i64, ttl_seconds: i64) -> bool {
    stored_at.is_some_and(|stored| is_fresh(stored, now, ttl_seconds))
}

/// Warning for an offline caller that reused an old file.
#[must_use]
pub(crate) fn offline_cache_warning(stored_at: Option<i64>) -> String {
    match stored_at.and_then(format_unix_timestamp) {
        Some(when) => format!("using cached results from {when}"),
        None => "using expired cached results".to_owned(),
    }
}

/// Warning after a network error when an old file was reused.
#[must_use]
pub(crate) fn network_cache_warning(stored_at: Option<i64>) -> String {
    match stored_at.and_then(format_unix_timestamp) {
        Some(when) => format!("using cached results from {when} after a network error"),
        None => "using cached results after a network error".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::store::{FindingKind, Severity};

    use super::read_advisory;

    #[test]
    fn an_old_advisory_cache_keeps_its_words_and_an_unknown_word_is_dropped() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        let root = std::env::temp_dir().join(format!("craftward-cache-{nanos}"));
        let dir = root.join("advisories");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("GHSA-1.json"),
            r#"{
  "stored_at": 1,
  "advisory": {
    "id": "GHSA-1",
    "aliases": ["CVE-1"],
    "summary": "hole",
    "severity": "high",
    "affected": [{"ecosystem": "crates.io", "name": "demo", "fixed": ["1.2.3"]}],
    "withdrawn": false,
    "kind": "vulnerability"
  }
}
"#,
        )
        .unwrap();
        let cached = read_advisory(&root, "GHSA-1").unwrap();
        assert_eq!(cached.advisory.severity, Severity::High);
        assert_eq!(cached.advisory.kind, FindingKind::Vulnerability);
        assert_eq!(cached.advisory.aliases, ["CVE-1"]);
        fs::write(
            dir.join("GHSA-2.json"),
            r#"{
  "stored_at": 1,
  "advisory": {
    "id": "GHSA-2",
    "aliases": [],
    "summary": "hole",
    "severity": "high",
    "affected": [],
    "kind": "not-a-kind"
  }
}
"#,
        )
        .unwrap();
        assert!(read_advisory(&root, "GHSA-2").is_none());
        let _ = fs::remove_dir_all(root);
    }
}
