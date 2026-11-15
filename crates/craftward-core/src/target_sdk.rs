//! Reads `targetSdk` from Gradle files and a Gradle version catalog.
//!
//! Comments are ignored. A version-catalog reference is resolved only from
//! the `[versions]` table of a `*.toml` file listed in `detect_from`.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::model::AndroidConfig;
use crate::paths::join_stored;

/// Where `targetSdk` came from, and any problems along the way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detection {
    /// The level that will be compared with Play deadlines.
    pub value: Option<u32>,
    /// `configured`, or `path:line` of the level that was used.
    pub source: Option<String>,
    /// Disagreements, unreadable files, and values that could not be read.
    pub warnings: Vec<String>,
}

/// Finds `targetSdk` for one project.
///
/// A configured `target_sdk` wins and the files are not read. Otherwise the
/// highest value wins. When the files disagree, `warnings` names every place.
#[must_use]
pub fn detect(project_id: &str, source_dir: &Path, config: &AndroidConfig) -> Detection {
    if let Some(value) = config.target_sdk {
        return Detection {
            value: Some(value),
            source: Some("configured".to_owned()),
            warnings: Vec::new(),
        };
    }
    let mut warnings = Vec::new();
    let mut files = Vec::new();
    for relative in &config.detect_from {
        match fs::read_to_string(join_stored(source_dir, relative)) {
            Ok(text) => files.push(SourceFile {
                relative: relative.clone(),
                text,
            }),
            Err(error) => warnings.push(format!("could not read {relative}: {error}")),
        }
    }
    let catalogs = files
        .iter()
        .filter(|file| is_toml(&file.relative))
        .map(|file| versions(&file.text))
        .collect::<Vec<_>>();
    let mut hits = Vec::new();
    let mut unresolved = Vec::new();
    for file in &files {
        if is_toml(&file.relative) {
            continue;
        }
        scan_gradle(file, &catalogs, &mut hits, &mut unresolved);
    }
    let Some(first) = hits.first() else {
        warnings.push(format!(
            "could not determine targetSdk for {project_id}; set stack.android.target_sdk"
        ));
        return Detection {
            value: None,
            source: None,
            warnings,
        };
    };
    let mut best = first;
    for hit in &hits[1..] {
        if hit.value > best.value {
            best = hit;
        }
    }
    if hits.iter().any(|hit| hit.value != best.value) {
        let others = hits
            .iter()
            .filter(|hit| hit.value != best.value)
            .map(|hit| format!("{} is {}", hit.source, hit.value))
            .collect::<Vec<_>>()
            .join(", ");
        warnings.push(format!(
            "targetSdk differs; using {} from {}; {others}",
            best.value, best.source
        ));
    }
    for place in unresolved {
        warnings.push(format!("could not resolve targetSdk at {place}"));
    }
    Detection {
        value: Some(best.value),
        source: Some(best.source.clone()),
        warnings,
    }
}

struct SourceFile {
    relative: String,
    text: String,
}

struct Hit {
    value: u32,
    source: String,
}

fn scan_gradle(
    file: &SourceFile,
    catalogs: &[BTreeMap<String, u32>],
    hits: &mut Vec<Hit>,
    unresolved: &mut Vec<String>,
) {
    let stripped = strip_gradle(&file.text);
    for (index, line) in stripped.lines().enumerate() {
        let Some(found) = assignment(line) else {
            continue;
        };
        let place = format!("{}:{}", file.relative, index + 1);
        match found {
            Assignment::Number(value) => hits.push(Hit {
                value,
                source: place,
            }),
            Assignment::Catalog(key) => match lookup(catalogs, &key) {
                Some(value) => hits.push(Hit {
                    value,
                    source: place,
                }),
                None => unresolved.push(place),
            },
            Assignment::Unresolved => unresolved.push(place),
        }
    }
}

fn lookup(catalogs: &[BTreeMap<String, u32>], key: &str) -> Option<u32> {
    catalogs
        .iter()
        .find_map(|catalog| catalog.get(key).copied())
}

enum Assignment {
    Number(u32),
    Catalog(String),
    Unresolved,
}

fn assignment(line: &str) -> Option<Assignment> {
    let end = keyword_end(line)?;
    let rest = line[end..].trim_start();
    let rest = rest.strip_prefix('=').map_or(rest, str::trim_start);
    if let Some(value) = leading_number(rest) {
        return Some(Assignment::Number(value));
    }
    if let Some(key) = catalog_key(rest) {
        return Some(Assignment::Catalog(key));
    }
    Some(Assignment::Unresolved)
}

fn keyword_end(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if line.is_char_boundary(index)
            && (word_at(line, index, "targetSdkVersion") || word_at(line, index, "targetSdk"))
        {
            let word = if line[index..].starts_with("targetSdkVersion") {
                "targetSdkVersion"
            } else {
                "targetSdk"
            };
            return Some(index + word.len());
        }
        index += 1;
    }
    None
}

fn word_at(line: &str, index: usize, word: &str) -> bool {
    let bytes = line.as_bytes();
    if index > 0 && is_ident(bytes[index - 1]) {
        return false;
    }
    let end = index + word.len();
    if end > bytes.len() || &line[index..end] != word {
        return false;
    }
    end == bytes.len() || !is_ident(bytes[end])
}

fn leading_number(text: &str) -> Option<u32> {
    let bytes = text.as_bytes();
    let quoted = matches!(bytes.first(), Some(b'"' | b'\''));
    let mut index = usize::from(quoted);
    let start = index;
    while index < bytes.len() && bytes[index].is_ascii_digit() {
        index += 1;
    }
    if index == start {
        return None;
    }
    if quoted {
        if index >= bytes.len() || (bytes[index] != b'"' && bytes[index] != b'\'') {
            return None;
        }
    } else if index < bytes.len()
        && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_' || bytes[index] == b'.')
    {
        return None;
    }
    text[start..index].parse().ok().filter(|value| *value >= 1)
}

fn catalog_key(text: &str) -> Option<String> {
    const PREFIX: &str = "libs.versions.";
    let rest = text.strip_prefix(PREFIX)?;
    let end = rest
        .find(|char: char| !char.is_ascii_alphanumeric() && char != '_' && char != '-')
        .unwrap_or(rest.len());
    if end == 0 {
        return None;
    }
    Some(rest[..end].to_owned())
}

fn versions(text: &str) -> BTreeMap<String, u32> {
    let mut map = BTreeMap::new();
    let mut in_versions = false;
    for line in text.lines() {
        let code = strip_toml_comment(line);
        let trimmed = code.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(header) = table_header(trimmed) {
            in_versions = header == "versions";
            continue;
        }
        if !in_versions {
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        let key = key.trim().trim_matches(['"', '\'']);
        if key.is_empty() || key.contains('.') {
            continue;
        }
        if let Some(number) = toml_int(value.trim()) {
            map.insert(key.to_owned(), number);
        }
    }
    map
}

fn table_header(text: &str) -> Option<&str> {
    let inner = text.strip_prefix('[')?.strip_suffix(']')?.trim();
    if inner.is_empty() || inner.contains(['[', ']']) {
        return None;
    }
    Some(inner)
}

fn toml_int(value: &str) -> Option<u32> {
    let bare = value.trim().trim_matches(['"', '\'']);
    if bare.is_empty() || !bare.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    bare.parse().ok().filter(|number| *number >= 1)
}

fn strip_toml_comment(line: &str) -> String {
    let mut out = String::new();
    let mut chars = line.chars().peekable();
    let mut quote = None;
    while let Some(char) = chars.next() {
        if let Some(open) = quote {
            out.push(char);
            if char == '\\' {
                if let Some(escaped) = chars.next() {
                    out.push(escaped);
                }
                continue;
            }
            if char == open {
                quote = None;
            }
            continue;
        }
        if char == '"' || char == '\'' {
            quote = Some(char);
            out.push(char);
            continue;
        }
        if char == '#' {
            break;
        }
        out.push(char);
    }
    out
}

fn strip_gradle(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut index = 0;
    while index < chars.len() {
        if chars[index] == '"' || chars[index] == '\'' {
            index = copy_string(&chars, index, &mut out);
            continue;
        }
        if chars[index] == '/' && chars.get(index + 1) == Some(&'/') {
            while index < chars.len() && chars[index] != '\n' {
                out.push(' ');
                index += 1;
            }
            continue;
        }
        if chars[index] == '/' && chars.get(index + 1) == Some(&'*') {
            index = skip_block(&chars, index, &mut out);
            continue;
        }
        out.push(chars[index]);
        index += 1;
    }
    out
}

fn copy_string(chars: &[char], start: usize, out: &mut String) -> usize {
    if chars[start] == '"'
        && chars.get(start + 1) == Some(&'"')
        && chars.get(start + 2) == Some(&'"')
    {
        out.push_str("\"\"\"");
        let mut index = start + 3;
        while index < chars.len() {
            if chars[index] == '"'
                && chars.get(index + 1) == Some(&'"')
                && chars.get(index + 2) == Some(&'"')
            {
                out.push_str("\"\"\"");
                return index + 3;
            }
            out.push(chars[index]);
            index += 1;
        }
        return index;
    }
    let quote = chars[start];
    out.push(quote);
    let mut index = start + 1;
    while index < chars.len() {
        out.push(chars[index]);
        if chars[index] == '\\' {
            index += 1;
            if index < chars.len() {
                out.push(chars[index]);
            }
        } else if chars[index] == quote || chars[index] == '\n' {
            return index + 1;
        }
        index += 1;
    }
    index
}

fn skip_block(chars: &[char], start: usize, out: &mut String) -> usize {
    out.push(' ');
    out.push(' ');
    let mut index = start + 2;
    while index < chars.len() {
        if chars[index] == '*' && chars.get(index + 1) == Some(&'/') {
            out.push(' ');
            out.push(' ');
            return index + 2;
        }
        if chars[index] == '\n' {
            out.push('\n');
        } else {
            out.push(' ');
        }
        index += 1;
    }
    index
}

fn is_toml(relative: &str) -> bool {
    Path::new(relative)
        .extension()
        .is_some_and(|extension| extension == "toml")
}

fn is_ident(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}
