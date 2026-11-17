//! Queries and vulnerability records from OSV.
//!
//! Batch requests use `POST https://api.osv.dev/v1/querybatch`. One vulnerability
//! is then read with `GET https://api.osv.dev/v1/vulns/{id}`. The batch endpoint
//! accepts at most 1000 queries. That limit is the one recorded by the official
//! OSV client.

use serde::Deserialize;

use crate::cvss::{score_v3, severity_from_qualitative};
use crate::http::{
    HttpClient, HttpError, HttpRequest, MAX_RESPONSE_BYTES, Method, accepted_body_or,
};
use crate::json_loose::{
    lenient_vec, optional_strict_string, optional_string, strict_vec, string_or_empty,
};
use crate::lockfile::PackageQuery;
use crate::percent::encode;
use crate::store::{FindingKind, Severity, SeveritySource};
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
    /// Where [`Self::severity`] came from.
    pub severity_source: SeveritySource,
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
    let (mut severity, mut severity_source) = resolve_severity(&record);
    let affected = record
        .affected
        .iter()
        .filter_map(affected_package)
        .collect();
    let withdrawn = record
        .withdrawn
        .as_deref()
        .is_some_and(|text| !text.is_empty());
    if kind == FindingKind::Malicious {
        severity = Severity::Critical;
        severity_source = SeveritySource::Advisory;
    }
    Ok(Advisory {
        id,
        aliases: record.aliases,
        summary: record.summary,
        severity,
        severity_source,
        affected,
        withdrawn,
        kind,
    })
}

fn resolve_severity(record: &OsvVuln) -> (Severity, SeveritySource) {
    if let Some(label) = record
        .database_specific
        .as_ref()
        .and_then(|specific| specific.severity.as_deref())
    {
        let mapped = map_severity(label);
        if mapped != Severity::Unknown {
            return (mapped, SeveritySource::Advisory);
        }
    }
    let mut best: Option<(Severity, SeveritySource)> = None;
    for entry in &record.severity {
        let candidate = match entry.kind.as_deref() {
            Some("CVSS_V3" | "CVSS_V3_1" | "CVSS_V3.1" | "CVSS_V3.0") => entry
                .score
                .as_deref()
                .and_then(score_v3)
                .map(|(_, severity)| (severity, SeveritySource::Cvss)),
            Some("CVSS_V4" | "CVSS_V4_0") => entry
                .score
                .as_deref()
                .and_then(severity_from_qualitative)
                .or_else(|| {
                    entry
                        .score
                        .as_deref()
                        .and_then(|text| text.split('/').next())
                        .and_then(severity_from_qualitative)
                })
                .map(|severity| (severity, SeveritySource::Advisory)),
            _ => entry
                .score
                .as_deref()
                .and_then(severity_from_qualitative)
                .map(|severity| (severity, SeveritySource::Advisory)),
        };
        if let Some((severity, source)) = candidate {
            let replace =
                best.is_none_or(|(current, _)| severity_rank(severity) < severity_rank(current));
            if replace {
                best = Some((severity, source));
            }
        }
    }
    best.unwrap_or((Severity::Unknown, SeveritySource::Unknown))
}

fn severity_rank(severity: Severity) -> u8 {
    match severity {
        Severity::Critical => 0,
        Severity::High => 1,
        Severity::Medium => 2,
        Severity::Low => 3,
        Severity::Unknown => 4,
    }
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

#[derive(Debug)]
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
    let response: BatchResponse = serde_json::from_slice(body).map_err(|error| {
        let detail = error.to_string();
        HttpError {
            message: if error.is_syntax() || error.is_eof() {
                format!("OSV batch response is not JSON: {error}")
            } else if detail.contains("expected array")
                || detail.contains("vulnerability id")
                || detail.contains("missing field")
            {
                format!("OSV batch response is invalid: {error}")
            } else {
                "OSV batch response has no results".to_owned()
            },
        }
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
    severity: Vec<OsvSeverity>,
    #[serde(default, deserialize_with = "strict_vec")]
    affected: Vec<OsvAffected>,
}

#[derive(Deserialize)]
struct OsvSeverity {
    #[serde(rename = "type", default, deserialize_with = "optional_string")]
    kind: Option<String>,
    #[serde(default, deserialize_with = "optional_string")]
    score: Option<String>,
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
    #[serde(default, deserialize_with = "optional_strict_string")]
    informational: Option<String>,
    #[serde(default, deserialize_with = "strict_vec")]
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
    #[serde(default, deserialize_with = "strict_vulns")]
    vulns: Vec<VulnId>,
    #[serde(default, deserialize_with = "optional_string")]
    next_page_token: Option<String>,
}

#[derive(Deserialize)]
struct VulnId {
    id: String,
}

fn strict_vulns<'de, D>(deserializer: D) -> Result<Vec<VulnId>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let vulns: Vec<VulnId> = strict_vec(deserializer)?;
    for vuln in &vulns {
        if vuln.id.is_empty() {
            return Err(serde::de::Error::custom("vulnerability id is empty"));
        }
    }
    Ok(vulns)
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

    #[test]
    fn affected_must_be_an_array_when_present() {
        let error = parse_advisory(&body(&serde_json::json!({
            "id": "GHSA-bad",
            "affected": {"package": {"name": "demo"}}
        })))
        .expect_err("affected object");
        assert!(
            error.contains("expected array") || error.contains("invalid type"),
            "{error}"
        );
    }

    #[test]
    fn categories_and_informational_must_have_their_documented_types() {
        let categories = parse_advisory(&body(&serde_json::json!({
            "id": "RUSTSEC-1",
            "affected": [{"database_specific": {"categories": "Malicious"}}]
        })))
        .expect_err("categories string");
        assert!(
            categories.contains("expected array") || categories.contains("invalid type"),
            "{categories}"
        );
        let informational = parse_advisory(&body(&serde_json::json!({
            "id": "RUSTSEC-1",
            "affected": [{"database_specific": {"informational": 1}}]
        })))
        .expect_err("informational number");
        assert!(
            informational.contains("expected string") || informational.contains("invalid type"),
            "{informational}"
        );
    }

    #[test]
    fn severity_prefers_ghsa_label_then_cvss_v3() {
        use crate::store::SeveritySource;

        let advisory = parse_advisory(&body(&serde_json::json!({
            "id": "GHSA-1",
            "database_specific": {"severity": "HIGH"},
            "severity": [{
                "type": "CVSS_V3",
                "score": "CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:H/A:H"
            }],
            "affected": []
        })))
        .unwrap();
        assert_eq!(advisory.severity, Severity::High);
        assert_eq!(advisory.severity_source, SeveritySource::Advisory);

        let from_cvss = parse_advisory(&body(&serde_json::json!({
            "id": "GHSA-2",
            "severity": [{
                "type": "CVSS_V3",
                "score": "CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:H/A:H"
            }],
            "affected": []
        })))
        .unwrap();
        assert_eq!(from_cvss.severity, Severity::Critical);
        assert_eq!(from_cvss.severity_source, SeveritySource::Cvss);

        let v4_label = parse_advisory(&body(&serde_json::json!({
            "id": "GHSA-3",
            "severity": [{
                "type": "CVSS_V4",
                "score": "CRITICAL"
            }],
            "affected": []
        })))
        .unwrap();
        assert_eq!(v4_label.severity, Severity::Critical);

        let v4_vector_only = parse_advisory(&body(&serde_json::json!({
            "id": "GHSA-4",
            "severity": [{
                "type": "CVSS_V4",
                "score": "CVSS:4.0/AV:N/AC:L/AT:N/PR:N/UI:N/VC:H/VI:H/VA:H/SC:N/SI:N/SA:N"
            }],
            "affected": []
        })))
        .unwrap();
        assert_eq!(v4_vector_only.severity, Severity::Unknown);
        assert_eq!(v4_vector_only.severity_source, SeveritySource::Unknown);
    }

    #[test]
    fn batch_vulns_must_be_an_array_of_usable_ids() {
        use super::parse_batch;

        let ok = parse_batch(
            &body(&serde_json::json!({
                "results": [{"vulns": [{"id": "GHSA-1"}]}]
            })),
            1,
        )
        .unwrap();
        assert_eq!(ok[0].ids, ["GHSA-1"]);

        let missing = parse_batch(&body(&serde_json::json!({ "results": [{}] })), 1).unwrap();
        assert_eq!(missing[0].ids, Vec::<String>::new());

        let wrong_type = parse_batch(
            &body(&serde_json::json!({
                "results": [{"vulns": {"id": "GHSA-1"}}]
            })),
            1,
        )
        .expect_err("vulns object");
        assert!(
            wrong_type.message.contains("invalid") || wrong_type.message.contains("array"),
            "{}",
            wrong_type.message
        );

        let empty_id = parse_batch(
            &body(&serde_json::json!({
                "results": [{"vulns": [{"id": ""}]}]
            })),
            1,
        )
        .expect_err("empty id");
        assert!(
            empty_id.message.contains("invalid") || empty_id.message.contains("empty"),
            "{}",
            empty_id.message
        );

        let no_id = parse_batch(
            &body(&serde_json::json!({
                "results": [{"vulns": [{"modified": "2026-01-01T00:00:00Z"}]}]
            })),
            1,
        )
        .expect_err("missing id");
        assert!(
            no_id.message.contains("invalid") || no_id.message.contains("missing"),
            "{}",
            no_id.message
        );
    }
}
