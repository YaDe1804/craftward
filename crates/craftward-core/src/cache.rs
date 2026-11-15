//! JSON cache for OSV query results and vulnerability records.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::fs_atomic::write_atomic;
use crate::lockfile::PackageQuery;
use crate::osv::Advisory;

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
    pub severity: String,
    /// Affected packages and fixed versions.
    pub affected: Vec<StoredAffected>,
    /// Whether the record was withdrawn. Absent in caches written before this field.
    #[serde(default)]
    pub withdrawn: bool,
    /// Advisory kind. Absent caches are read as `vulnerability`.
    #[serde(default = "vulnerability_kind")]
    pub kind: String,
}

fn vulnerability_kind() -> String {
    "vulnerability".to_owned()
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
            severity: advisory.severity.clone(),
            withdrawn: advisory.withdrawn,
            kind: advisory.kind.clone(),
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
    let _ = write_json(&query_path(cache_dir, package), cached);
}

/// Reads a vulnerability record from the cache.
#[must_use]
pub fn read_advisory(cache_dir: &Path, id: &str) -> Option<CachedAdvisory> {
    read_json(&advisory_path(cache_dir, id))
}

/// Writes a vulnerability record.
pub fn write_advisory(cache_dir: &Path, cached: &CachedAdvisory) {
    let _ = write_json(&advisory_path(cache_dir, &cached.advisory.id), cached);
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

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Option<T> {
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    let mut text = serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".to_owned());
    text.push('\n');
    write_atomic(path, text.as_bytes())
}

fn encode(text: &str) -> String {
    let mut out = String::new();
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.') {
            out.push(char::from(byte));
        } else {
            let _ = write!(out, "%{byte:02X}");
        }
    }
    out
}
