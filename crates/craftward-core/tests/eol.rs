//! endoflife.date matching, without using the network.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use craftward_core::{
    Component, ComponentKind, FindingKind, HttpClient, HttpError, HttpRequest, HttpResponse,
    Severity, check_components,
};

const TODAY: &str = "2026-11-15";

struct Fake;

impl HttpClient for Fake {
    fn execute(&self, request: &HttpRequest<'_>) -> Result<HttpResponse, HttpError> {
        let body = if request.url.ends_with("/products/") {
            serde_json::json!({
                "schema_version": "1.2.1",
                "generated_at": "2026-11-15T00:00:00Z",
                "total": 4,
                "result": [
                    {"name": "nodejs", "aliases": ["node.js"]},
                    {"name": "node", "aliases": ["nodejs"]},
                    {"name": "postgresql", "aliases": ["postgres"]},
                    {"name": "ubuntu", "aliases": []},
                    {"name": "flags", "aliases": []}
                ]
            })
        } else if request.url.contains("/postgresql/") {
            serde_json::json!({
                "result": {"releases": [
                    {"name": "9", "isEol": true, "eolFrom": "2020-11-12"},
                    {"name": "9.3", "isEol": false, "eolFrom": "2026-11-15"},
                    {"name": "16", "isEol": false, "eolFrom": null}
                ]}
            })
        } else if request.url.contains("/ubuntu/") {
            serde_json::json!({
                "result": {"releases": [
                    {"name": "22.04", "isEol": false, "eolFrom": "2027-02-13"}
                ]}
            })
        } else if request.url.contains("/flags/") {
            serde_json::json!({
                "result": {"releases": [
                    {"name": "1", "isEol": true, "eolFrom": "2020-01-01"},
                    {"name": "2", "isEol": true, "eolFrom": null},
                    {"name": "3", "isEol": false, "eolFrom": "2027-01-14"},
                    {"name": "4", "isEol": false, "eolFrom": null},
                    {"name": "5", "isEol": true, "eolFrom": "2027-06-01"},
                    {"name": "6", "isEol": false, "eolFrom": "2020-01-01"},
                    {"name": "7", "isEol": true, "eolFrom": "soon"}
                ]}
            })
        } else {
            return Ok(HttpResponse {
                status: 404,
                body: Vec::new(),
            });
        };
        Ok(HttpResponse {
            status: 200,
            body: serde_json::to_vec(&body).unwrap(),
        })
    }
}

fn scratch() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let path = std::env::temp_dir().join(format!("craftward-eol-{nanos}"));
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn component(name: &str, version: Option<&str>, product: Option<&str>) -> Component {
    Component {
        kind: ComponentKind::Runtime,
        name: name.to_owned(),
        version: version.map(str::to_owned),
        note: None,
        eol_product: product.map(str::to_owned),
    }
}

fn check(components: &[Component], days: u32) -> craftward_core::EolReport {
    let cache = scratch();
    check_components(
        components,
        &Fake,
        &craftward_core::EolOptions {
            cache_dir: &cache,
            refresh: false,
            offline: false,
            now: 0,
            today: TODAY,
            warning_days: days,
        },
    )
}

#[test]
fn product_and_cycle_rules() {
    let unique = check(&[component("PostgreSQL", Some("9.2.0"), None)], 90);
    assert!(unique.error.is_none(), "{:?}", unique.error);
    assert_eq!(unique.hits.len(), 1);
    assert_eq!(unique.hits[0].id, "EOL:postgresql:9");
    assert_eq!(unique.hits[0].kind, FindingKind::Eol);
    assert_eq!(unique.hits[0].severity, Severity::High);

    let prefix = check(&[component("postgres", Some("9.3.0"), None)], 90);
    assert_eq!(prefix.hits[0].id, "EOL:postgresql:9.3");
    assert_eq!(prefix.hits[0].kind, FindingKind::EolSoon);
    assert_eq!(prefix.hits[0].severity, Severity::Medium);
    assert_eq!(prefix.hits[0].url, "https://endoflife.date/postgresql");

    let missing_cycle = check(&[component("PostgreSQL", Some("8.0.0"), None)], 90);
    assert_eq!(missing_cycle.hits.len(), 0);
    assert!(
        missing_cycle
            .warnings
            .iter()
            .any(|warning| warning.contains("no endoflife.date cycle")),
        "{:?}",
        missing_cycle.warnings
    );

    let supported = check(&[component("PostgreSQL", Some("16.1"), None)], 90);
    assert_eq!(supported.hits.len(), 0);
    assert_eq!(supported.warnings.len(), 0, "{:?}", supported.warnings);

    let ambiguous = check(&[component("Node.js", Some("22.0.0"), None)], 90);
    assert_eq!(ambiguous.hits.len(), 0);
    assert!(
        ambiguous
            .warnings
            .iter()
            .any(|warning| { warning == "no endoflife.date product for Node.js; set eol_product" }),
        "{:?}",
        ambiguous.warnings
    );

    let explicit = check(&[component("Linux", Some("22.04.5"), Some("ubuntu"))], 90);
    assert_eq!(explicit.hits[0].id, "EOL:ubuntu:22.04");
    assert_eq!(explicit.hits[0].kind, FindingKind::EolSoon);

    let soon_window = check(&[component("Linux", Some("22.04.5"), Some("ubuntu"))], 30);
    assert_eq!(soon_window.hits.len(), 0, "{:?}", soon_window.hits);

    let unknown = check(&[component("Linux", Some("1"), Some("missing"))], 90);
    assert!(
        unknown
            .warnings
            .iter()
            .any(|warning| warning.contains("set eol_product")),
        "{:?}",
        unknown.warnings
    );

    let skipped = check(&[component("PostgreSQL", None, None)], 90);
    assert!(!skipped.checked);
    assert_eq!(skipped.hits.len(), 0);
    assert_eq!(skipped.warnings.len(), 0);
}

#[test]
fn support_flags_none_and_a_newer_version() {
    let ended = check(&[component("Old", Some("1.2.0"), Some("flags"))], 90);
    assert_eq!(ended.hits.len(), 1);
    assert_eq!(ended.hits[0].kind, FindingKind::Eol);
    assert_eq!(ended.hits[0].summary, "support ended on 2020-01-01");
    assert_eq!(ended.warnings.len(), 0, "{:?}", ended.warnings);

    let unknown = check(&[component("Old", Some("2.0"), Some("flags"))], 90);
    assert_eq!(unknown.hits.len(), 1);
    assert_eq!(unknown.hits[0].kind, FindingKind::Eol);
    assert_eq!(unknown.hits[0].severity, Severity::High);
    assert_eq!(unknown.hits[0].summary, "support ended on unknown");
    assert_eq!(unknown.warnings.len(), 0, "{:?}", unknown.warnings);

    let soon = check(&[component("Current", Some("3.1"), Some("flags"))], 90);
    assert_eq!(soon.hits.len(), 1);
    assert_eq!(soon.hits[0].kind, FindingKind::EolSoon);
    assert_eq!(soon.hits[0].summary, "support ends on 2027-01-14");
    let outside = check(&[component("Current", Some("3.1"), Some("flags"))], 30);
    assert_eq!(outside.hits.len(), 0);
    assert_eq!(outside.warnings.len(), 0, "{:?}", outside.warnings);

    let current = check(&[component("Current", Some("4.1"), Some("flags"))], 90);
    assert_eq!(current.hits.len(), 0);
    assert_eq!(current.warnings.len(), 0, "{:?}", current.warnings);
    assert!(current.checked);

    let disabled = check(&[component("PostgreSQL", Some("9.2.0"), Some("none"))], 90);
    assert!(!disabled.checked);
    assert_eq!(disabled.hits.len(), 0);
    assert_eq!(disabled.warnings.len(), 0, "{:?}", disabled.warnings);

    let newer = check(&[component("PostgreSQL", Some("17.0"), None)], 90);
    assert_eq!(newer.hits.len(), 0);
    assert_eq!(
        newer.warnings,
        vec!["PostgreSQL 17.0 is newer than any cycle known to endoflife.date".to_owned()]
    );

    let future_flag = check(&[component("Odd", Some("5.0"), Some("flags"))], 90);
    assert_eq!(future_flag.hits.len(), 0);
    assert!(
        future_flag
            .warnings
            .iter()
            .any(|warning| warning.contains("contradictory")),
        "{:?}",
        future_flag.warnings
    );
    let past_flag = check(&[component("Odd", Some("6.0"), Some("flags"))], 90);
    assert!(
        past_flag
            .warnings
            .iter()
            .any(|warning| warning.contains("contradictory"))
    );
    let broken = check(&[component("Odd", Some("7.0"), Some("flags"))], 90);
    assert!(
        broken
            .warnings
            .iter()
            .any(|warning| warning.contains("unusable"))
    );
}
