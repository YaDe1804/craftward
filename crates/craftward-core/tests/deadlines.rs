//! Play deadline data, targetSdk detection, and deadline findings.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use craftward_core::{
    AndroidConfig, AndroidStore, CatalogOrigin, FindingKind, FormFactor, HttpClient, HttpError,
    HttpRequest, HttpResponse, RadarOptions, Severity, detect_target_sdk, load_deadlines,
    parse_document, render_deadlines, scan, stale_warning, utc_day,
};

const EMBEDDED: &str = include_str!("../../../data/platform-deadlines.json");
/// 2026-11-15 00:00:00 UTC.
const TODAY: i64 = 1_794_700_800;

struct Quiet;

impl HttpClient for Quiet {
    fn execute(&self, _request: &HttpRequest<'_>) -> Result<HttpResponse, HttpError> {
        Err(HttpError {
            message: "not used".to_owned(),
        })
    }
}

fn scratch(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let path = std::env::temp_dir().join(format!("craftward-deadlines-{name}-{nanos}"));
    fs::create_dir_all(&path).expect("scratch");
    path
}

fn options(now: i64, warning_days: u32) -> RadarOptions {
    RadarOptions {
        refresh: false,
        offline: true,
        now,
        eol_warning_days: 90,
        deadline_warning_days: warning_days,
    }
}

fn android(detect_from: &[&str]) -> AndroidConfig {
    AndroidConfig {
        target_sdk: None,
        min_sdk: None,
        form_factor: FormFactor::Phone,
        store: AndroidStore::GooglePlay,
        detect_from: detect_from.iter().map(|path| (*path).to_owned()).collect(),
    }
}

fn write(root: &Path, relative: &str, text: &str) {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, text).unwrap();
}

#[test]
fn the_shipped_file_is_valid_and_dated() {
    let parsed = parse_document(EMBEDDED).expect("embedded deadlines");
    assert_eq!(parsed.data_version, "2026-11-15");
    assert!(parsed.entries.iter().any(|entry| {
        entry.form_factor == "phone"
            && entry.requirement == "new_apps_and_updates"
            && entry.api_level == 36
            && entry.deadline == "2026-08-31"
            && entry.source.starts_with("https://")
            && entry.checked_on == "2026-11-15"
    }));
    assert!(parsed.entries.iter().any(|entry| entry.form_factor == "xr"));
}

#[test]
fn invalid_deadline_files_are_rejected() {
    let missing = EMBEDDED.replace("\"note\":", "\"notes\":");
    assert!(parse_document(&missing).is_err());
    let date = EMBEDDED.replace("2026-08-31", "2026-02-31");
    let error = parse_document(&date).expect_err("bad date");
    assert!(error.contains("YYYY-MM-DD"), "{error}");
    let url = EMBEDDED.replace("https://support.google.com", "http://support.google.com");
    let error = parse_document(&url).expect_err("http");
    assert!(error.contains("https"), "{error}");
    let duplicate = r#"{
        "schema_version": 1,
        "data_version": "2026-11-15",
        "deadlines": [
          {
            "platform": "google_play", "form_factor": "phone",
            "requirement": "existing_apps", "api_level": 35,
            "deadline": "2026-08-31",
            "source": "https://example.com/a", "checked_on": "2026-11-15",
            "note": "one"
          },
          {
            "platform": "google_play", "form_factor": "phone",
            "requirement": "existing_apps", "api_level": 34,
            "deadline": "2026-08-31",
            "source": "https://example.com/b", "checked_on": "2026-11-15",
            "note": "two"
          }
        ]
    }"#;
    let error = parse_document(duplicate).expect_err("duplicate");
    assert!(error.contains("duplicates"), "{error}");
}

#[test]
fn staleness_uses_a_fixed_day_and_the_newer_file_wins() {
    assert!(stale_warning("2026-11-15", "2027-11-15").is_none());
    let stale = stale_warning("2026-11-15", "2027-11-16").expect("366 days");
    assert!(stale.contains("may be out of date"), "{stale}");

    let root = scratch("catalog");
    let older = root.join("older");
    fs::create_dir_all(&older).unwrap();
    fs::write(
        older.join("platform-deadlines.json"),
        sample("2020-01-01", 1, "2020-01-01"),
    )
    .unwrap();
    let loaded = load_deadlines(&older).unwrap();
    assert_eq!(loaded.origin, CatalogOrigin::Builtin);
    assert_eq!(loaded.ignored_older.as_deref(), Some("2020-01-01"));
    let text = render_deadlines(&loaded, "2026-11-15");
    assert!(text.contains("built-in data"), "{text}");
    assert!(text.contains("older and was not used"), "{text}");

    let newer = root.join("newer");
    fs::create_dir_all(&newer).unwrap();
    fs::write(
        newer.join("platform-deadlines.json"),
        sample("2026-12-01", 40, "2026-12-01"),
    )
    .unwrap();
    let loaded = load_deadlines(&newer).unwrap();
    assert_eq!(loaded.origin, CatalogOrigin::DataDirectory);
    assert_eq!(loaded.data_version, "2026-12-01");
    let text = render_deadlines(&loaded, "2026-11-15");
    assert!(text.contains("data directory"), "{text}");
    assert!(text.contains("API 40"), "{text}");

    let broken = root.join("broken");
    fs::create_dir_all(&broken).unwrap();
    fs::write(broken.join("platform-deadlines.json"), "{").unwrap();
    let loaded = load_deadlines(&broken).unwrap();
    assert_eq!(loaded.origin, CatalogOrigin::Builtin);
    assert!(loaded.warning.unwrap().contains("invalid"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn each_gradle_spelling_and_comments_and_the_catalog() {
    let root = scratch("spellings");
    for (name, text, expected) in [
        ("eq.gradle", "targetSdk = 35\n", 35),
        ("space.gradle", "targetSdk 35\n", 35),
        ("version.gradle", "targetSdkVersion 35\n", 35),
        ("version-eq.gradle", "targetSdkVersion = 35\n", 35),
    ] {
        write(&root, name, text);
        let found = detect_target_sdk("demo", &root, &android(&[name]));
        assert_eq!(found.value, Some(expected), "{name}");
        assert_eq!(found.source.as_deref(), Some(format!("{name}:1").as_str()));
    }
    write(
        &root,
        "commented.gradle",
        "// targetSdk = 99\n/* targetSdk = 98 */\ntargetSdk = 35\n",
    );
    let commented = detect_target_sdk("demo", &root, &android(&["commented.gradle"]));
    assert_eq!(commented.value, Some(35));
    assert!(commented.warnings.is_empty(), "{:?}", commented.warnings);

    write(
        &root,
        "gradle/libs.versions.toml",
        "[versions]\n# targetSdk = 1\ntargetSdk = \"34\" # phone\n[libraries]\n",
    );
    write(
        &root,
        "app/build.gradle.kts",
        "targetSdk = libs.versions.targetSdk.get().toInt()\n",
    );
    let catalog = detect_target_sdk(
        "demo",
        &root,
        &android(&["app/build.gradle.kts", "gradle/libs.versions.toml"]),
    );
    assert_eq!(catalog.value, Some(34), "{:?}", catalog.warnings);
    assert_eq!(catalog.source.as_deref(), Some("app/build.gradle.kts:1"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn disagreements_unresolved_values_and_a_configured_level() {
    let root = scratch("values");
    write(&root, "low.gradle", "targetSdk = 33\n");
    write(&root, "high.gradle", "targetSdk = 35\n");
    let mixed = detect_target_sdk("demo", &root, &android(&["low.gradle", "high.gradle"]));
    assert_eq!(mixed.value, Some(33));
    assert!(
        mixed.warnings[0].contains("high.gradle:1 is 35"),
        "{:?}",
        mixed.warnings
    );

    write(
        &root,
        "flutter.gradle",
        "targetSdk = flutter.targetSdkVersion\n",
    );
    let missing = detect_target_sdk("demo", &root, &android(&["flutter.gradle"]));
    assert_eq!(missing.value, None);
    assert!(
        missing.warnings.iter().any(|warning| {
            warning == "could not determine targetSdk for demo; set stack.android.target_sdk"
        }),
        "{:?}",
        missing.warnings
    );

    let mut configured = android(&["low.gradle"]);
    configured.target_sdk = Some(40);
    let chosen = detect_target_sdk("demo", &root, &configured);
    assert_eq!(chosen.value, Some(40));
    assert_eq!(chosen.source.as_deref(), Some("configured"));
    assert_eq!(chosen.warnings.len(), 0);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn strings_and_ignored_blocks_do_not_count_as_target_sdk() {
    let root = scratch("safe-sdk");
    write(
        &root,
        "string.gradle",
        "val note = \"targetSdk = 99\"\ntargetSdk = 34\n",
    );
    let from_string = detect_target_sdk("demo", &root, &android(&["string.gradle"]));
    assert_eq!(from_string.value, Some(34));
    assert_eq!(from_string.source.as_deref(), Some("string.gradle:2"));
    assert!(
        from_string.warnings.is_empty(),
        "{:?}",
        from_string.warnings
    );

    write(
        &root,
        "test.gradle",
        "targetSdk = 33\ntestOptions {\n  targetSdk = 35\n}\n",
    );
    let from_test = detect_target_sdk("demo", &root, &android(&["test.gradle"]));
    assert_eq!(from_test.value, Some(33));
    assert!(from_test.warnings.is_empty(), "{:?}", from_test.warnings);

    write(
        &root,
        "lint.gradle",
        "targetSdk = 32\nlint {\n  targetSdk = 35\n}\n",
    );
    let from_lint = detect_target_sdk("demo", &root, &android(&["lint.gradle"]));
    assert_eq!(from_lint.value, Some(32));
    assert!(from_lint.warnings.is_empty(), "{:?}", from_lint.warnings);

    write(
        &root,
        "devices.gradle",
        "targetSdk = 31\nmanagedDevices {\n  targetSdk = 35\n}\n",
    );
    let from_devices = detect_target_sdk("demo", &root, &android(&["devices.gradle"]));
    assert_eq!(from_devices.value, Some(31));
    assert!(
        from_devices.warnings.is_empty(),
        "{:?}",
        from_devices.warnings
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn target_sdk_30_reports_only_the_current_row_of_each_requirement() {
    let root = scratch("current");
    let dir = root.join("phone");
    fs::create_dir_all(dir.join("app")).unwrap();
    fs::create_dir_all(dir.join("data")).unwrap();
    fs::write(dir.join("app/build.gradle.kts"), "targetSdk = 30\n").unwrap();
    let project = dir.join("craftward.json");
    fs::write(
        &project,
        "{\n  \"schema_version\": 1,\n  \"id\": \"phone\",\n  \"name\": \"Phone\",\n  \"status\": \"idea\",\n  \"stack\": {\n    \"components\": [],\n    \"lockfiles\": [],\n    \"android\": { \"detect_from\": [\"app/build.gradle.kts\"] }\n  }\n}\n",
    )
    .unwrap();
    let report = scan(
        &[project],
        &Quiet,
        &dir.join("cache"),
        &dir.join("data"),
        &options(TODAY, 180),
    )
    .remove(0);
    assert_eq!(report.findings.len(), 2, "{:?}", report.findings);
    let updates = report
        .findings
        .iter()
        .find(|finding| finding.id == "PLAY:phone:new_apps_and_updates")
        .expect("new apps");
    assert_eq!(updates.kind, FindingKind::PlatformDeadline);
    assert!(updates.summary.contains("required 36"));
    assert!(updates.summary.contains("deadline 2026-08-31"));
    let existing = report
        .findings
        .iter()
        .find(|finding| finding.id == "PLAY:phone:existing_apps")
        .expect("existing apps");
    assert!(existing.summary.contains("required 35"));
    assert!(existing.summary.contains("deadline 2026-08-31"));
    assert!(
        report
            .findings
            .iter()
            .all(|finding| !finding.summary.contains("2025-08-31"))
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn deadlines_follow_the_day_the_factor_and_the_store() {
    assert_eq!(utc_day(TODAY).as_deref(), Some("2026-11-15"));
    let root = scratch("rules");
    let past = scan_sdk(
        &root,
        "past",
        30,
        "phone",
        "google_play",
        sample("2026-11-16", 36, "2026-01-01"),
        180,
    );
    assert_eq!(past.findings.len(), 1, "{:?}", past.findings);
    assert_eq!(past.findings[0].kind, FindingKind::PlatformDeadline);
    assert_eq!(past.findings[0].severity, Severity::High);
    assert_eq!(past.findings[0].ecosystem, "platform");
    assert_eq!(past.findings[0].package, "google-play");
    assert_eq!(past.findings[0].id, "PLAY:phone:new_apps_and_updates");
    assert!(past.findings[0].summary.contains("below required 36"));
    assert!(
        past.findings[0]
            .summary
            .contains("detected at app/build.gradle.kts:1")
    );
    assert!(past.findings[0].url.starts_with("https://"));
    assert_eq!(
        past.target_sdk_note.as_deref(),
        Some("targetSdk 30 (app/build.gradle.kts:1)")
    );

    let soon = scan_sdk(
        &root,
        "soon",
        30,
        "phone",
        "google_play",
        sample("2026-11-16", 36, "2027-02-01"),
        180,
    );
    assert_eq!(soon.findings[0].kind, FindingKind::PlatformDeadlineSoon);
    assert_eq!(soon.findings[0].severity, Severity::Medium);
    assert_eq!(soon.findings[0].id, "PLAY:phone:new_apps_and_updates");

    let far = scan_sdk(
        &root,
        "far",
        30,
        "phone",
        "google_play",
        sample("2026-11-16", 36, "2027-08-01"),
        180,
    );
    assert!(far.findings.is_empty(), "{:?}", far.findings);

    let enough = scan_sdk(
        &root,
        "enough",
        36,
        "phone",
        "google_play",
        sample("2026-11-16", 36, "2026-01-01"),
        180,
    );
    assert!(enough.findings.is_empty(), "{:?}", enough.findings);
    assert!(enough.deadlines_checked);

    let wear = scan_sdk(&root, "wear", 30, "wear", "google_play", wear_sample(), 180);
    assert!(
        wear.findings
            .iter()
            .all(|finding| finding.id.contains(":wear:"))
    );
    assert_eq!(wear.findings.len(), 1);

    let skipped = scan_sdk(
        &root,
        "none",
        1,
        "phone",
        "none",
        sample("2026-11-16", 99, "2020-01-01"),
        180,
    );
    assert_eq!(skipped.findings.len(), 0);
    assert!(!skipped.deadlines_checked);
    assert!(skipped.warnings.is_empty(), "{:?}", skipped.warnings);

    let absent = scan_without_android(&root);
    assert_eq!(absent.findings.len(), 0);
    assert!(absent.warnings.is_empty(), "{:?}", absent.warnings);
    let _ = fs::remove_dir_all(root);
}

fn sample(data_version: &str, api: u32, deadline: &str) -> String {
    format!(
        r#"{{
  "schema_version": 1,
  "data_version": "{data_version}",
  "deadlines": [{{
    "platform": "google_play",
    "form_factor": "phone",
    "requirement": "new_apps_and_updates",
    "api_level": {api},
    "deadline": "{deadline}",
    "source": "https://example.com/play",
    "checked_on": "2026-11-15",
    "note": "sample"
  }}]
}}
"#
    )
}

fn wear_sample() -> String {
    r#"{
  "schema_version": 1,
  "data_version": "2026-11-16",
  "deadlines": [
    {
      "platform": "google_play", "form_factor": "phone",
      "requirement": "existing_apps", "api_level": 99,
      "deadline": "2020-01-01", "source": "https://example.com/phone",
      "checked_on": "2026-11-15", "note": "phone"
    },
    {
      "platform": "google_play", "form_factor": "wear",
      "requirement": "existing_apps", "api_level": 34,
      "deadline": "2020-01-01", "source": "https://example.com/wear",
      "checked_on": "2026-11-15", "note": "wear"
    }
  ]
}"#
    .to_owned()
}

fn scan_sdk(
    root: &Path,
    name: &str,
    sdk: u32,
    factor: &str,
    store: &str,
    deadlines: String,
    warning_days: u32,
) -> craftward_core::ProjectReport {
    let dir = root.join(name);
    fs::create_dir_all(dir.join("app")).unwrap();
    fs::create_dir_all(dir.join("data")).unwrap();
    fs::write(
        dir.join("app/build.gradle.kts"),
        format!("targetSdk = {sdk}\n"),
    )
    .unwrap();
    fs::write(dir.join("data/platform-deadlines.json"), deadlines).unwrap();
    let project = dir.join("craftward.json");
    fs::write(
        &project,
        format!(
            r#"{{
  "schema_version": 1,
  "id": "{name}",
  "name": "Demo",
  "status": "idea",
  "stack": {{
    "components": [],
    "lockfiles": [],
    "android": {{
      "form_factor": "{factor}",
      "store": "{store}",
      "detect_from": ["app/build.gradle.kts"]
    }}
  }}
}}
"#
        ),
    )
    .unwrap();
    scan(
        &[project],
        &Quiet,
        &dir.join("cache"),
        &dir.join("data"),
        &options(TODAY, warning_days),
    )
    .remove(0)
}

fn scan_without_android(root: &Path) -> craftward_core::ProjectReport {
    let dir = root.join("plain");
    fs::create_dir_all(&dir).unwrap();
    let project = dir.join("craftward.json");
    fs::write(
        &project,
        "{\n  \"schema_version\": 1,\n  \"id\": \"plain\",\n  \"name\": \"Plain\",\n  \"status\": \"idea\"\n}\n",
    )
    .unwrap();
    scan(
        &[project],
        &Quiet,
        &dir.join("cache"),
        &dir.join("data"),
        &options(TODAY, 180),
    )
    .remove(0)
}
