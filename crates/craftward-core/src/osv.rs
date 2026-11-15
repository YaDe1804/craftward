//! Queries and vulnerability records from OSV.
//!
//! Batch requests use `POST https://api.osv.dev/v1/querybatch`. One vulnerability
//! is then read with `GET https://api.osv.dev/v1/vulns/{id}`. The batch endpoint
//! accepts at most 1000 queries. That limit is the one recorded by the official
//! OSV client.

use std::fmt::Write as _;

use serde_json::{Map, Value};

use crate::http::{HttpClient, HttpError, HttpRequest, HttpResponse, MAX_RESPONSE_BYTES};
use crate::lockfile::PackageQuery;

/// Queries in one `querybatch` request.
pub const BATCH_LIMIT: usize = 1000;

/// How long a package query may be reused.
pub const QUERY_TTL_SECONDS: i64 = 24 * 60 * 60;

/// How long a vulnerability record may be reused.
pub const ADVISORY_TTL_SECONDS: i64 = 7 * 24 * 60 * 60;

const QUERY_URL: &str = "https://api.osv.dev/v1/querybatch";
const VULN_URL: &str = "https://api.osv.dev/v1/vulns/";

/// A vulnerability record reduced to the fields the radar shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Advisory {
    /// OSV id.
    pub id: String,
    /// Other ids, such as CVE ids.
    pub aliases: Vec<String>,
    /// Short description. Empty when OSV omitted `summary`.
    pub summary: String,
    /// `low`, `medium`, `high`, `critical`, or `unknown`.
    pub severity: String,
    /// Packages named by the record, with the versions that fix the issue.
    pub affected: Vec<AffectedPackage>,
}

/// One affected package inside an [`Advisory`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AffectedPackage {
    /// OSV ecosystem name.
    pub ecosystem: String,
    /// Package name.
    pub name: String,
    /// Versions in which the vulnerability is fixed.
    pub fixed: Vec<String>,
}

/// Asks OSV which vulnerability ids apply to each package.
///
/// The returned vectors line up with `packages`. Requests are split into
/// batches of [`BATCH_LIMIT`]. A `next_page_token` is followed until the page
/// is complete.
///
/// # Errors
///
/// Returns [`HttpError`] when a request fails or the response is not the
/// documented batch shape.
pub fn query_ids(
    client: &impl HttpClient,
    packages: &[PackageQuery],
) -> Result<Vec<Vec<String>>, HttpError> {
    let mut ids = Vec::with_capacity(packages.len());
    for chunk in packages.chunks(BATCH_LIMIT) {
        ids.extend(query_chunk(client, chunk)?);
    }
    Ok(ids)
}

/// Downloads one vulnerability.
///
/// # Errors
///
/// Returns [`HttpError`] when the request fails or the body is not a
/// vulnerability object.
pub fn fetch_advisory(client: &impl HttpClient, id: &str) -> Result<Advisory, HttpError> {
    let url = format!("{VULN_URL}{}", encode_path(id));
    let response = client.execute(&HttpRequest {
        method: "GET",
        url: &url,
        body: None,
    })?;
    let body = accepted_body(response)?;
    parse_advisory(&body).map_err(|message| HttpError { message })
}

/// Reads an OSV vulnerability body.
///
/// # Errors
///
/// Returns a message when the body is not the expected JSON object.
pub fn parse_advisory(body: &[u8]) -> Result<Advisory, String> {
    let value: Value = serde_json::from_slice(body).map_err(|error| error.to_string())?;
    let Some(object) = value.as_object() else {
        return Err("OSV vulnerability is not an object".to_owned());
    };
    let id = required_string(object, "id")?;
    let aliases = string_list(object.get("aliases"));
    let summary = object
        .get("summary")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let severity = object
        .get("database_specific")
        .and_then(Value::as_object)
        .and_then(|specific| specific.get("severity"))
        .and_then(Value::as_str)
        .map_or_else(|| "unknown".to_owned(), map_severity);
    let affected = object
        .get("affected")
        .and_then(Value::as_array)
        .map_or_default(|items| items.iter().filter_map(affected_package).collect());
    Ok(Advisory {
        id,
        aliases,
        summary,
        severity,
        affected,
    })
}

/// Fixed versions that apply to `package`.
#[must_use]
pub fn fixed_versions(advisory: &Advisory, package: &PackageQuery) -> Vec<String> {
    let mut versions = Vec::new();
    for affected in &advisory.affected {
        if affected.ecosystem != package.ecosystem
            || !same_name(&affected.name, &package.name, &package.ecosystem)
        {
            continue;
        }
        for version in &affected.fixed {
            if !versions.iter().any(|existing| existing == version) {
                versions.push(version.clone());
            }
        }
    }
    versions
}

/// Sort key. Smaller values are more severe.
#[must_use]
pub fn severity_rank(severity: &str) -> u8 {
    match severity {
        "critical" => 0,
        "high" => 1,
        "medium" => 2,
        "low" => 3,
        _ => 4,
    }
}

fn query_chunk(
    client: &impl HttpClient,
    packages: &[PackageQuery],
) -> Result<Vec<Vec<String>>, HttpError> {
    let mut collected = vec![Vec::new(); packages.len()];
    let mut tokens = vec![None; packages.len()];
    let mut first = true;
    for _ in 0..10 {
        let selected: Vec<usize> = if first {
            (0..packages.len()).collect()
        } else {
            tokens
                .iter()
                .enumerate()
                .filter_map(|(index, token)| token.as_ref().map(|_| index))
                .collect()
        };
        if selected.is_empty() {
            return Ok(collected);
        }
        let body = batch_body(packages, &selected, &tokens)?;
        let response = client.execute(&HttpRequest {
            method: "POST",
            url: QUERY_URL,
            body: Some(&body),
        })?;
        let body = accepted_body(response)?;
        let page = parse_batch(&body, selected.len())?;
        let mut next = vec![None; packages.len()];
        for (offset, item) in page.into_iter().enumerate() {
            let index = selected[offset];
            collected[index].extend(item.ids);
            next[index] = item.next;
        }
        first = false;
        if next.iter().all(Option::is_none) {
            return Ok(collected);
        }
        tokens = next;
    }
    Err(HttpError {
        message: "OSV pagination did not finish".to_owned(),
    })
}

struct BatchItem {
    ids: Vec<String>,
    next: Option<String>,
}

fn batch_body(
    packages: &[PackageQuery],
    selected: &[usize],
    tokens: &[Option<String>],
) -> Result<Vec<u8>, HttpError> {
    let queries: Vec<Value> = selected
        .iter()
        .map(|index| {
            let package = &packages[*index];
            let mut query = Map::new();
            let mut named = Map::new();
            named.insert(
                "ecosystem".to_owned(),
                Value::String(package.ecosystem.clone()),
            );
            named.insert("name".to_owned(), Value::String(package.name.clone()));
            query.insert("package".to_owned(), Value::Object(named));
            query.insert("version".to_owned(), Value::String(package.version.clone()));
            if let Some(token) = &tokens[*index] {
                query.insert("page_token".to_owned(), Value::String(token.clone()));
            }
            Value::Object(query)
        })
        .collect();
    let mut root = Map::new();
    root.insert("queries".to_owned(), Value::Array(queries));
    serde_json::to_vec(&Value::Object(root)).map_err(|error| HttpError {
        message: error.to_string(),
    })
}

fn parse_batch(body: &[u8], expected: usize) -> Result<Vec<BatchItem>, HttpError> {
    let value: Value = serde_json::from_slice(body).map_err(|error| HttpError {
        message: format!("OSV batch response is not JSON: {error}"),
    })?;
    let results = value
        .get("results")
        .and_then(Value::as_array)
        .ok_or_else(|| HttpError {
            message: "OSV batch response has no results".to_owned(),
        })?;
    if results.len() != expected {
        return Err(HttpError {
            message: format!(
                "OSV returned {} results for {expected} queries",
                results.len()
            ),
        });
    }
    results
        .iter()
        .map(|result| {
            let Some(object) = result.as_object() else {
                return Err(HttpError {
                    message: "OSV batch result is not an object".to_owned(),
                });
            };
            let ids = object
                .get("vulns")
                .and_then(Value::as_array)
                .map_or_default(|vulns| {
                    vulns
                        .iter()
                        .filter_map(|vuln| {
                            vuln.get("id").and_then(Value::as_str).map(str::to_owned)
                        })
                        .collect()
                });
            let next = object
                .get("next_page_token")
                .and_then(Value::as_str)
                .map(str::to_owned);
            Ok(BatchItem { ids, next })
        })
        .collect()
}

fn affected_package(value: &Value) -> Option<AffectedPackage> {
    let object = value.as_object()?;
    let package = object.get("package")?.as_object()?;
    let ecosystem = package.get("ecosystem")?.as_str()?.to_owned();
    let name = package.get("name")?.as_str()?.to_owned();
    let mut fixed = Vec::new();
    if let Some(ranges) = object.get("ranges").and_then(Value::as_array) {
        for range in ranges {
            let Some(range) = range.as_object() else {
                continue;
            };
            let kind = range.get("type").and_then(Value::as_str).unwrap_or("");
            if kind != "ECOSYSTEM" && kind != "SEMVER" {
                continue;
            }
            let Some(events) = range.get("events").and_then(Value::as_array) else {
                continue;
            };
            for event in events {
                if let Some(version) = event.get("fixed").and_then(Value::as_str)
                    && !fixed.iter().any(|existing| existing == version)
                {
                    fixed.push(version.to_owned());
                }
            }
        }
    }
    Some(AffectedPackage {
        ecosystem,
        name,
        fixed,
    })
}

fn map_severity(value: &str) -> String {
    match value.to_ascii_uppercase().as_str() {
        "LOW" => "low".to_owned(),
        "MODERATE" => "medium".to_owned(),
        "HIGH" => "high".to_owned(),
        "CRITICAL" => "critical".to_owned(),
        _ => "unknown".to_owned(),
    }
}

fn same_name(left: &str, right: &str, ecosystem: &str) -> bool {
    if ecosystem == "crates.io" {
        let fold = |text: &str| text.replace('_', "-").to_ascii_lowercase();
        fold(left) == fold(right)
    } else {
        left == right
    }
}

fn required_string(object: &Map<String, Value>, key: &str) -> Result<String, String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("OSV vulnerability has no {key}"))
}

fn string_list(value: Option<&Value>) -> Vec<String> {
    value.and_then(Value::as_array).map_or_default(|items| {
        items
            .iter()
            .filter_map(|item| item.as_str().map(str::to_owned))
            .collect()
    })
}

fn accepted_body(response: HttpResponse) -> Result<Vec<u8>, HttpError> {
    if response.body.len() > MAX_RESPONSE_BYTES {
        return Err(HttpError {
            message: format!("response exceeded {MAX_RESPONSE_BYTES} bytes"),
        });
    }
    if !(200..300).contains(&response.status) {
        return Err(HttpError {
            message: format!("OSV returned HTTP {}", response.status),
        });
    }
    Ok(response.body)
}

fn encode_path(text: &str) -> String {
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
