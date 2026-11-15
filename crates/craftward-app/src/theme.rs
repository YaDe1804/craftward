//! Named colours, spacing, and type scale for the desktop UI.
//!
//! Values match the dark HTML roadmaps. Call sites must use these names; do
//! not hard-code RGB or font sizes elsewhere in this crate.

use eframe::egui::{Color32, CornerRadius, FontId, Margin, Stroke, Style, TextStyle, Visuals};

/// Window and panel background (`#0e1116`).
pub const BACKGROUND: Color32 = Color32::from_rgb(0x0e, 0x11, 0x16);

/// Card surface (`#161b22`).
pub const CARD: Color32 = Color32::from_rgb(0x16, 0x1b, 0x22);

/// Nested card / secondary surface (`#1c232c`).
pub const CARD_INNER: Color32 = Color32::from_rgb(0x1c, 0x23, 0x2c);

/// Borders and dividers (`#2a323d`).
pub const LINE: Color32 = Color32::from_rgb(0x2a, 0x32, 0x3d);

/// Primary body text (`#e6edf3`).
pub const TEXT: Color32 = Color32::from_rgb(0xe6, 0xed, 0xf3);

/// Secondary / muted text (`#8b98a7`).
pub const MUTED: Color32 = Color32::from_rgb(0x8b, 0x98, 0xa7);

/// Strong section heading (neutral, not accent).
pub const HEADING: Color32 = Color32::from_rgb(0xf0, 0xf3, 0xf6);

/// Accent for controls and progress only (`#58a6ff`).
pub const ACCENT: Color32 = Color32::from_rgb(0x58, 0xa6, 0xff);

/// Completed work (`#3fb950`).
pub const DONE: Color32 = Color32::from_rgb(0x3f, 0xb9, 0x50);

/// Work in progress (`#d29922`).
pub const RUNNING: Color32 = Color32::from_rgb(0xd2, 0x99, 0x22);

/// Critical severity marker (`#f85149`).
pub const SEVERITY_CRITICAL: Color32 = Color32::from_rgb(0xf8, 0x51, 0x49);

/// High severity marker (`#db6d28`).
pub const SEVERITY_HIGH: Color32 = Color32::from_rgb(0xdb, 0x6d, 0x28);

/// Medium severity marker (`#d29922`).
pub const SEVERITY_MEDIUM: Color32 = RUNNING;

/// Low severity marker (`#8b98a7`).
pub const SEVERITY_LOW: Color32 = MUTED;

/// Empty progress track fill (`#232b35`).
pub const PROGRESS_TRACK: Color32 = Color32::from_rgb(0x23, 0x2b, 0x35);

/// Outer page margin in points.
pub const PAGE_MARGIN: i8 = 16;

/// Gap between portfolio cards in points.
pub const CARD_GAP: f32 = 12.0;

/// Inner padding of a card in points.
pub const CARD_PADDING: i8 = 14;

/// Corner radius for cards and buttons.
pub const CARD_ROUNDING: u8 = 10;

/// Smallest allowed font size in points.
#[cfg_attr(not(test), allow(dead_code))]
pub const MIN_FONT_SIZE: f32 = 12.0;

/// Width from which the portfolio uses two columns.
pub const GRID_TWO_COLUMNS_FROM: f32 = 900.0;

/// Width from which the portfolio uses three columns.
pub const GRID_THREE_COLUMNS_FROM: f32 = 1400.0;

/// Width from which the detail view uses two columns.
pub const DETAIL_TWO_COLUMNS_FROM: f32 = 900.0;

/// Named type scale. Every UI text uses one of these sizes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TypeScale {
    /// Window / project title.
    pub title: f32,
    /// Section heading.
    pub section: f32,
    /// Body copy.
    pub body: f32,
    /// Secondary info (ids, paths). Must be ≥ [`MIN_FONT_SIZE`].
    pub secondary: f32,
}

/// The Craftward type scale.
pub const TYPE: TypeScale = TypeScale {
    title: 22.0,
    section: 16.0,
    body: 14.0,
    secondary: 12.0,
};

impl TypeScale {
    /// Every size in the scale, for tests.
    #[must_use]
    #[cfg_attr(not(test), allow(dead_code))]
    pub const fn all(self) -> [f32; 4] {
        [self.title, self.section, self.body, self.secondary]
    }

    /// Font id for titles.
    #[must_use]
    pub fn title_id(self) -> FontId {
        FontId::proportional(self.title)
    }

    /// Font id for section headings.
    #[must_use]
    pub fn section_id(self) -> FontId {
        FontId::proportional(self.section)
    }

    /// Font id for body text.
    #[must_use]
    pub fn body_id(self) -> FontId {
        FontId::proportional(self.body)
    }

    /// Font id for secondary text.
    #[must_use]
    pub fn secondary_id(self) -> FontId {
        FontId::proportional(self.secondary)
    }
}

/// Applies the Craftward dark theme to an egui style.
#[must_use]
pub fn style() -> Style {
    let mut style = Style {
        visuals: visuals(),
        ..Style::default()
    };
    style.spacing.item_spacing = eframe::egui::vec2(8.0, 6.0);
    style.spacing.button_padding = eframe::egui::vec2(10.0, 6.0);
    style.spacing.window_margin = Margin::same(PAGE_MARGIN);
    style
        .text_styles
        .insert(TextStyle::Heading, TYPE.title_id());
    style.text_styles.insert(TextStyle::Body, TYPE.body_id());
    style.text_styles.insert(TextStyle::Button, TYPE.body_id());
    style
        .text_styles
        .insert(TextStyle::Small, TYPE.secondary_id());
    style
        .text_styles
        .insert(TextStyle::Monospace, TYPE.body_id());
    style
}

fn visuals() -> Visuals {
    let mut visuals = Visuals::dark();
    visuals.override_text_color = Some(TEXT);
    visuals.panel_fill = BACKGROUND;
    visuals.window_fill = BACKGROUND;
    visuals.extreme_bg_color = CARD_INNER;
    visuals.faint_bg_color = CARD;
    visuals.widgets.noninteractive.bg_fill = CARD;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, LINE);
    visuals.widgets.noninteractive.corner_radius = CornerRadius::same(CARD_ROUNDING);
    visuals.widgets.inactive.bg_fill = CARD;
    visuals.widgets.inactive.weak_bg_fill = CARD_INNER;
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, LINE);
    visuals.widgets.inactive.corner_radius = CornerRadius::same(CARD_ROUNDING);
    visuals.widgets.hovered.bg_fill = CARD_INNER;
    visuals.widgets.hovered.weak_bg_fill = CARD_INNER;
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, ACCENT);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT);
    visuals.widgets.hovered.corner_radius = CornerRadius::same(CARD_ROUNDING);
    visuals.widgets.active.bg_fill = CARD_INNER;
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, ACCENT);
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
    visuals.widgets.active.corner_radius = CornerRadius::same(CARD_ROUNDING);
    visuals.selection.bg_fill = Color32::from_rgba_unmultiplied(0x58, 0xa6, 0xff, 48);
    visuals.selection.stroke = Stroke::new(1.0, ACCENT);
    visuals.window_corner_radius = CornerRadius::same(CARD_ROUNDING);
    visuals.menu_corner_radius = CornerRadius::same(CARD_ROUNDING);
    visuals
}

/// Relative luminance of an sRGB colour (WCAG 2.1).
#[must_use]
#[cfg_attr(not(test), allow(dead_code))]
pub fn relative_luminance(colour: Color32) -> f64 {
    let channels = [colour.r(), colour.g(), colour.b()].map(|byte| {
        let value = f64::from(byte) / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    });
    0.2126 * channels[0] + 0.7152 * channels[1] + 0.0722 * channels[2]
}

/// Contrast ratio of two colours (WCAG 2.1). Always ≥ 1.0.
#[must_use]
#[cfg_attr(not(test), allow(dead_code))]
pub fn contrast_ratio(foreground: Color32, background: Color32) -> f64 {
    let light = relative_luminance(foreground);
    let dark = relative_luminance(background);
    let (higher, lower) = if light >= dark {
        (light, dark)
    } else {
        (dark, light)
    };
    (higher + 0.05) / (lower + 0.05)
}

#[cfg(test)]
mod tests {
    use super::{
        ACCENT, BACKGROUND, CARD, CARD_INNER, DONE, MIN_FONT_SIZE, MUTED, RUNNING, TEXT, TYPE,
        contrast_ratio,
    };

    #[test]
    fn text_colours_meet_wcag_aa_against_surfaces() {
        let pairs = [
            (TEXT, BACKGROUND),
            (TEXT, CARD),
            (TEXT, CARD_INNER),
            (MUTED, BACKGROUND),
            (MUTED, CARD),
            (ACCENT, BACKGROUND),
            (ACCENT, CARD),
            (DONE, BACKGROUND),
            (DONE, CARD),
            (RUNNING, BACKGROUND),
            (RUNNING, CARD),
        ];
        for (foreground, background) in pairs {
            let ratio = contrast_ratio(foreground, background);
            assert!(
                ratio >= 4.5,
                "contrast {ratio:.2}:1 is below 4.5:1 for {foreground:?} on {background:?}"
            );
        }
    }

    #[test]
    fn type_scale_keeps_secondary_at_least_twelve_points() {
        for size in TYPE.all() {
            assert!(
                size >= MIN_FONT_SIZE,
                "font size {size} is below the {MIN_FONT_SIZE} pt minimum"
            );
        }
        assert_eq!(TYPE.secondary, MIN_FONT_SIZE);
    }
}
