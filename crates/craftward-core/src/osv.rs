//! Queries and vulnerability records from OSV.
//!
//! Batch requests use `POST https://api.osv.dev/v1/querybatch`. One vulnerability
//! is then read with `GET https://api.osv.dev/v1/vulns/{id}`. The batch endpoint
//! accepts at most 1000 queries. That limit is the one recorded by the official
//! OSV client.

use serde::Deserialize;

use crate::http::{
    HttpClient, HttpError, HttpRequest, MAX_RESPONSE_BYTES, Method, accepted_body_or,
};
use crate::json_loose::{lenient_vec, optional_string, string_or_empty};
use crate::lockfile::PackageQuery;
use crate::percent::encode;
use crate::store::{FindingKind, Severity};
use serde_json::Value;

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
    /// How severe the record is.
    pub severity: Severity,
    /// Packages named by the record, with the versions that fix the issue.
    pub affected: Vec<AffectedPackage>,
    /// The record has a non-empty `withdrawn` timestamp.
    pub withdrawn: bool,
    /// Advisory kind.
    pub kind: FindingKind,
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
    let url = format!("{VULN_URL}{}", encode(id));
    let response = client.execute(&HttpRequest {
        method: Method::Get,
        url: &url,
        body: None,
    })?;
    let body = take_body(response)?;
    parse_advisory(&body).map_err(|message| HttpError { message })
}

/// Reads an OSV vulnerability body.
///
/// # Errors
///
/// Returns a message when the body is not the expected JSON object.
pub fn parse_advisory(body: &[u8]) -> Result<Advisory, String> {
    let object = body
        .iter()
        .find(|byte| !byte.is_ascii_whitespace())
        .is_some_and(|byte| *byte == b'{');
    if !object {
        return match serde_json::from_slice::<serde::de::IgnoredAny>(body) {
            Ok(_) => Err("OSV vulnerability is not an object".to_owned()),
            Err(error) => Err(error.to_string()),
        };
    }
    let record: OsvVuln = serde_json::from_slice(body).map_err(|error| error.to_string())?;
    let id = record
        .id
        .clone()
        .ok_or_else(|| "OSV vulnerability has no id".to_owned())?;
    let kind = classify_kind(&id, &record);
    let severity = record
        .database_specific
        .as_ref()
        .and_then(|specific| specific.severity.as_deref())
        .map_or(Severity::Unknown, map_severity);
    let affected = record
        .affected
        .iter()
        .filter_map(affected_package)
        .collect();
    let withdrawn = record
        .withdrawn
        .as_deref()
        .is_some_and(|text| !text.is_empty());
    let severity = if kind == FindingKind::Malicious {
        Severity::Critical
    } else {
        severity
    };
    Ok(Advisory {
        id,
        aliases: record.aliases,
        summary: record.summary,
        severity,
        affected,
        withdrawn,
        kind,
    })
}

fn classify_kind(id: &str, record: &OsvVuln) -> FindingKind {
    if id.starts_with("MAL-") || has_malicious_category(record) {
        return FindingKind::Malicious;
    }
    match informational(record) {
        Some("unmaintained") => FindingKind::Unmaintained,
        Some("unsound") => FindingKind::Unsound,
        Some("notice" | _) => FindingKind::Notice,
        None => FindingKind::Vulnerability,
    }
}

fn has_malicious_category(record: &OsvVuln) -> bool {
    record.affected.iter().any(|affected| {
        affected.database_specific.as_ref().is_some_and(|specific| {
            specific
                .categories
                .iter()
                .any(|category| category.eq_ignore_ascii_case("malicious"))
        })
    })
}

fn informational(record: &OsvVuln) -> Option<&str> {
    record
        .affected
        .iter()
        .find_map(|affected| note(affected.database_specific.as_ref()))
        .or_else(|| note(record.database_specific.as_ref()))
}

fn note(specific: Option<&DatabaseSpecific>) -> Option<&str> {
    specific
        .and_then(|specific| specific.informational.as_deref())
        .filter(|text| !text.is_empty())
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
            method: Method::Post,
            url: QUERY_URL,
            body: Some(&body),
        })?;
        let body = take_body(response)?;
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
    let queries = selected
        .iter()
        .map(|index| {
            let package = &packages[*index];
            BatchQuery {
                package: BatchPackage {
                    ecosystem: &package.ecosystem,
                    name: &package.name,
                },
                version: &package.version,
                page_token: tokens[*index].as_deref(),
            }
        })
        .collect();
    serde_json::to_vec(&BatchRequest { queries }).map_err(|error| HttpError {
        message: error.to_string(),
    })
}

fn parse_batch(body: &[u8], expected: usize) -> Result<Vec<BatchItem>, HttpError> {
    let response: BatchResponse = serde_json::from_slice(body).map_err(|error| HttpError {
        message: if error.is_syntax() || error.is_eof() {
            format!("OSV batch response is not JSON: {error}")
        } else {
            "OSV batch response has no results".to_owned()
        },
    })?;
    let Some(results) = response.results else {
        return Err(HttpError {
            message: "OSV batch response has no results".to_owned(),
        });
    };
    if results.len() != expected {
        return Err(HttpError {
            message: format!(
                "OSV returned {} results for {expected} queries",
                results.len()
            ),
        });
    }
    Ok(results
        .into_iter()
        .map(|result| BatchItem {
            ids: result.vulns.into_iter().map(|vuln| vuln.id).collect(),
            next: result.next_page_token,
        })
        .collect())
}

fn affected_package(affected: &OsvAffected) -> Option<AffectedPackage> {
    let package = affected.package.as_ref()?;
    let ecosystem = package.ecosystem.as_ref()?.clone();
    let name = package.name.as_ref()?.clone();
    let mut fixed = Vec::new();
    for range in &affected.ranges {
        let kind = range.kind.as_deref().unwrap_or("");
        if kind != "ECOSYSTEM" && kind != "SEMVER" {
            continue;
        }
        for event in &range.events {
            if let Some(version) = &event.fixed
                && !fixed.iter().any(|existing| existing == version)
            {
                fixed.push(version.clone());
            }
        }
    }
    Some(AffectedPackage {
        ecosystem,
        name,
        fixed,
    })
}

fn map_severity(value: &str) -> Severity {
    match value.to_ascii_uppercase().as_str() {
        "LOW" => Severity::Low,
        "MODERATE" => Severity::Medium,
        "HIGH" => Severity::High,
        "CRITICAL" => Severity::Critical,
        _ => Severity::Unknown,
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

fn take_body(response: crate::http::HttpResponse) -> Result<Vec<u8>, HttpError> {
    accepted_body_or(
        response,
        || HttpError {
            message: format!("response exceeded {MAX_RESPONSE_BYTES} bytes"),
        },
        |status| HttpError {
            message: format!("OSV returned HTTP {status}"),
        },
    )
}

#[derive(Deserialize)]
struct OsvVuln {
    #[serde(default, deserialize_with = "optional_string")]
    id: Option<String>,
    #[serde(default, deserialize_with = "lenient_vec")]
    aliases: Vec<String>,
    #[serde(default, deserialize_with = "string_or_empty")]
    summary: String,
    #[serde(default, deserialize_with = "optional_string")]
    withdrawn: Option<String>,
    #[serde(default)]
    database_specific: Option<DatabaseSpecific>,
    #[serde(default, deserialize_with = "lenient_vec")]
    affected: Vec<OsvAffected>,
}

#[derive(Deserialize)]
struct OsvAffected {
    /// Absent or the wrong shape when OSV still carries categories or notices.
    #[serde(default, deserialize_with = "optional_package")]
    package: Option<OsvPackage>,
    #[serde(default, deserialize_with = "lenient_vec")]
    ranges: Vec<OsvRange>,
    #[serde(default)]
    database_specific: Option<DatabaseSpecific>,
}

#[derive(Deserialize)]
struct OsvPackage {
    #[serde(default, deserialize_with = "optional_string")]
    ecosystem: Option<String>,
    #[serde(default, deserialize_with = "optional_string")]
    name: Option<String>,
}

fn optional_package<'de, D>(deserializer: D) -> Result<Option<OsvPackage>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(serde_json::from_value(Value::deserialize(deserializer)?).ok())
}

#[derive(Deserialize)]
struct OsvRange {
    #[serde(rename = "type", default, deserialize_with = "optional_string")]
    kind: Option<String>,
    #[serde(default, deserialize_with = "lenient_vec")]
    events: Vec<OsvEvent>,
}

#[derive(Deserialize)]
struct OsvEvent {
    #[serde(default, deserialize_with = "optional_string")]
    fixed: Option<String>,
}

#[derive(Deserialize, Default)]
struct DatabaseSpecific {
    #[serde(default, deserialize_with = "optional_string")]
    severity: Option<String>,
    #[serde(default, deserialize_with = "optional_string")]
    informational: Option<String>,
    #[serde(default, deserialize_with = "lenient_vec")]
    categories: Vec<String>,
}

#[derive(serde::Serialize)]
struct BatchRequest<'a> {
    queries: Vec<BatchQuery<'a>>,
}

#[derive(serde::Serialize)]
struct BatchQuery<'a> {
    package: BatchPackage<'a>,
    version: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    page_token: Option<&'a str>,
}

#[derive(serde::Serialize)]
struct BatchPackage<'a> {
    ecosystem: &'a str,
    name: &'a str,
}

#[derive(Deserialize)]
struct BatchResponse {
    #[serde(default)]
    results: Option<Vec<BatchResult>>,
}

#[derive(Deserialize)]
struct BatchResult {
    #[serde(default, deserialize_with = "lenient_vec")]
    vulns: Vec<VulnId>,
    #[serde(default, deserialize_with = "optional_string")]
    next_page_token: Option<String>,
}

#[derive(Deserialize)]
struct VulnId {
    id: String,
}

#[cfg(test)]
mod tests {
    use crate::store::{FindingKind, Severity};

    use super::parse_advisory;

    fn body(value: &serde_json::Value) -> Vec<u8> {
        serde_json::to_vec(value).expect("json")
    }

    #[test]
    fn kinds_follow_the_osv_and_rustsec_marks() {
        let vulnerability = parse_advisory(&body(&serde_json::json!({
            "id": "GHSA-1",
            "summary": "hole",
            "affected": [{"package": {"ecosystem": "crates.io", "name": "demo"}}]
        })))
        .unwrap();
        assert_eq!(vulnerability.kind, FindingKind::Vulnerability);
        assert!(!vulnerability.withdrawn);
        assert_eq!(vulnerability.severity, Severity::Unknown);

        let malicious = parse_advisory(&body(&serde_json::json!({
            "id": "MAL-2024-1",
            "database_specific": {"severity": "LOW"},
            "affected": []
        })))
        .unwrap();
        assert_eq!(malicious.kind, FindingKind::Malicious);
        assert_eq!(malicious.severity, Severity::Critical);

        let category = parse_advisory(&body(&serde_json::json!({
            "id": "RUSTSEC-2022-1",
            "affected": [{
                "database_specific": {"categories": ["Malicious"]}
            }]
        })))
        .unwrap();
        assert_eq!(category.kind, FindingKind::Malicious);
        assert_eq!(category.severity, Severity::Critical);

        for (word, kind) in [
            ("unmaintained", FindingKind::Unmaintained),
            ("unsound", FindingKind::Unsound),
            ("notice", FindingKind::Notice),
            ("other-note", FindingKind::Notice),
        ] {
            let advisory = parse_advisory(&body(&serde_json::json!({
                "id": "RUSTSEC-1",
                "affected": [{"database_specific": {"informational": word}}]
            })))
            .unwrap();
            assert_eq!(advisory.kind, kind, "{word}");
        }
    }

    #[test]
    fn a_withdrawn_timestamp_marks_the_record() {
        let advisory = parse_advisory(&body(&serde_json::json!({
            "id": "GHSA-2",
            "withdrawn": "2024-01-01T00:00:00Z",
            "affected": []
        })))
        .unwrap();
        assert!(advisory.withdrawn);
        let empty = parse_advisory(&body(&serde_json::json!({
            "id": "GHSA-3",
            "withdrawn": "",
            "affected": []
        })))
        .unwrap();
        assert!(!empty.withdrawn);
    }
}
