//! Lockfile readers.

use craftward_core::{LockKind, parse_lock_text};

#[test]
fn cargo_lock_versions_3_and_4_skip_workspace_and_git_packages() {
    for version in [3, 4] {
        let text = format!(
            "\
version = {version}

[[package]]
name = \"demo\"
version = \"0.1.0\"

[[package]]
name = \"serde\"
version = \"1.0.0\"
source = \"registry+https://github.com/rust-lang/crates.io-index\"

[[package]]
name = \"other\"
version = \"2.0.0\"
source = \"git+https://example.com/other#abcdef\"
"
        );
        let parsed = parse_lock_text(LockKind::Cargo, &text).expect("parse");
        assert_eq!(parsed.kind, "cargo");
        assert_eq!(parsed.skipped, 2);
        assert_eq!(parsed.queried.len(), 1);
        assert_eq!(parsed.queried[0].ecosystem, "crates.io");
        assert_eq!(parsed.queried[0].name, "serde");
        assert_eq!(parsed.queried[0].version, "1.0.0");
    }
}

#[test]
fn cargo_lock_reports_an_unsupported_version_and_a_broken_line() {
    let unsupported = parse_lock_text(LockKind::Cargo, "version = 2\n").expect_err("version");
    assert_eq!(unsupported.line, Some(1));
    assert!(
        unsupported.message.contains("not supported"),
        "{}",
        unsupported.message
    );
    let broken = "\
version = 3

[[package]]
name = bare
version = \"1.0.0\"
";
    let failure = parse_lock_text(LockKind::Cargo, broken).expect_err("broken");
    assert_eq!(failure.line, Some(4));
    assert!(
        failure.message.contains("quoted string"),
        "{}",
        failure.message
    );
}

#[test]
fn package_lock_versions_2_and_3_skip_links_and_query_duplicates_once() {
    for version in [2, 3] {
        let text = format!(
            r#"{{
  "lockfileVersion": {version},
  "packages": {{
    "": {{ "name": "app", "version": "1.0.0" }},
    "node_modules/left": {{ "version": "1.2.3" }},
    "node_modules/left-link": {{ "version": "1.2.3", "link": true }},
    "node_modules/parent/node_modules/@scope/right": {{ "version": "4.5.6" }},
    "node_modules/alias": {{ "name": "left", "version": "1.2.3" }}
  }}
}}"#,
        );
        let parsed = parse_lock_text(LockKind::Npm, &text).expect("parse");
        assert_eq!(parsed.queried.len(), 2, "{:?}", parsed.queried);
        assert!(
            parsed
                .queried
                .iter()
                .any(|package| package.name == "left" && package.version == "1.2.3")
        );
        assert!(
            parsed
                .queried
                .iter()
                .any(|package| package.name == "@scope/right" && package.version == "4.5.6")
        );
        assert!(
            parsed
                .queried
                .iter()
                .all(|package| package.ecosystem == "npm")
        );
    }
}

#[test]
fn package_lock_version_1_is_rejected_with_a_line() {
    let text = "{\n  \"lockfileVersion\": 1,\n  \"dependencies\": {}\n}\n";
    let failure = parse_lock_text(LockKind::Npm, text).expect_err("version 1");
    assert_eq!(failure.line, Some(2));
    assert!(
        failure.message.contains("version 1 is not supported"),
        "{}",
        failure.message
    );
}
