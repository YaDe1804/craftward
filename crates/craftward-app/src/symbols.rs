//! UI symbols rendered with egui's bundled fonts.
//!
//! With `monochrome_emoji_fonts` enabled on egui, proportional text includes
//! Noto Emoji and emoji-icon-font (both OFL-1.1, covered by the
//! `epaint_default_fonts` licence exception in `deny.toml`). Status glyphs
//! therefore render; every badge still shows a readable English label beside
//! the symbol.
//!
//! Each constant is one meaning. Non-empty characters are unique across
//! meanings except U+26A0, which is shared by high and critical severity.

use eframe::egui::FontDefinitions;

/// Open / idea / not started (U+25CB).
pub const OPEN: &str = "\u{25CB}";

/// In progress / in development.
///
/// Spec asked for U+25D0 (`◐`), which is absent from Ubuntu-Light and the
/// monochrome emoji fonts. U+25D1 (`◑`) is the closest half-circle that
/// `Fonts::has_glyph` finds in the proportional fallback chain.
pub const RUNNING: &str = "\u{25D1}";

/// Done / resolved (U+2714).
pub const DONE: &str = "\u{2714}";

/// Released project — no clear glyph; label only.
pub const RELEASED: &str = "";

/// Maintenance project — no clear glyph; label only.
pub const MAINTENANCE: &str = "";

/// Paused project — no clear glyph; label only.
pub const PAUSED: &str = "";

/// Discontinued project — no clear glyph; label only.
pub const CLOSED: &str = "";

/// Dropped task / won't-fix issue (U+2716).
pub const DROPPED: &str = "\u{2716}";

/// Low severity — text and colour only.
pub const SEVERITY_LOW: &str = "";

/// Medium severity — text and colour only.
pub const SEVERITY_MEDIUM: &str = "";

/// High severity warning (U+26A0).
pub const SEVERITY_HIGH: &str = "\u{26A0}";

/// Critical severity warning (U+26A0; same glyph as high by design).
pub const SEVERITY_CRITICAL: &str = "\u{26A0}";

/// Unknown severity — text and colour only.
pub const UNKNOWN: &str = "";

/// Vulnerability finding kind — text only.
pub const KIND_VULNERABILITY: &str = "";

/// Malicious finding kind — text only.
pub const KIND_MALICIOUS: &str = "";

/// Unmaintained finding kind — text only.
pub const KIND_UNMAINTAINED: &str = "";

/// Unsound finding kind — text only.
pub const KIND_UNSOUND: &str = "";

/// Notice finding kind — text only.
pub const KIND_NOTICE: &str = "";

/// End-of-life finding kind — text only.
pub const KIND_EOL: &str = "";

/// EOL-soon finding kind — text only.
pub const KIND_EOL_SOON: &str = "";

/// Platform-deadline finding kind — text only.
pub const KIND_PLATFORM_DEADLINE: &str = "";

/// Platform-deadline-soon finding kind — text only.
pub const KIND_PLATFORM_DEADLINE_SOON: &str = "";

/// Task row in the deadlines list — text only.
pub const TASK: &str = "";

/// Watch deadline — text only.
pub const WATCH_DEADLINE: &str = "";

/// Watch expiry — text only.
pub const WATCH_EXPIRY: &str = "";

/// Watch review — text only.
pub const WATCH_REVIEW: &str = "";

/// Watch other — text only.
pub const WATCH_OTHER: &str = "";

/// Platform stack kind — text only.
pub const PLATFORM: &str = "";

/// Language stack kind — text only.
pub const LANGUAGE: &str = "";

/// Framework stack kind — text only.
pub const FRAMEWORK: &str = "";

/// Runtime stack kind — text only.
pub const RUNTIME: &str = "";

/// SDK stack kind — text only.
pub const SDK: &str = "";

/// Service stack kind — text only.
pub const SERVICE: &str = "";

/// Tool stack kind — text only.
pub const TOOL: &str = "";

/// Other stack kind — text only.
pub const OTHER: &str = "";

/// Every meaning and its symbol. The uniqueness test walks this table.
#[cfg_attr(not(test), allow(dead_code))]
pub const MEANINGS: &[(&str, &str)] = &[
    ("open", OPEN),
    ("running", RUNNING),
    ("done", DONE),
    ("released", RELEASED),
    ("maintenance", MAINTENANCE),
    ("paused", PAUSED),
    ("closed", CLOSED),
    ("dropped", DROPPED),
    ("severity_low", SEVERITY_LOW),
    ("severity_medium", SEVERITY_MEDIUM),
    ("severity_high", SEVERITY_HIGH),
    ("severity_critical", SEVERITY_CRITICAL),
    ("unknown", UNKNOWN),
    ("kind_vulnerability", KIND_VULNERABILITY),
    ("kind_malicious", KIND_MALICIOUS),
    ("kind_unmaintained", KIND_UNMAINTAINED),
    ("kind_unsound", KIND_UNSOUND),
    ("kind_notice", KIND_NOTICE),
    ("kind_eol", KIND_EOL),
    ("kind_eol_soon", KIND_EOL_SOON),
    ("kind_platform_deadline", KIND_PLATFORM_DEADLINE),
    ("kind_platform_deadline_soon", KIND_PLATFORM_DEADLINE_SOON),
    ("task", TASK),
    ("watch_deadline", WATCH_DEADLINE),
    ("watch_expiry", WATCH_EXPIRY),
    ("watch_review", WATCH_REVIEW),
    ("watch_other", WATCH_OTHER),
    ("platform", PLATFORM),
    ("language", LANGUAGE),
    ("framework", FRAMEWORK),
    ("runtime", RUNTIME),
    ("sdk", SDK),
    ("service", SERVICE),
    ("tool", TOOL),
    ("other", OTHER),
];

/// Every non-empty symbol string drawn by the desktop UI.
#[cfg_attr(not(test), allow(dead_code))]
pub const ALL: &[&str] = &[
    OPEN,
    RUNNING,
    DONE,
    DROPPED,
    SEVERITY_HIGH,
    SEVERITY_CRITICAL,
];

/// Font names installed for the proportional family under our egui features.
#[must_use]
#[cfg_attr(not(test), allow(dead_code))]
pub fn proportional_font_names() -> Vec<String> {
    FontDefinitions::default()
        .families
        .get(&eframe::egui::FontFamily::Proportional)
        .cloned()
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use eframe::egui::{FontDefinitions, FontFamily, FontId};
    use eframe::epaint::text::{Fonts, TextOptions};

    use super::{ALL, MEANINGS, proportional_font_names};

    fn warning_pair(left: &str, right: &str) -> bool {
        matches!(
            (left, right),
            ("severity_high", "severity_critical") | ("severity_critical", "severity_high")
        )
    }

    #[test]
    fn default_fonts_include_monochrome_emoji() {
        let definitions = FontDefinitions::default();
        let mut names: Vec<&str> = definitions.font_data.keys().map(String::as_str).collect();
        names.sort_unstable();
        assert_eq!(
            names,
            [
                "Hack",
                "NotoEmoji-Regular",
                "Ubuntu-Light",
                "egui-icons",
                "emoji-icon-font"
            ]
        );
        assert!(
            definitions.font_data.contains_key("NotoEmoji-Regular"),
            "NotoEmoji comes with egui's monochrome_emoji_fonts feature"
        );
        assert!(
            definitions.font_data.contains_key("emoji-icon-font"),
            "emoji-icon-font comes with egui's monochrome_emoji_fonts feature"
        );
        let proportional = proportional_font_names();
        assert!(
            proportional.contains(&"NotoEmoji-Regular".to_owned())
                || proportional.contains(&"emoji-icon-font".to_owned()),
            "proportional fallback should include an emoji font: {proportional:?}"
        );
        let mono = definitions
            .families
            .get(&FontFamily::Monospace)
            .expect("monospace family");
        assert!(mono.contains(&"Hack".to_owned()));
    }

    #[test]
    fn every_ui_symbol_has_a_glyph_in_bundled_fonts() {
        let mut fonts = Fonts::new(TextOptions::default(), FontDefinitions::default());
        let font_id = FontId::proportional(14.0);
        for symbol in ALL {
            for character in symbol.chars() {
                assert!(
                    fonts.has_glyph(&font_id, character),
                    "missing glyph for {character:?} (U+{:04X}) in symbol {symbol:?}",
                    character as u32
                );
            }
        }
    }

    #[test]
    fn preferred_half_circle_u25d0_is_absent_so_u25d1_is_used() {
        let mut fonts = Fonts::new(TextOptions::default(), FontDefinitions::default());
        let font_id = FontId::proportional(14.0);
        assert!(
            !fonts.has_glyph(&font_id, '\u{25D0}'),
            "U+25D0 unexpectedly present; prefer it over U+25D1 for RUNNING"
        );
        assert!(fonts.has_glyph(&font_id, '\u{25D1}'));
        assert_eq!(super::RUNNING, "\u{25D1}");
    }

    #[test]
    fn no_character_is_used_for_two_unrelated_meanings() {
        let mut owner: HashMap<char, &'static str> = HashMap::new();
        for (meaning, symbol) in MEANINGS {
            for character in symbol.chars() {
                if let Some(previous) = owner.insert(character, meaning) {
                    assert!(
                        previous == *meaning || warning_pair(previous, meaning),
                        "character {character:?} (U+{:04X}) is used for both '{previous}' and '{meaning}'",
                        character as u32
                    );
                }
            }
        }
    }
}
