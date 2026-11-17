//! Locale-aware calendar dates for the desktop forms.
//!
//! Storage stays ISO `YYYY-MM-DD`. Display and typed input follow the system
//! locale: German (`de*`) uses `DD.MM.YYYY`; everything else uses ISO.
//!
//! Locale resolution order: OS setting via `sys-locale`, then `LC_TIME` /
//! `LC_ALL` / `LANG`, then ISO (empty tag).

use craftward_core::is_calendar_date;

/// Provides locale tags for date formatting without talking to the OS in tests.
pub trait LocaleSource {
    /// Preferred OS / application locale (for example from `sys-locale`).
    fn system_locale(&self) -> Option<String>;

    /// Fallback from process environment variables.
    fn env_locale(&self) -> Option<String>;
}

/// Production source: `sys-locale`, then `LANG` / `LC_*`.
#[derive(Debug, Default, Clone, Copy)]
pub struct ProcessLocale;

impl LocaleSource for ProcessLocale {
    fn system_locale(&self) -> Option<String> {
        sys_locale::get_locale().and_then(normalize_locale_tag)
    }

    fn env_locale(&self) -> Option<String> {
        for key in ["LC_TIME", "LC_ALL", "LANG"] {
            if let Ok(value) = std::env::var(key)
                && let Some(tag) = normalize_locale_tag(value)
            {
                return Some(tag);
            }
        }
        None
    }
}

/// Resolves a locale tag: system, then env, then empty (ISO fallback).
#[must_use]
pub fn resolve_locale_tag(source: &impl LocaleSource) -> String {
    source
        .system_locale()
        .or_else(|| source.env_locale())
        .unwrap_or_default()
}

/// Whether `source` prefers day-month-year order (German).
#[must_use]
pub fn prefers_day_month_year_with(source: &impl LocaleSource) -> bool {
    is_german_locale(&resolve_locale_tag(source))
}

/// Whether the process locale prefers day-month-year order (German).
#[must_use]
pub fn prefers_day_month_year() -> bool {
    prefers_day_month_year_with(&ProcessLocale)
}

/// Placeholder / validation hint for due-date fields.
#[must_use]
pub fn date_hint() -> &'static str {
    if prefers_day_month_year() {
        "DD.MM.YYYY or YYYY-MM-DD"
    } else {
        "YYYY-MM-DD or DD.MM.YYYY"
    }
}

/// Formats an ISO `YYYY-MM-DD` date for display.
#[must_use]
pub fn format_date_for_display(iso: &str) -> String {
    format_date_for_display_with(iso, &ProcessLocale)
}

/// Formats an ISO date using an injected locale source.
#[must_use]
pub fn format_date_for_display_with(iso: &str, source: &impl LocaleSource) -> String {
    if !is_calendar_date(iso) {
        return iso.to_owned();
    }
    if prefers_day_month_year_with(source) {
        let (year, rest) = iso.split_once('-').unwrap_or(("", ""));
        let (month, day) = rest.split_once('-').unwrap_or(("", ""));
        format!("{day}.{month}.{year}")
    } else {
        iso.to_owned()
    }
}

/// Parses a typed date into ISO `YYYY-MM-DD`.
///
/// Accepts `YYYY-MM-DD` and `DD.MM.YYYY`. Returns [`None`] when the text is
/// empty or not a valid calendar day.
#[must_use]
pub fn parse_date_input(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    if is_calendar_date(trimmed) {
        return Some(trimmed.to_owned());
    }
    let parts: Vec<&str> = trimmed.split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    let day = parts[0];
    let month = parts[1];
    let year = parts[2];
    if day.len() > 2 || month.len() > 2 || year.len() != 4 {
        return None;
    }
    let day: u32 = day.parse().ok()?;
    let month: u32 = month.parse().ok()?;
    let year: u32 = year.parse().ok()?;
    if !(1..=31).contains(&day) || !(1..=12).contains(&month) || year < 1 {
        return None;
    }
    let iso = format!("{year:04}-{month:02}-{day:02}");
    if is_calendar_date(&iso) {
        Some(iso)
    } else {
        None
    }
}

/// Validation message when a non-empty date field is invalid.
#[must_use]
pub fn invalid_date_message() -> &'static str {
    if prefers_day_month_year() {
        "Due date must be DD.MM.YYYY or YYYY-MM-DD"
    } else {
        "Due date must be YYYY-MM-DD or DD.MM.YYYY"
    }
}

fn is_german_locale(tag: &str) -> bool {
    tag.to_ascii_lowercase()
        .split(['_', '-', '.', '@'])
        .next()
        .is_some_and(|lang| lang == "de")
}

fn normalize_locale_tag(value: impl AsRef<str>) -> Option<String> {
    let trimmed = value.as_ref().trim();
    if trimmed.is_empty() || trimmed == "C" || trimmed == "POSIX" {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        LocaleSource, format_date_for_display_with, parse_date_input, prefers_day_month_year_with,
        resolve_locale_tag,
    };

    #[derive(Debug, Default)]
    struct FixedLocale {
        system: Option<&'static str>,
        env: Option<&'static str>,
    }

    impl LocaleSource for FixedLocale {
        fn system_locale(&self) -> Option<String> {
            self.system.map(str::to_owned)
        }

        fn env_locale(&self) -> Option<String> {
            self.env.map(str::to_owned)
        }
    }

    #[test]
    fn parse_accepts_iso_and_dotted_german() {
        assert_eq!(
            parse_date_input("2027-03-31").as_deref(),
            Some("2027-03-31")
        );
        assert_eq!(
            parse_date_input("31.03.2027").as_deref(),
            Some("2027-03-31")
        );
        assert_eq!(parse_date_input("1.3.2027").as_deref(), Some("2027-03-01"));
        assert_eq!(parse_date_input(""), None);
        assert_eq!(parse_date_input("32.01.2027"), None);
        assert_eq!(parse_date_input("2027-13-01"), None);
        assert_eq!(parse_date_input("not-a-date"), None);
    }

    #[test]
    fn system_locale_wins_over_env() {
        let source = FixedLocale {
            system: Some("de-DE"),
            env: Some("en_US.UTF-8"),
        };
        assert_eq!(resolve_locale_tag(&source), "de-DE");
        assert!(prefers_day_month_year_with(&source));
        assert_eq!(
            format_date_for_display_with("2027-03-31", &source),
            "31.03.2027"
        );
    }

    #[test]
    fn env_locale_is_used_when_system_is_absent() {
        let source = FixedLocale {
            system: None,
            env: Some("de_DE.UTF-8"),
        };
        assert!(prefers_day_month_year_with(&source));
        let english = FixedLocale {
            system: None,
            env: Some("en_GB.UTF-8"),
        };
        assert!(!prefers_day_month_year_with(&english));
        assert_eq!(
            format_date_for_display_with("2027-03-31", &english),
            "2027-03-31"
        );
    }

    #[test]
    fn empty_sources_fall_back_to_iso_display() {
        let source = FixedLocale::default();
        assert_eq!(resolve_locale_tag(&source), "");
        assert!(!prefers_day_month_year_with(&source));
        assert_eq!(
            format_date_for_display_with("2027-03-31", &source),
            "2027-03-31"
        );
    }

    #[test]
    fn german_display_round_trips_through_parse() {
        let source = FixedLocale {
            system: Some("de_CH"),
            env: None,
        };
        let shown = format_date_for_display_with("2027-03-31", &source);
        assert_eq!(shown, "31.03.2027");
        assert_eq!(parse_date_input(&shown).as_deref(), Some("2027-03-31"));
    }
}
