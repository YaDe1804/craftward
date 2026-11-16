//! UI symbols that must render with egui's bundled fonts.
//!
//! With eframe's `default_fonts` feature (and without `monochrome_emoji_fonts`),
//! egui loads only **Ubuntu-Light**, **Hack**, and **egui-icons**. Noto Emoji and
//! emoji-icon-font are not installed. The status glyphs we would prefer
//! (U+2714 check, U+25D0 half circle, U+25CB empty circle, U+2716 cross,
//! U+26A0 warning) are missing from that set, so every badge uses **label text
//! only** (plus colour). An empty symbol is better than a character that means
//! something else (for example √ or ×).
//!
//! Each constant is one meaning. No character is reused across meanings.
//! [`Fonts::has_glyph`] walks the full proportional fallback chain.

use eframe::egui::FontDefinitions;

/// Open / idea / not started — text only (U+25CB not in bundled fonts).
pub const OPEN: &str = "";

/// In progress / in development — text only (U+25D0 not in bundled fonts).
pub const RUNNING: &str = "";

/// Done / resolved — text only (U+2714 not in bundled fonts).
pub const DONE: &str = "";

/// Released project — text only (no clear glyph in bundled fonts).
pub const RELEASED: &str = "";

/// Maintenance project — text only.
pub const MAINTENANCE: &str = "";

/// Paused project — text only.
pub const PAUSED: &str = "";

/// Discontinued project — text only.
pub const CLOSED: &str = "";

/// Dropped task / won't-fix issue — text only (U+2716 not in bundled fonts).
pub const DROPPED: &str = "";

/// Low severity — text and colour only.
pub const SEVERITY_LOW: &str = "";

/// Medium severity — text and colour only.
pub const SEVERITY_MEDIUM: &str = "";

/// High severity — text and colour only (U+26A0 not in bundled fonts).
pub const SEVERITY_HIGH: &str = "";

/// Critical severity — text and colour only (U+26A0 not in bundled fonts).
pub const SEVERITY_CRITICAL: &str = "";

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
pub const ALL: &[&str] = &[];

/// Font names installed for the proportional family under our eframe features.
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

    #[test]
    fn default_fonts_load_ubuntu_hack_and_egui_icons_only() {
        let definitions = FontDefinitions::default();
        let mut names: Vec<&str> = definitions.font_data.keys().map(String::as_str).collect();
        names.sort_unstable();
        assert_eq!(names, ["Hack", "Ubuntu-Light", "egui-icons"]);
        assert_eq!(
            FontDefinitions::builtin_font_names(),
            &["Ubuntu-Light", "egui-icons", "Hack"]
        );
        assert_eq!(
            proportional_font_names(),
            ["Ubuntu-Light".to_owned(), "egui-icons".to_owned()]
        );
        assert!(
            !definitions.font_data.contains_key("NotoEmoji-Regular"),
            "NotoEmoji is only present with egui's monochrome_emoji_fonts feature"
        );
        assert!(
            !definitions.font_data.contains_key("emoji-icon-font"),
            "emoji-icon-font is only present with egui's monochrome_emoji_fonts feature"
        );
        let mono = definitions
            .families
            .get(&FontFamily::Monospace)
            .expect("monospace family");
        assert_eq!(mono, &["Hack", "Ubuntu-Light", "egui-icons"]);
    }

    #[test]
    fn preferred_status_glyphs_are_absent_without_emoji_fonts() {
        let mut fonts = Fonts::new(TextOptions::default(), FontDefinitions::default());
        let font_id = FontId::proportional(14.0);
        for (label, character) in [
            ("check U+2714", '✔'),
            ("half circle U+25D0", '◐'),
            ("empty circle U+25CB", '○'),
            ("cross U+2716", '✖'),
            ("warning U+26A0", '⚠'),
        ] {
            assert!(
                !fonts.has_glyph(&font_id, character),
                "{label} unexpectedly present in the proportional fallback chain"
            );
        }
    }

    #[test]
    fn every_ui_symbol_has_a_glyph_in_bundled_fonts() {
        let mut fonts = Fonts::new(TextOptions::default(), FontDefinitions::default());
        let font_id = FontId::proportional(14.0);
        // has_glyph consults every font in FontFamily::Proportional.
        assert_eq!(
            proportional_font_names(),
            ["Ubuntu-Light".to_owned(), "egui-icons".to_owned()]
        );
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
    fn no_character_is_used_for_two_meanings() {
        let mut owner: HashMap<char, &'static str> = HashMap::new();
        for (meaning, symbol) in MEANINGS {
            for character in symbol.chars() {
                if let Some(previous) = owner.insert(character, meaning) {
                    assert_eq!(
                        previous, *meaning,
                        "character {character:?} (U+{:04X}) is used for both '{previous}' and '{meaning}'",
                        character as u32
                    );
                }
            }
        }
    }
}
