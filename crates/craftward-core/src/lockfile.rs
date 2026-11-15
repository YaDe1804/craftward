//! Readers for the lockfiles named by a project.
//!
//! Cargo.lock is parsed here. There is no TOML library.

use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::path::Path;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// One package that can be sent to OSV.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackageQuery {
    /// OSV ecosystem name, such as `crates.io` or `npm`.
    pub ecosystem: String,
    /// Package name as recorded by that ecosystem.
    pub name: String,
    /// Exact version.
    pub version: String,
}

/// Packages taken from one lockfile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedLock {
    /// `cargo` or `npm`.
    pub kind: LockKind,
    /// Packages to query. Each name and version appears once.
    pub queried: Vec<PackageQuery>,
    /// Packages that were seen and not queried.
    pub skipped: usize,
}

/// Why a lockfile could not be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockFailure {
    /// 1-based line, when the failure belongs to one line.
    pub line: Option<u32>,
    /// English explanation.
    pub message: String,
}

impl std::fmt::Display for LockFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for LockFailure {}

/// Which reader a file name selects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LockKind {
    /// `Cargo.lock`.
    Cargo,
    /// `package-lock.json`.
    Npm,
    /// A name this radar does not read.
    Unknown,
}

impl LockKind {
    /// Stable stored word: `cargo`, `npm`, or `unknown`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cargo => "cargo",
            Self::Npm => "npm",
            Self::Unknown => "unknown",
        }
    }

    /// Every value.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[Self::Cargo, Self::Npm, Self::Unknown]
    }
}

impl FromStr for LockKind {
    type Err = UnknownLockKind;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text {
            "cargo" => Ok(Self::Cargo),
            "npm" => Ok(Self::Npm),
            "unknown" => Ok(Self::Unknown),
            _ => Err(UnknownLockKind {
                value: text.to_owned(),
            }),
        }
    }
}

impl fmt::Display for LockKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Text that is not `cargo`, `npm`, or `unknown`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownLockKind {
    value: String,
}

impl fmt::Display for UnknownLockKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "unknown lockfile kind: {}", self.value)
    }
}

impl std::error::Error for UnknownLockKind {}

/// Selects a reader from the file name, ignoring the directory.
#[must_use]
pub fn lock_kind(path: &Path) -> LockKind {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return LockKind::Unknown;
    };
    if name.eq_ignore_ascii_case("Cargo.lock") {
        LockKind::Cargo
    } else if name.eq_ignore_ascii_case("package-lock.json") {
        LockKind::Npm
    } else {
        LockKind::Unknown
    }
}

/// Reads and parses one lockfile.
///
/// # Errors
///
/// Returns [`LockFailure`] when the file is missing, not UTF-8, or does not
/// match a supported shape. Unknown file names are not an error; the caller
/// warns and continues.
pub fn read_lockfile(path: &Path) -> Result<ParsedLock, LockFailure> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(LockFailure {
                line: None,
                message: format!("lockfile not found: {}", path.display()),
            });
        }
        Err(error) => {
            return Err(LockFailure {
                line: None,
                message: format!("failed to read {}: {error}", path.display()),
            });
        }
    };
    parse_lock_text(lock_kind(path), &text)
}

/// Parses lockfile text that has already been read.
///
/// # Errors
///
/// Returns [`LockFailure`] for an unsupported version or a broken file.
/// [`LockKind::Unknown`] is reported as a failure so callers can tell it apart
/// from a parsed file; [`read_lockfile`] is not used for that case.
pub fn parse_lock_text(kind: LockKind, text: &str) -> Result<ParsedLock, LockFailure> {
    match kind {
        LockKind::Cargo => parse_cargo(text),
        LockKind::Npm => parse_npm(text),
        LockKind::Unknown => Err(LockFailure {
            line: None,
            message: "unknown lockfile type".to_owned(),
        }),
    }
}

fn parse_cargo(text: &str) -> Result<ParsedLock, LockFailure> {
    let mut format_version = None;
    let mut packages = Vec::new();
    let mut current: Option<CargoPackage> = None;
    let mut skipping = false;
    let mut in_array = false;
    for (index, line) in text.lines().enumerate() {
        let line_no = u32::try_from(index).unwrap_or(u32::MAX) + 1;
        let trimmed = line.trim();
        if in_array {
            if trimmed.ends_with(']') {
                in_array = false;
            }
            continue;
        }
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed == "[[package]]" {
            finish_package(&mut packages, current.take(), line_no)?;
            skipping = false;
            current = Some(CargoPackage {
                line: line_no,
                name: None,
                version: None,
                source: None,
            });
            continue;
        }
        if trimmed.starts_with('[') {
            finish_package(&mut packages, current.take(), line_no)?;
            skipping = true;
            continue;
        }
        if skipping {
            continue;
        }
        let Some((key, value)) = split_entry(trimmed) else {
            return Err(invalid(line_no, "expected name = value"));
        };
        if value.starts_with('[') && !value.ends_with(']') {
            in_array = true;
            continue;
        }
        if current.is_none() && key == "version" && !value.starts_with('"') {
            format_version = Some((line_no, parse_format_version(line_no, value)?));
            continue;
        }
        let package = current.as_mut().ok_or_else(|| {
            invalid(
                line_no,
                "a field appeared before the format version or a package",
            )
        })?;
        match key {
            "name" => package.name = Some(parse_string(line_no, value)?),
            "version" => package.version = Some(parse_string(line_no, value)?),
            "source" => package.source = Some(parse_string(line_no, value)?),
            _ => {}
        }
    }
    finish_package(&mut packages, current.take(), 0)?;
    let Some((_, version)) = format_version else {
        return Err(invalid(1, "Cargo.lock has no format version"));
    };
    if version != 3 && version != 4 {
        let line = format_version.map_or(1, |(line, _)| line);
        return Err(invalid(
            line,
            &format!(
                "Cargo.lock version {version} is not supported; supported versions are 3 and 4"
            ),
        ));
    }
    Ok(collect_cargo(packages))
}

struct CargoPackage {
    line: u32,
    name: Option<String>,
    version: Option<String>,
    source: Option<String>,
}

fn finish_package(
    packages: &mut Vec<CargoPackage>,
    package: Option<CargoPackage>,
    _next_line: u32,
) -> Result<(), LockFailure> {
    let Some(package) = package else {
        return Ok(());
    };
    if package.name.is_none() || package.version.is_none() {
        return Err(invalid(
            package.line,
            "a Cargo package needs a name and a version",
        ));
    }
    packages.push(package);
    Ok(())
}

fn collect_cargo(packages: Vec<CargoPackage>) -> ParsedLock {
    let mut queried = Vec::new();
    let mut seen = BTreeSet::new();
    let mut skipped = 0_usize;
    for package in packages {
        let name = package.name.unwrap_or_default();
        let version = package.version.unwrap_or_default();
        let Some(source) = package.source else {
            skipped += 1;
            continue;
        };
        if !is_crates_io(&source) {
            skipped += 1;
            continue;
        }
        let query = PackageQuery {
            ecosystem: "crates.io".to_owned(),
            name,
            version,
        };
        if seen.insert(query.clone()) {
            queried.push(query);
        }
    }
    ParsedLock {
        kind: LockKind::Cargo,
        queried,
        skipped,
    }
}

fn is_crates_io(source: &str) -> bool {
    source.contains("github.com/rust-lang/crates.io-index") || source.contains("index.crates.io")
}

fn parse_format_version(line: u32, value: &str) -> Result<u32, LockFailure> {
    value
        .parse()
        .map_err(|_| invalid(line, "the Cargo.lock format version is not an integer"))
}

fn split_entry(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.split_once('=')?;
    Some((key.trim(), value.trim()))
}

fn parse_string(line: u32, value: &str) -> Result<String, LockFailure> {
    let bytes = value.as_bytes();
    if bytes.len() < 2 || bytes[0] != b'"' || bytes[bytes.len() - 1] != b'"' {
        return Err(invalid(line, "expected a quoted string"));
    }
    let mut out = String::new();
    let mut escaped = false;
    for byte in &bytes[1..bytes.len() - 1] {
        if escaped {
            match byte {
                b'\\' => out.push('\\'),
                b'"' => out.push('"'),
                _ => return Err(invalid(line, "unsupported escape in a string")),
            }
            escaped = false;
        } else if *byte == b'\\' {
            escaped = true;
        } else {
            out.push(char::from(*byte));
        }
    }
    if escaped {
        return Err(invalid(line, "a string ended with an unfinished escape"));
    }
    Ok(out)
}

fn parse_npm(text: &str) -> Result<ParsedLock, LockFailure> {
    let value: serde_json::Value = serde_json::from_str(text).map_err(|error| {
        let line = u32::try_from(error.line()).ok();
        LockFailure {
            line,
            message: format!("invalid JSON: {error}"),
        }
    })?;
    let Some(object) = value.as_object() else {
        return Err(invalid(1, "package-lock.json must be an object"));
    };
    let version = object
        .get("lockfileVersion")
        .ok_or_else(|| invalid(1, "package-lock.json has no lockfileVersion"))?;
    let Some(version) = version.as_u64() else {
        return Err(invalid(
            1,
            "package-lock.json lockfileVersion must be a number",
        ));
    };
    if version == 1 {
        return Err(invalid(
            line_of(text, "\"lockfileVersion\"").unwrap_or(1),
            "package-lock.json version 1 is not supported",
        ));
    }
    if version != 2 && version != 3 {
        return Err(invalid(
            line_of(text, "\"lockfileVersion\"").unwrap_or(1),
            &format!(
                "package-lock.json version {version} is not supported; supported versions are 2 and 3"
            ),
        ));
    }
    let Some(packages) = object
        .get("packages")
        .and_then(serde_json::Value::as_object)
    else {
        return Err(invalid(1, "package-lock.json has no packages object"));
    };
    let mut queried = Vec::new();
    let mut seen = BTreeSet::new();
    for (key, value) in packages {
        if key.is_empty() {
            continue;
        }
        let Some(entry) = value.as_object() else {
            return Err(invalid(
                line_of(text, &format!("\"{key}\"")).unwrap_or(1),
                "a package entry must be an object",
            ));
        };
        if entry.get("link").and_then(serde_json::Value::as_bool) == Some(true) {
            continue;
        }
        let name = entry
            .get("name")
            .and_then(serde_json::Value::as_str)
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
            .or_else(|| npm_name(key));
        let Some(name) = name else {
            return Err(invalid(
                line_of(text, &format!("\"{key}\"")).unwrap_or(1),
                "a package entry has no name",
            ));
        };
        let Some(version) = entry.get("version").and_then(serde_json::Value::as_str) else {
            return Err(invalid(
                line_of(text, &format!("\"{key}\"")).unwrap_or(1),
                "a package entry has no version",
            ));
        };
        let query = PackageQuery {
            ecosystem: "npm".to_owned(),
            name,
            version: version.to_owned(),
        };
        if seen.insert(query.clone()) {
            queried.push(query);
        }
    }
    Ok(ParsedLock {
        kind: LockKind::Npm,
        queried,
        skipped: 0,
    })
}

fn npm_name(key: &str) -> Option<String> {
    let parts: Vec<&str> = key.split('/').filter(|part| !part.is_empty()).collect();
    let index = parts.iter().rposition(|part| *part == "node_modules")?;
    let rest = &parts[index + 1..];
    let (scope, name) = match rest {
        [scope, name, ..] if scope.starts_with('@') => (*scope, *name),
        [name, ..] => return Some((*name).to_owned()),
        [] => return None,
    };
    Some(format!("{scope}/{name}"))
}

fn line_of(text: &str, needle: &str) -> Option<u32> {
    text.lines()
        .position(|line| line.contains(needle))
        .and_then(|index| u32::try_from(index).ok())
        .map(|index| index + 1)
}

fn invalid(line: u32, message: &str) -> LockFailure {
    LockFailure {
        line: Some(line),
        message: message.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::LockKind;

    #[test]
    fn lock_kinds_round_trip() {
        for kind in LockKind::all() {
            let word = serde_json::to_value(kind).unwrap();
            assert_eq!(word.as_str(), Some(kind.as_str()));
            assert_eq!(LockKind::from_str(kind.as_str()).unwrap(), *kind);
            assert_eq!(serde_json::from_value::<LockKind>(word).unwrap(), *kind);
        }
        assert!(LockKind::from_str("yarn").is_err());
        assert!(serde_json::from_str::<LockKind>("\"yarn\"").is_err());
    }
}
