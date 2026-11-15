//! Scan project lockfiles and match them to OSV vulnerability records.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::cache::{self, CachedAdvisory, CachedQuery};
use crate::date::format_unix_timestamp;
use crate::http::HttpClient;
use crate::load::load_from_path;
use crate::lockfile::{self, LockKind, PackageQuery, ParsedLock, read_lockfile};
use crate::osv::{self, ADVISORY_TTL_SECONDS, Advisory, QUERY_TTL_SECONDS};
use crate::paths::resolve_project_paths;

/// Options for one radar run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RadarOptions {
    /// Ignore fresh cache entries and query OSV again.
    pub refresh: bool,
    /// Never contact OSV. Missing cache entries are errors.
    pub offline: bool,
    /// POSIX seconds used for cache expiry.
    pub now: i64,
}

/// One project after a radar run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectReport {
    /// Project id, or the file path when the file could not be read.
    pub id: String,
    /// Project file.
    pub path: String,
    /// Lockfiles that were read.
    pub lockfiles: Vec<LockfileReport>,
    /// Vulnerabilities, most severe first.
    pub findings: Vec<Finding>,
    /// Warnings that did not fail the project.
    pub warnings: Vec<String>,
    /// Why this project could not be checked. Other projects are unaffected.
    pub error: Option<String>,
    /// Timestamp of cached data used after a network failure.
    pub stale_cache_from: Option<String>,
}

/// Counts for one lockfile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LockfileReport {
    /// Lockfile path.
    pub path: String,
    /// `cargo`, `npm`, or `unknown`.
    pub kind: String,
    /// Packages sent to OSV.
    pub queried: usize,
    /// Packages that were not sent.
    pub skipped: usize,
}

/// One vulnerability in one package version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Finding {
    /// OSV ecosystem.
    pub ecosystem: String,
    /// Package name.
    pub package: String,
    /// Package version.
    pub version: String,
    /// OSV id.
    pub id: String,
    /// Other ids, such as CVE ids.
    pub aliases: Vec<String>,
    /// `low`, `medium`, `high`, `critical`, or `unknown`.
    pub severity: String,
    /// Short description.
    pub summary: String,
    /// Versions in which this package is fixed.
    pub fixed_versions: Vec<String>,
    /// Page on osv.dev.
    pub url: String,
}

/// Checks each project file. A failure is stored on that project.
#[must_use]
pub fn scan(
    project_files: &[PathBuf],
    client: &impl HttpClient,
    cache_dir: &Path,
    options: &RadarOptions,
) -> Vec<ProjectReport> {
    let mut prepared = project_files
        .iter()
        .map(|path| prepare(path))
        .collect::<Vec<_>>();
    let mut needed = Vec::new();
    for (index, project) in prepared.iter().enumerate() {
        if project.error.is_some() {
            continue;
        }
        for package in &project.packages {
            needed.push((index, package.clone()));
        }
    }
    let looked_up = lookup_packages(&needed, client, cache_dir, options);
    let advisories = lookup_advisories(&looked_up, client, cache_dir, options);
    for project in &mut prepared {
        if project.error.is_some() {
            continue;
        }
        fill_project(project, &looked_up, &advisories);
    }
    prepared.into_iter().map(Prepared::into_report).collect()
}

/// Plain-text report. Paths use the separators already stored in the report.
#[must_use]
pub fn render_text(projects: &[ProjectReport]) -> String {
    if projects.is_empty() {
        return "no projects\n".to_owned();
    }
    let mut out = String::new();
    for project in projects {
        out.push_str(&project.id);
        out.push('\n');
        if project.lockfiles.is_empty() && project.error.is_none() && project.warnings.is_empty() {
            out.push_str("  no lockfiles configured\n");
            continue;
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
        if project.findings.is_empty() && project.error.is_none() && !project.lockfiles.is_empty() {
            out.push_str("  no findings\n");
        }
        for finding in &project.findings {
            let _ = writeln!(out, "  {} {}", finding.package, finding.version);
            let aliases = if finding.aliases.is_empty() {
                String::new()
            } else {
                format!(" ({})", finding.aliases.join(", "))
            };
            let _ = writeln!(out, "    {}{aliases} {}", finding.id, finding.severity);
            if !finding.summary.is_empty() {
                let _ = writeln!(out, "    {}", finding.summary);
            }
            if !finding.fixed_versions.is_empty() {
                let _ = writeln!(out, "    fixed in {}", finding.fixed_versions.join(", "));
            }
            let _ = writeln!(out, "    {}", finding.url);
        }
        if let Some(error) = &project.error {
            for line in error.lines() {
                let _ = writeln!(out, "  error: {line}");
            }
        }
    }
    out
}

/// JSON report. The field names are part of the documented format.
///
/// # Errors
///
/// Returns a message when the report cannot be encoded.
pub fn render_json(projects: &[ProjectReport]) -> Result<String, String> {
    let mut text = serde_json::to_string_pretty(&RadarDocument { projects })
        .map_err(|error| error.to_string())?;
    text.push('\n');
    Ok(text)
}

#[derive(Serialize)]
struct RadarDocument<'a> {
    projects: &'a [ProjectReport],
}

struct Prepared {
    id: String,
    path: String,
    lockfiles: Vec<LockfileReport>,
    packages: Vec<PackageQuery>,
    warnings: Vec<String>,
    error: Option<String>,
    findings: Vec<Finding>,
    stale_cache_from: Option<String>,
}

impl Prepared {
    fn into_report(self) -> ProjectReport {
        ProjectReport {
            id: self.id,
            path: self.path,
            lockfiles: self.lockfiles,
            findings: self.findings,
            warnings: self.warnings,
            error: self.error,
            stale_cache_from: self.stale_cache_from,
        }
    }
}

fn prepare(path: &Path) -> Prepared {
    let path_text = path.display().to_string();
    let project = match load_from_path(path) {
        Ok(project) => project,
        Err(error) => {
            return Prepared {
                id: path_text.clone(),
                path: path_text,
                lockfiles: Vec::new(),
                packages: Vec::new(),
                warnings: Vec::new(),
                error: Some(error.to_string()),
                findings: Vec::new(),
                stale_cache_from: None,
            };
        }
    };
    let resolved = resolve_project_paths(path, &project);
    if resolved.lockfiles.is_empty() {
        return Prepared {
            id: project.id,
            path: path_text,
            lockfiles: Vec::new(),
            packages: Vec::new(),
            warnings: Vec::new(),
            error: None,
            findings: Vec::new(),
            stale_cache_from: None,
        };
    }
    let mut lockfiles = Vec::new();
    let mut packages = Vec::new();
    let mut warnings = Vec::new();
    let mut errors = Vec::new();
    for lock_path in &resolved.lockfiles {
        match lockfile::lock_kind(lock_path) {
            LockKind::Unknown => {
                warnings.push(format!("unknown lockfile type: {}", lock_path.display()));
            }
            LockKind::Cargo | LockKind::Npm => match read_lockfile(lock_path) {
                Ok(parsed) => {
                    packages.extend(parsed.queried.iter().cloned());
                    lockfiles.push(lockfile_report(lock_path, &parsed));
                }
                Err(failure) => errors.push(lock_message(lock_path, &failure)),
            },
        }
    }
    Prepared {
        id: project.id,
        path: path_text,
        lockfiles,
        packages,
        warnings,
        error: if errors.is_empty() {
            None
        } else {
            Some(errors.join("\n"))
        },
        findings: Vec::new(),
        stale_cache_from: None,
    }
}

fn lockfile_report(path: &Path, parsed: &ParsedLock) -> LockfileReport {
    LockfileReport {
        path: path.display().to_string(),
        kind: parsed.kind.to_owned(),
        queried: parsed.queried.len(),
        skipped: parsed.skipped,
    }
}

fn lock_message(path: &Path, failure: &lockfile::LockFailure) -> String {
    match failure.line {
        Some(line) => format!("{}:{line}: {}", path.display(), failure.message),
        None => format!("{}: {}", path.display(), failure.message),
    }
}

struct PackageLookup {
    key: PackageQuery,
    result: Result<IdList, String>,
}

struct IdList {
    ids: Vec<String>,
    stale_at: Option<i64>,
    from_network_error: bool,
}

fn lookup_packages(
    needed: &[(usize, PackageQuery)],
    client: &impl HttpClient,
    cache_dir: &Path,
    options: &RadarOptions,
) -> Vec<PackageLookup> {
    let mut unique = Vec::new();
    for (_, package) in needed {
        if !unique
            .iter()
            .any(|existing: &PackageQuery| existing == package)
        {
            unique.push(package.clone());
        }
    }
    let mut pending = Vec::new();
    let mut resolved = BTreeMap::new();
    for package in &unique {
        match cached_query(package, cache_dir, options) {
            CacheUse::Ready(list) => {
                resolved.insert(package.clone(), Ok(list));
            }
            CacheUse::Fetch => pending.push(package.clone()),
            CacheUse::Missing(message) => {
                resolved.insert(package.clone(), Err(message));
            }
        }
    }
    if !pending.is_empty() {
        match osv::query_ids(client, &pending) {
            Ok(groups) => {
                for (package, ids) in pending.iter().zip(groups) {
                    cache::write_query(
                        cache_dir,
                        package,
                        &CachedQuery {
                            stored_at: options.now,
                            ids: ids.clone(),
                        },
                    );
                    resolved.insert(
                        package.clone(),
                        Ok(IdList {
                            ids,
                            stale_at: None,
                            from_network_error: false,
                        }),
                    );
                }
            }
            Err(error) => {
                for package in &pending {
                    resolved.insert(
                        package.clone(),
                        fallback_query(package, cache_dir, &error.message),
                    );
                }
            }
        }
    }
    unique
        .into_iter()
        .map(|key| PackageLookup {
            result: resolved
                .remove(&key)
                .unwrap_or_else(|| Err("package was not looked up".to_owned())),
            key,
        })
        .collect()
}

enum CacheUse {
    Ready(IdList),
    Fetch,
    Missing(String),
}

fn cached_query(package: &PackageQuery, cache_dir: &Path, options: &RadarOptions) -> CacheUse {
    if options.refresh && !options.offline {
        return CacheUse::Fetch;
    }
    match cache::read_query(cache_dir, package) {
        Some(cached) if cache::is_fresh(cached.stored_at, options.now, QUERY_TTL_SECONDS) => {
            CacheUse::Ready(IdList {
                ids: cached.ids,
                stale_at: None,
                from_network_error: false,
            })
        }
        Some(cached) if options.offline => CacheUse::Ready(IdList {
            ids: cached.ids,
            stale_at: Some(cached.stored_at),
            from_network_error: false,
        }),
        Some(_) if !options.offline => CacheUse::Fetch,
        None if options.offline => CacheUse::Missing(format!(
            "{} {} is not in the cache",
            package.name, package.version
        )),
        _ => CacheUse::Fetch,
    }
}

fn fallback_query(
    package: &PackageQuery,
    cache_dir: &Path,
    message: &str,
) -> Result<IdList, String> {
    match cache::read_query(cache_dir, package) {
        Some(cached) => Ok(IdList {
            ids: cached.ids,
            stale_at: Some(cached.stored_at),
            from_network_error: true,
        }),
        None => Err(format!(
            "OSV query failed for {} {}: {message}",
            package.name, package.version
        )),
    }
}

fn lookup_advisories(
    packages: &[PackageLookup],
    client: &impl HttpClient,
    cache_dir: &Path,
    options: &RadarOptions,
) -> BTreeMap<String, Result<AdvisoryHit, String>> {
    let mut ids = Vec::new();
    for package in packages {
        if let Ok(list) = &package.result {
            for id in &list.ids {
                if !ids.iter().any(|existing| existing == id) {
                    ids.push(id.clone());
                }
            }
        }
    }
    let mut found = BTreeMap::new();
    for id in ids {
        found.insert(
            id.clone(),
            lookup_one_advisory(&id, client, cache_dir, options),
        );
    }
    found
}

struct AdvisoryHit {
    advisory: Advisory,
    stale_at: Option<i64>,
    from_network_error: bool,
}

fn lookup_one_advisory(
    id: &str,
    client: &impl HttpClient,
    cache_dir: &Path,
    options: &RadarOptions,
) -> Result<AdvisoryHit, String> {
    if !options.refresh || options.offline {
        if let Some(cached) = cache::read_advisory(cache_dir, id) {
            let fresh = cache::is_fresh(cached.stored_at, options.now, ADVISORY_TTL_SECONDS);
            if fresh || options.offline {
                return Ok(AdvisoryHit {
                    advisory: cached.advisory.into(),
                    stale_at: if fresh { None } else { Some(cached.stored_at) },
                    from_network_error: false,
                });
            }
        } else if options.offline {
            return Err(format!("{id} is not in the cache"));
        }
    }
    match osv::fetch_advisory(client, id) {
        Ok(advisory) => {
            cache::write_advisory(
                cache_dir,
                &CachedAdvisory {
                    stored_at: options.now,
                    advisory: (&advisory).into(),
                },
            );
            Ok(AdvisoryHit {
                advisory,
                stale_at: None,
                from_network_error: false,
            })
        }
        Err(error) => match cache::read_advisory(cache_dir, id) {
            Some(cached) => Ok(AdvisoryHit {
                advisory: cached.advisory.into(),
                stale_at: Some(cached.stored_at),
                from_network_error: true,
            }),
            None => Err(format!("OSV query failed for {id}: {error}")),
        },
    }
}

fn fill_project(
    project: &mut Prepared,
    packages: &[PackageLookup],
    advisories: &BTreeMap<String, Result<AdvisoryHit, String>>,
) {
    let mut findings = Vec::new();
    let mut errors = Vec::new();
    let mut stale: Option<i64> = None;
    let mut network_fallback = false;
    let owned = project.packages.clone();
    for package in &owned {
        let Some(lookup) = packages.iter().find(|item| item.key == *package) else {
            continue;
        };
        let list = match &lookup.result {
            Ok(list) => list,
            Err(message) => {
                errors.push(message.clone());
                continue;
            }
        };
        note_stale(
            &mut stale,
            &mut network_fallback,
            list.stale_at,
            list.from_network_error,
        );
        for id in &list.ids {
            match advisories.get(id) {
                Some(Ok(hit)) => {
                    note_stale(
                        &mut stale,
                        &mut network_fallback,
                        hit.stale_at,
                        hit.from_network_error,
                    );
                    findings.push(finding(package, &hit.advisory));
                }
                Some(Err(message)) => errors.push(message.clone()),
                None => errors.push(format!("{id} was not loaded")),
            }
        }
    }
    findings.sort_by(|left, right| {
        osv::severity_rank(&left.severity)
            .cmp(&osv::severity_rank(&right.severity))
            .then(left.package.cmp(&right.package))
            .then(left.version.cmp(&right.version))
            .then(left.id.cmp(&right.id))
    });
    findings.dedup_by(|left, right| {
        left.package == right.package && left.version == right.version && left.id == right.id
    });
    project.findings = findings;
    if !errors.is_empty() {
        let mut combined = errors;
        if let Some(existing) = project.error.take() {
            combined.insert(0, existing);
        }
        project.error = Some(combined.join("\n"));
    }
    if network_fallback {
        project.stale_cache_from = stale.and_then(format_unix_timestamp);
    } else if options_offline_stale(stale) {
        project.stale_cache_from = stale.and_then(format_unix_timestamp);
        project.warnings.push(match &project.stale_cache_from {
            Some(when) => format!("using cached results from {when}"),
            None => "using expired cached results".to_owned(),
        });
        project.stale_cache_from = None;
    }
}

fn options_offline_stale(stale: Option<i64>) -> bool {
    stale.is_some()
}

fn note_stale(
    oldest: &mut Option<i64>,
    network: &mut bool,
    stale_at: Option<i64>,
    from_network: bool,
) {
    if from_network {
        *network = true;
    }
    if let Some(stamp) = stale_at {
        *oldest = Some(oldest.map_or(stamp, |current| current.min(stamp)));
    }
}

fn finding(package: &PackageQuery, advisory: &Advisory) -> Finding {
    Finding {
        ecosystem: package.ecosystem.clone(),
        package: package.name.clone(),
        version: package.version.clone(),
        id: advisory.id.clone(),
        aliases: advisory.aliases.clone(),
        severity: advisory.severity.clone(),
        summary: advisory.summary.clone(),
        fixed_versions: osv::fixed_versions(advisory, package),
        url: format!("https://osv.dev/vulnerability/{}", advisory.id),
    }
}
