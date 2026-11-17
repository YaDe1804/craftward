//! Project category strings and platform labels derived from the stack.

use crate::model::{ComponentKind, Project};

/// Maximum length of a stored category after normalization.
pub const CATEGORY_MAX_LEN: usize = 40;

/// Trims, collapses internal whitespace to single spaces, and validates.
///
/// Returns [`None`] when the result would be empty, longer than
/// [`CATEGORY_MAX_LEN`], or contain a control character.
#[must_use]
pub fn normalize_category(input: &str) -> Option<String> {
    let mut out = String::new();
    let mut pending_space = false;
    for ch in input.chars() {
        if ch.is_control() {
            return None;
        }
        if ch.is_whitespace() {
            if !out.is_empty() {
                pending_space = true;
            }
            continue;
        }
        if pending_space {
            out.push(' ');
            pending_space = false;
        }
        out.push(ch);
    }
    if out.is_empty() || out.chars().count() > CATEGORY_MAX_LEN {
        return None;
    }
    Some(out)
}

/// Normalizes `input`, then matches it case-insensitively against `existing`.
///
/// When a match is found, returns that existing spelling and `true` (caller
/// may show a short hint). Otherwise returns the freshly normalized string
/// and `false`.
///
/// # Errors
///
/// Returns an English validation message when normalization fails.
pub fn normalize_category_against(
    existing: &[String],
    input: &str,
) -> Result<(String, bool), String> {
    let normalized = normalize_category(input).ok_or_else(|| {
        format!(
            "category must be 1–{CATEGORY_MAX_LEN} characters after trimming, with no control characters"
        )
    })?;
    if let Some(match_existing) = existing
        .iter()
        .find(|known| eq_ignore_ascii_case(known, &normalized))
    {
        let unified = !match_existing.as_str().eq(&normalized);
        return Ok((match_existing.clone(), unified));
    }
    Ok((normalized, false))
}

/// Readable platform labels for one project, case-insensitively unique.
///
/// Sources: `stack.android` (always contributes `"Android"`) and components
/// whose kind is `platform`. Duplicate names that differ only by letter case
/// collapse to one entry (preferring a known display label).
#[must_use]
pub fn project_platforms(project: &Project) -> Vec<String> {
    let mut labels: Vec<(String, String)> = Vec::new();
    if project
        .stack
        .as_ref()
        .and_then(|stack| stack.android.as_ref())
        .is_some()
    {
        push_platform(&mut labels, "Android");
    }
    if let Some(stack) = &project.stack {
        for component in &stack.components {
            if component.kind == ComponentKind::Platform {
                push_platform(&mut labels, &component.name);
            }
        }
    }
    let mut result: Vec<String> = labels.into_iter().map(|(_, display)| display).collect();
    result.sort_by_key(|left| left.to_ascii_lowercase());
    result
}

fn push_platform(labels: &mut Vec<(String, String)>, raw: &str) {
    let Some(display) = display_platform_name(raw) else {
        return;
    };
    let key = display.to_ascii_lowercase();
    if labels.iter().any(|(existing, _)| existing == &key) {
        return;
    }
    labels.push((key, display));
}

/// Maps a raw platform name to a readable label.
#[must_use]
pub fn display_platform_name(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let lower = trimmed.to_ascii_lowercase();
    let known = match lower.as_str() {
        "android" => "Android",
        "windows" | "win32" | "win" => "Windows",
        "linux" => "Linux",
        "macos" | "mac os" | "mac os x" | "osx" | "darwin" => "macOS",
        "ios" => "iOS",
        "web" | "browser" => "Web",
        "tvos" => "tvOS",
        "watchos" => "watchOS",
        _ => "",
    };
    if known.is_empty() {
        Some(title_case_words(trimmed))
    } else {
        Some(known.to_owned())
    }
}

fn title_case_words(text: &str) -> String {
    text.split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => {
                    let mut out = first.to_uppercase().collect::<String>();
                    out.extend(chars.flat_map(char::to_lowercase));
                    out
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn eq_ignore_ascii_case(left: &str, right: &str) -> bool {
    left.eq_ignore_ascii_case(right)
}

#[cfg(test)]
mod tests {
    use super::{
        display_platform_name, normalize_category, normalize_category_against, project_platforms,
    };
    use crate::model::{
        AndroidConfig, AndroidStore, Component, ComponentKind, FormFactor, Project, ProjectStatus,
        Stack,
    };

    #[test]
    fn normalize_collapses_spaces_and_rejects_controls() {
        assert_eq!(
            normalize_category("  Security   Tools  ").as_deref(),
            Some("Security Tools")
        );
        assert!(normalize_category("").is_none());
        assert!(normalize_category("   ").is_none());
        assert!(normalize_category("bad\nname").is_none());
        assert!(normalize_category(&"x".repeat(41)).is_none());
    }

    #[test]
    fn against_unifies_letter_case() {
        let existing = vec!["Security Tools".to_owned()];
        let (value, unified) =
            normalize_category_against(&existing, "security  tools").expect("ok");
        assert_eq!(value, "Security Tools");
        assert!(unified);
    }

    #[test]
    fn platforms_merge_android_block_and_component() {
        let project = Project {
            schema_version: 1,
            id: "demo".to_owned(),
            name: "Demo".to_owned(),
            status: ProjectStatus::Idea,
            source_dir: None,
            description: None,
            current_version: None,
            category: None,
            tags: None,
            updated: None,
            links: None,
            stack: Some(Stack {
                components: vec![Component {
                    kind: ComponentKind::Platform,
                    name: "android".to_owned(),
                    version: None,
                    note: None,
                    eol_product: None,
                }],
                lockfiles: Vec::new(),
                android: Some(AndroidConfig {
                    target_sdk: None,
                    min_sdk: None,
                    form_factor: FormFactor::Phone,
                    store: AndroidStore::GooglePlay,
                    detect_from: Vec::new(),
                }),
            }),
            plan: None,
            issues: None,
            watch_items: None,
        };
        assert_eq!(project_platforms(&project), vec!["Android".to_owned()]);
        assert_eq!(display_platform_name("macos").as_deref(), Some("macOS"));
    }
}
