//! OSV matching, cache expiry, and the radar report.

use std::cell::RefCell;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use craftward_core::{
    HttpClient, HttpError, HttpRequest, HttpResponse, MAX_RESPONSE_BYTES, RadarOptions, scan,
};

const NOW: i64 = 1_700_000_000;

struct Fake {
    batches: RefCell<Vec<usize>>,
    fail: bool,
    huge: bool,
    pages: bool,
}

impl HttpClient for Fake {
    fn execute(&self, request: &HttpRequest<'_>) -> Result<HttpResponse, HttpError> {
        if self.fail {
            return Err(HttpError {
                message: "connection refused".to_owned(),
            });
        }
        if self.huge {
            return Ok(HttpResponse {
                status: 200,
                body: vec![0; MAX_RESPONSE_BYTES + 1],
            });
        }
        if request.method == "POST" {
            let value: serde_json::Value =
                serde_json::from_slice(request.body.unwrap_or_default()).expect("batch json");
            let queries = value["queries"].as_array().expect("queries");
            self.batches.borrow_mut().push(queries.len());
            let results = queries
                .iter()
                .map(|query| batch_result(query, self.pages))
                .collect::<Vec<_>>();
            return Ok(HttpResponse {
                status: 200,
                body: serde_json::to_vec(&serde_json::json!({ "results": results })).unwrap(),
            });
        }
        let id = request.url.rsplit('/').next().unwrap_or("");
        Ok(HttpResponse {
            status: 200,
            body: advisory(id).into_bytes(),
        })
    }
}

fn batch_result(query: &serde_json::Value, pages: bool) -> serde_json::Value {
    let name = query["package"]["name"].as_str().unwrap_or("");
    let token = query.get("page_token").and_then(|value| value.as_str());
    if name == "unrated" {
        return serde_json::json!({
            "vulns": [{"id": "GHSA-unknown", "modified": "2026-01-01T00:00:00Z"}]
        });
    }
    if name != "vulnerable" {
        return serde_json::json!({});
    }
    if pages && token.is_none() {
        return serde_json::json!({
            "vulns": [{"id": "GHSA-page", "modified": "2026-01-01T00:00:00Z"}],
            "next_page_token": "more"
        });
    }
    let id = if pages { "GHSA-rest" } else { "GHSA-1111" };
    serde_json::json!({
        "vulns": [{"id": id, "modified": "2026-01-01T00:00:00Z"}]
    })
}

fn advisory(id: &str) -> String {
    let severity = if id == "GHSA-unknown" {
        serde_json::Value::Null
    } else {
        serde_json::json!("HIGH")
    };
    let specific = if severity.is_null() {
        serde_json::json!({})
    } else {
        serde_json::json!({ "severity": severity })
    };
    serde_json::json!({
        "id": id,
        "aliases": ["CVE-2026-1"],
        "summary": "Example hole",
        "database_specific": specific,
        "affected": [{
            "package": {"ecosystem": "crates.io", "name": "vulnerable"},
            "ranges": [{
                "type": "SEMVER",
                "events": [{"introduced": "0.0.0"}, {"fixed": "1.2.3"}]
            }]
        }]
    })
    .to_string()
}

fn scratch(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let path = std::env::temp_dir().join(format!("craftward-radar-{name}-{nanos}"));
    fs::create_dir_all(&path).expect("scratch");
    path
}

fn project(directory: &Path, lockfile: &str) -> PathBuf {
    fs::create_dir_all(directory).unwrap();
    fs::write(directory.join("Cargo.lock"), lockfile).unwrap();
    let path = directory.join("craftward.json");
    fs::write(
        &path,
        r#"{
  "schema_version": 1,
  "id": "demo",
  "name": "Demo",
  "status": "idea",
  "stack": {"components": [], "lockfiles": ["Cargo.lock", "yarn.lock"]}
}
"#,
    )
    .unwrap();
    path
}

fn cargo_lock() -> &'static str {
    "\
version = 4

[[package]]
name = \"vulnerable\"
version = \"1.0.0\"
source = \"registry+https://github.com/rust-lang/crates.io-index\"

[[package]]
name = \"safe\"
version = \"2.0.0\"
source = \"registry+https://github.com/rust-lang/crates.io-index\"
"
}

fn options(refresh: bool, offline: bool, now: i64) -> RadarOptions {
    RadarOptions {
        refresh,
        offline,
        now,
        eol_warning_days: 90,
    }
}

#[test]
fn findings_use_batch_details_and_map_severity() {
    let root = scratch("findings");
    let file = project(&root, cargo_lock());
    let cache = root.join("cache");
    let client = Fake {
        batches: RefCell::new(Vec::new()),
        fail: false,
        huge: false,
        pages: false,
    };
    let reports = scan(&[file], &client, &cache, &options(false, false, NOW));
    let report = &reports[0];
    assert!(report.error.is_none(), "{:?}", report.error);
    assert!(
        report
            .warnings
            .iter()
            .any(|warning| warning.contains("yarn.lock"))
    );
    assert_eq!(report.findings.len(), 1);
    let finding = &report.findings[0];
    assert_eq!(finding.package, "vulnerable");
    assert_eq!(finding.version, "1.0.0");
    assert_eq!(finding.id, "GHSA-1111");
    assert_eq!(finding.aliases, ["CVE-2026-1".to_owned()]);
    assert_eq!(finding.severity, "high");
    assert_eq!(finding.summary, "Example hole");
    assert_eq!(finding.fixed_versions, ["1.2.3".to_owned()]);
    assert_eq!(finding.url, "https://osv.dev/vulnerability/GHSA-1111");
    assert_eq!(client.batches.borrow().as_slice(), [2]);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn large_batches_are_split_and_pages_are_followed() {
    let root = scratch("batch");
    let mut lines = String::from("version = 4\n");
    for index in 0..1001 {
        let _ = writeln!(
            lines,
            "\n[[package]]\nname = \"pkg{index}\"\nversion = \"1.0.0\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\""
        );
    }
    let file = project(&root, &lines);
    let client = Fake {
        batches: RefCell::new(Vec::new()),
        fail: false,
        huge: false,
        pages: false,
    };
    let reports = scan(
        &[file],
        &client,
        &root.join("cache"),
        &options(false, false, NOW),
    );
    assert!(reports[0].error.is_none(), "{:?}", reports[0].error);
    assert_eq!(client.batches.borrow().as_slice(), [1000, 1]);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn pagination_unknown_severity_and_the_size_limit() {
    let root = scratch("page");
    let file = project(&root, cargo_lock());
    let client = Fake {
        batches: RefCell::new(Vec::new()),
        fail: false,
        huge: false,
        pages: true,
    };
    let reports = scan(
        std::slice::from_ref(&file),
        &client,
        &root.join("cache-page"),
        &options(false, false, NOW),
    );
    let ids: Vec<_> = reports[0]
        .findings
        .iter()
        .map(|finding| finding.id.as_str())
        .collect();
    assert!(
        ids.contains(&"GHSA-page") && ids.contains(&"GHSA-rest"),
        "{ids:?}"
    );
    let unrated = "\
version = 4

[[package]]
name = \"unrated\"
version = \"1.0.0\"
source = \"registry+https://github.com/rust-lang/crates.io-index\"
";
    let unrated_file = project(&root.join("unrated"), unrated);
    let reports = scan(
        &[unrated_file],
        &client,
        &root.join("cache-unrated"),
        &options(false, false, NOW),
    );
    assert_eq!(reports[0].findings[0].severity, "unknown");
    let huge = Fake {
        batches: RefCell::new(Vec::new()),
        fail: false,
        huge: true,
        pages: false,
    };
    let rejected = scan(
        &[file],
        &huge,
        &root.join("cache-huge"),
        &options(true, false, NOW),
    );
    assert!(
        rejected[0]
            .error
            .as_deref()
            .unwrap_or("")
            .contains("exceeded"),
        "{:?}",
        rejected[0].error
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn cache_expires_after_one_day_and_seven_days() {
    let root = scratch("ttl");
    let file = project(&root, cargo_lock());
    let cache = root.join("cache");
    let client = Fake {
        batches: RefCell::new(Vec::new()),
        fail: false,
        huge: false,
        pages: false,
    };
    let _ = scan(
        std::slice::from_ref(&file),
        &client,
        &cache,
        &options(false, false, NOW),
    );
    assert_eq!(client.batches.borrow().len(), 1);
    let _ = scan(
        std::slice::from_ref(&file),
        &client,
        &cache,
        &options(false, false, NOW + 86_400 - 1),
    );
    assert_eq!(
        client.batches.borrow().len(),
        1,
        "a fresh query stays cached"
    );
    let _ = scan(
        std::slice::from_ref(&file),
        &client,
        &cache,
        &options(false, false, NOW + 86_400),
    );
    assert_eq!(
        client.batches.borrow().len(),
        2,
        "a query expires after 24 hours"
    );
    let advisory = cache.join("advisories").join("GHSA-1111.json");
    let stored = fs::read_to_string(&advisory).unwrap();
    let aged = stored.replace(&NOW.to_string(), &(NOW - 604_800).to_string());
    fs::write(&advisory, aged).unwrap();
    let later = Fake {
        batches: RefCell::new(Vec::new()),
        fail: false,
        huge: false,
        pages: false,
    };
    let _ = scan(
        std::slice::from_ref(&file),
        &later,
        &cache,
        &options(false, false, NOW),
    );
    assert_eq!(later.batches.borrow().len(), 0, "the query is still fresh");
    let fetched = fs::read_to_string(&advisory).unwrap();
    assert!(
        fetched.contains(&NOW.to_string()),
        "an advisory older than 7 days is fetched again: {fetched}"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn refresh_offline_and_a_network_error_use_the_cache_as_specified() {
    let root = scratch("modes");
    let file = project(&root, cargo_lock());
    let cache = root.join("cache");
    let client = Fake {
        batches: RefCell::new(Vec::new()),
        fail: false,
        huge: false,
        pages: false,
    };
    let _ = scan(
        std::slice::from_ref(&file),
        &client,
        &cache,
        &options(false, false, NOW),
    );
    let _ = scan(
        std::slice::from_ref(&file),
        &client,
        &cache,
        &options(true, false, NOW),
    );
    assert_eq!(client.batches.borrow().len(), 2, "--refresh contacts OSV");
    let offline = Fake {
        batches: RefCell::new(Vec::new()),
        fail: true,
        huge: false,
        pages: false,
    };
    let reports = scan(
        std::slice::from_ref(&file),
        &offline,
        &cache,
        &options(false, true, NOW),
    );
    assert!(reports[0].error.is_none(), "{:?}", reports[0].error);
    assert_eq!(reports[0].findings.len(), 1);
    assert!(reports[0].stale_cache_from.is_none());
    let expired = scan(
        std::slice::from_ref(&file),
        &offline,
        &cache,
        &options(false, true, NOW + 86_400),
    );
    assert!(expired[0].error.is_none(), "{:?}", expired[0].error);
    assert!(
        expired[0]
            .warnings
            .iter()
            .any(|warning| warning.contains("using cached results from")),
        "{:?}",
        expired[0].warnings
    );
    let broken = Fake {
        batches: RefCell::new(Vec::new()),
        fail: true,
        huge: false,
        pages: false,
    };
    let fallback = scan(&[file], &broken, &cache, &options(true, false, NOW));
    assert!(fallback[0].error.is_none(), "{:?}", fallback[0].error);
    assert!(fallback[0].stale_cache_from.is_some());
    let text = craftward_core::render_text(&fallback);
    assert!(text.contains("after a network error"), "{text}");
    let empty = root.join("empty");
    fs::create_dir_all(&empty).unwrap();
    let empty_file = empty.join("craftward.json");
    fs::write(
        &empty_file,
        "{\n  \"schema_version\": 1,\n  \"id\": \"empty\",\n  \"name\": \"Empty\",\n  \"status\": \"idea\"\n}\n",
    )
    .unwrap();
    let quiet = scan(&[empty_file], &broken, &cache, &options(false, true, NOW));
    let text = craftward_core::render_text(&quiet);
    assert!(text.contains("no lockfiles configured"), "{text}");
    let _ = fs::remove_dir_all(root);
}
