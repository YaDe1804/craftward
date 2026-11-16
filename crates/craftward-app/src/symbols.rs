//! UI symbols that must render with egui's bundled fonts.
//!
//! Only characters covered by the default proportional fonts may appear here.
//! The glyph coverage test in this module fails when a new symbol lacks a glyph.

/// Idea / open / empty marker.
pub const OPEN: &str = "o";

/// Work in progress.
pub const RUNNING: &str = "~";

/// Completed or filled marker.
pub const DONE: &str = "+";

/// Released / strong filled marker.
pub const RELEASED: &str = "*";

/// Maintenance.
pub const MAINTENANCE: &str = "=";

/// Paused.
pub const PAUSED: &str = "||";

/// Discontinued / dropped / closed.
pub const CLOSED: &str = "x";

/// Dropped / won't-fix dash.
pub const DROPPED: &str = "-";

/// Low severity.
pub const SEVERITY_LOW: &str = "v";

/// Medium severity.
pub const SEVERITY_MEDIUM: &str = "*";

/// High severity.
pub const SEVERITY_HIGH: &str = "^";

/// Critical severity / attention.
pub const SEVERITY_CRITICAL: &str = "!";

/// Unknown / other.
pub const UNKNOWN: &str = "?";

/// Notice / info.
pub const NOTICE: &str = "i";

/// Review.
pub const REVIEW: &str = "@";

/// Unmaintained / soft marker.
pub const SOFT: &str = "~";

/// Task deadline kind.
pub const TASK: &str = "T";

/// Platform component.
pub const PLATFORM: &str = "#";

/// Language component.
pub const LANGUAGE: &str = "Aa";

/// Framework component.
pub const FRAMEWORK: &str = "%";

/// Runtime component.
pub const RUNTIME: &str = ">";

/// SDK component.
pub const SDK: &str = "$";

/// Service component.
pub const SERVICE: &str = "~";

/// Tool component.
pub const TOOL: &str = "+";

/// Generic / other bullet.
pub const OTHER: &str = "·";

/// Every symbol string drawn by the desktop UI.
///
/// Keep this list complete: the glyph test walks it.
#[cfg_attr(not(test), allow(dead_code))]
pub const ALL: &[&str] = &[
    OPEN,
    RUNNING,
    DONE,
    RELEASED,
    MAINTENANCE,
    PAUSED,
    CLOSED,
    DROPPED,
    SEVERITY_LOW,
    SEVERITY_MEDIUM,
    SEVERITY_HIGH,
    SEVERITY_CRITICAL,
    UNKNOWN,
    NOTICE,
    REVIEW,
    SOFT,
    TASK,
    PLATFORM,
    LANGUAGE,
    FRAMEWORK,
    RUNTIME,
    SDK,
    SERVICE,
    TOOL,
    OTHER,
];

#[cfg(test)]
mod tests {
    use eframe::egui::{FontDefinitions, FontId};
    use eframe::epaint::text::{Fonts, TextOptions};

    use super::ALL;

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
}
