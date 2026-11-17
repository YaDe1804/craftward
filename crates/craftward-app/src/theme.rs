//! Named colours, spacing, and type scale for the desktop UI.
//!
//! Values match the dark HTML roadmaps. Call sites must use these names; do
//! not hard-code RGB or font sizes elsewhere in this crate.

use craftward_core::ProjectStatus;
use eframe::egui::{
    Color32, ColorImage, CornerRadius, FontId, Margin, Stroke, Style, TextStyle, TextureOptions,
    Visuals,
};

/// Window and panel background (`#0e1116`).
pub const BACKGROUND: Color32 = Color32::from_rgb(0x0e, 0x11, 0x16);

/// Card surface (`#161b22`).
pub const CARD: Color32 = Color32::from_rgb(0x16, 0x1b, 0x22);

/// Nested card / secondary surface (`#1c232c`). Also known as `card2` in the roadmaps.
pub const CARD2: Color32 = Color32::from_rgb(0x1c, 0x23, 0x2c);

/// Alias for [`CARD2`] (nested surfaces).
pub const CARD_INNER: Color32 = CARD2;

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

/// Open / not started roadmap swatch (`#6e7b8b`).
///
/// Below 4.5:1 on card surfaces; use [`OPEN_TEXT`] for status labels.
#[cfg_attr(not(test), allow(dead_code))]
pub const OPEN: Color32 = Color32::from_rgb(0x6e, 0x7b, 0x8b);

/// Open-status text colour derived from [`OPEN`] so it meets WCAG AA 4.5:1
/// on [`BACKGROUND`], [`CARD`], and [`CARD2`] (`#8595a3`).
pub const OPEN_TEXT: Color32 = Color32::from_rgb(0x85, 0x95, 0xa3);

/// Critical severity marker (`#f85149`).
pub const SEVERITY_CRITICAL: Color32 = Color32::from_rgb(0xf8, 0x51, 0x49);

/// High severity marker (`#db6d28`).
pub const SEVERITY_HIGH: Color32 = Color32::from_rgb(0xdb, 0x6d, 0x28);

/// Medium severity marker (`#d29922`).
pub const SEVERITY_MEDIUM: Color32 = RUNNING;

/// Low severity marker (`#8b98a7`).
pub const SEVERITY_LOW: Color32 = MUTED;

/// Project status: idea (`#a371f7`).
pub const STATUS_IDEA: Color32 = Color32::from_rgb(0xa3, 0x71, 0xf7);

/// Project status: in development (`#d29922`).
pub const STATUS_IN_DEVELOPMENT: Color32 = Color32::from_rgb(0xd2, 0x99, 0x22);

/// Project status: released (`#3fb950`).
pub const STATUS_RELEASED: Color32 = Color32::from_rgb(0x3f, 0xb9, 0x50);

/// Project status: maintenance (`#58a6ff`).
pub const STATUS_MAINTENANCE: Color32 = Color32::from_rgb(0x58, 0xa6, 0xff);

/// Project status: paused (`#8b98a7`).
pub const STATUS_PAUSED: Color32 = Color32::from_rgb(0x8b, 0x98, 0xa7);

/// Project status: discontinued (`#a5534f`).
pub const STATUS_DISCONTINUED: Color32 = Color32::from_rgb(0xa5, 0x53, 0x4f);

/// Empty progress track fill (`#232b35`).
pub const PROGRESS_TRACK: Color32 = Color32::from_rgb(0x23, 0x2b, 0x35);

/// Idle opacity for the status glow (after WCAG AA contrast check).
///
/// Cap is 40 %; chosen as high as the refined on-glow text test allows.
pub const CARD_GLOW_OPACITY_IDLE: f32 = 0.40;

/// Hover opacity for the status glow (cap 50 %).
pub const CARD_GLOW_OPACITY_HOVER: f32 = 0.50;

/// Pressed opacity for the status glow (cap 60 %; peak used in contrast tests).
pub const CARD_GLOW_OPACITY_ACTIVE: f32 = 0.60;

/// Fraction of the card height covered by the glow (top, fading out).
pub const CARD_GLOW_HEIGHT_FRACTION: f32 = 0.5;

/// Opacity animation duration in seconds (~120 ms).
pub const CARD_GLOW_ANIMATION_SECS: f32 = 0.12;

/// Glow texture width in pixels.
pub const CARD_GLOW_TEXTURE_WIDTH: usize = 128;

/// Glow texture height in pixels.
pub const CARD_GLOW_TEXTURE_HEIGHT: usize = 64;

/// Horizontal radius of the elliptical falloff, as a fraction of texture width.
pub const CARD_GLOW_ELLIPSE_RX: f32 = 0.55;

/// Vertical radius of the elliptical falloff, as a fraction of texture height.
pub const CARD_GLOW_ELLIPSE_RY: f32 = 0.95;

/// Texture options: linear filtering, clamped edges.
pub const CARD_GLOW_TEXTURE_OPTIONS: TextureOptions = TextureOptions::LINEAR;

/// Minimum content height of a portfolio card (points), matching the UI.
pub const CARD_CONTENT_MIN_HEIGHT: f32 = 168.0;

/// Vertical item spacing used when modelling card row positions for glow contrast.
pub const CARD_ITEM_SPACING_Y: f32 = 6.0;

/// Opaque tint strength for project-status badge fills over [`CARD2`].
pub const PROJECT_STATUS_BADGE_FILL_TINT: f32 = 0.22;

/// Outer page margin in points.
pub const PAGE_MARGIN: i8 = 16;

/// Sidebar width in points.
pub const SIDEBAR_WIDTH: f32 = 168.0;

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

/// Colours for a project-status badge (text / fill / stroke) and card glow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectStatusStyle {
    /// Named status swatch (glow tint and stroke).
    pub swatch: Color32,
    /// Badge label colour, lightened when needed for ≥ 4.5:1 on [`Self::fill`].
    pub text: Color32,
    /// Lightly tinted badge background.
    pub fill: Color32,
    /// Badge border.
    pub stroke: Color32,
}

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

/// Single mapping from project lifecycle status to its named swatch.
#[must_use]
pub const fn project_status_colour(status: ProjectStatus) -> Color32 {
    match status {
        ProjectStatus::Idea => STATUS_IDEA,
        ProjectStatus::InDevelopment => STATUS_IN_DEVELOPMENT,
        ProjectStatus::Released => STATUS_RELEASED,
        ProjectStatus::Maintenance => STATUS_MAINTENANCE,
        ProjectStatus::Paused => STATUS_PAUSED,
        ProjectStatus::Discontinued => STATUS_DISCONTINUED,
    }
}

/// Badge colours derived from [`project_status_colour`].
///
/// [`ProjectStatusStyle::fill`] is an opaque RGB plate so the card glow behind
/// a badge does not change badge text contrast.
#[must_use]
pub fn project_status_style(status: ProjectStatus) -> ProjectStatusStyle {
    let swatch = project_status_colour(status);
    let fill = blend_over(CARD2, swatch, PROJECT_STATUS_BADGE_FILL_TINT);
    let text = lighten_to_contrast(swatch, fill, 4.5);
    ProjectStatusStyle {
        swatch,
        text,
        fill,
        stroke: swatch,
    }
}

/// Resolves a project-status style from a badge key, when it is a project status.
#[must_use]
pub fn project_status_style_for_key(key: &str) -> Option<ProjectStatusStyle> {
    let status = match key {
        "idea" => ProjectStatus::Idea,
        "in_development" => ProjectStatus::InDevelopment,
        "released" => ProjectStatus::Released,
        "maintenance" => ProjectStatus::Maintenance,
        "paused" => ProjectStatus::Paused,
        "discontinued" => ProjectStatus::Discontinued,
        _ => return None,
    };
    Some(project_status_style(status))
}

/// Unit weight of the glow texture at continuous pixel coordinates.
///
/// Same elliptical smoothstep falloff used when building the texture: centre at
/// top-middle (`cy = 0`), soft radial decay. Return value is 0…1 before the
/// card’s idle/hover/active opacity is applied.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn card_glow_texture_weight(x: f32, y: f32) -> f32 {
    let width = CARD_GLOW_TEXTURE_WIDTH as f32;
    let height = CARD_GLOW_TEXTURE_HEIGHT as f32;
    let cx = (width - 1.0) * 0.5;
    let cy = 0.0;
    let rx = width * CARD_GLOW_ELLIPSE_RX;
    let ry = height * CARD_GLOW_ELLIPSE_RY;
    let nx = (x - cx) / rx;
    let ny = (y - cy) / ry;
    let dist = (nx * nx + ny * ny).sqrt();
    (1.0 - smoothstep(0.0, 1.0, dist)).clamp(0.0, 1.0)
}

/// Glow texture weight at a vertical fraction of the glow rect (0 = top, 1 =
/// bottom), sampled on the horizontal centre line.
#[must_use]
#[cfg_attr(not(test), allow(dead_code))]
#[allow(clippy::cast_precision_loss)]
pub fn card_glow_weight_at_height_fraction(v: f32) -> f32 {
    let height = CARD_GLOW_TEXTURE_HEIGHT as f32;
    let width = CARD_GLOW_TEXTURE_WIDTH as f32;
    let y = v.clamp(0.0, 1.0) * (height - 1.0);
    let x = (width - 1.0) * 0.5;
    card_glow_texture_weight(x, y)
}

/// Builds the shared grayscale (white + alpha) radial glow texture.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn build_card_glow_image() -> ColorImage {
    let width = CARD_GLOW_TEXTURE_WIDTH;
    let height = CARD_GLOW_TEXTURE_HEIGHT;
    let mut pixels = Vec::with_capacity(width * height);
    for y in 0..height {
        for x in 0..width {
            let alpha = card_glow_texture_weight(x as f32, y as f32);
            pixels.push(Color32::from_rgba_unmultiplied(
                255,
                255,
                255,
                unit_to_byte(alpha),
            ));
        }
    }
    ColorImage::new([width, height], pixels)
}

/// Linear blend of `foreground` over `background` with opacity `alpha` in 0…1.
#[must_use]
pub fn blend_over(background: Color32, foreground: Color32, alpha: f32) -> Color32 {
    let alpha = alpha.clamp(0.0, 1.0);
    let inv = 1.0 - alpha;
    Color32::from_rgb(
        blend_channel(background.r(), foreground.r(), alpha, inv),
        blend_channel(background.g(), foreground.g(), alpha, inv),
        blend_channel(background.b(), foreground.b(), alpha, inv),
    )
}

/// Applies the Craftward dark theme to an egui style.
#[must_use]
pub fn style() -> Style {
    let mut style = Style {
        visuals: visuals(),
        ..Style::default()
    };
    style.spacing.item_spacing = eframe::egui::vec2(8.0, CARD_ITEM_SPACING_Y);
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
    visuals.extreme_bg_color = CARD2;
    visuals.faint_bg_color = CARD;
    visuals.widgets.noninteractive.bg_fill = CARD;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, LINE);
    visuals.widgets.noninteractive.corner_radius = CornerRadius::same(CARD_ROUNDING);
    visuals.widgets.inactive.bg_fill = CARD;
    visuals.widgets.inactive.weak_bg_fill = CARD2;
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, LINE);
    visuals.widgets.inactive.corner_radius = CornerRadius::same(CARD_ROUNDING);
    visuals.widgets.hovered.bg_fill = CARD2;
    visuals.widgets.hovered.weak_bg_fill = CARD2;
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, ACCENT);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT);
    visuals.widgets.hovered.corner_radius = CornerRadius::same(CARD_ROUNDING);
    visuals.widgets.active.bg_fill = CARD2;
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, ACCENT);
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
    visuals.widgets.active.corner_radius = CornerRadius::same(CARD_ROUNDING);
    visuals.selection.bg_fill = Color32::from_rgba_unmultiplied(0x58, 0xa6, 0xff, 48);
    visuals.selection.stroke = Stroke::new(1.0, ACCENT);
    visuals.window_corner_radius = CornerRadius::same(CARD_ROUNDING);
    visuals.menu_corner_radius = CornerRadius::same(CARD_ROUNDING);
    visuals
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn lighten_to_contrast(colour: Color32, background: Color32, minimum: f64) -> Color32 {
    if contrast_ratio(colour, background) >= minimum {
        return colour;
    }
    let mut best = colour;
    for step in 1_u8..=20 {
        let t = f32::from(step) / 20.0;
        let candidate = Color32::from_rgb(
            lighten_channel(colour.r(), t),
            lighten_channel(colour.g(), t),
            lighten_channel(colour.b(), t),
        );
        best = candidate;
        if contrast_ratio(candidate, background) >= minimum {
            return candidate;
        }
    }
    best
}

/// Maps a 0…1 unit value to an 8-bit channel.
#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn unit_to_byte(unit: f32) -> u8 {
    (unit.clamp(0.0, 1.0) * 255.0).round() as u8
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn blend_channel(background: u8, foreground: u8, alpha: f32, inv: f32) -> u8 {
    (f32::from(background) * inv + f32::from(foreground) * alpha).round() as u8
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn lighten_channel(byte: u8, t: f32) -> u8 {
    (f32::from(byte) + (255.0 - f32::from(byte)) * t).round() as u8
}

/// Relative luminance of an sRGB colour (WCAG 2.1).
#[must_use]
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
    use craftward_core::ProjectStatus;
    use eframe::egui::Color32;

    use super::{
        ACCENT, BACKGROUND, CARD, CARD_CONTENT_MIN_HEIGHT, CARD_GLOW_HEIGHT_FRACTION,
        CARD_GLOW_OPACITY_ACTIVE, CARD_GLOW_OPACITY_HOVER, CARD_GLOW_OPACITY_IDLE,
        CARD_ITEM_SPACING_Y, CARD_PADDING, CARD2, DONE, MIN_FONT_SIZE, MUTED, OPEN_TEXT, RUNNING,
        SEVERITY_CRITICAL, SEVERITY_HIGH, SEVERITY_LOW, SEVERITY_MEDIUM, STATUS_DISCONTINUED,
        STATUS_IDEA, STATUS_IN_DEVELOPMENT, STATUS_MAINTENANCE, STATUS_PAUSED, STATUS_RELEASED,
        TEXT, TYPE, blend_over, build_card_glow_image, card_glow_weight_at_height_fraction,
        contrast_ratio, project_status_colour, project_status_style,
    };

    fn all_text_colours() -> [Color32; 10] {
        [
            TEXT,
            MUTED,
            ACCENT,
            DONE,
            RUNNING,
            OPEN_TEXT,
            SEVERITY_CRITICAL,
            SEVERITY_HIGH,
            SEVERITY_MEDIUM,
            SEVERITY_LOW,
        ]
    }

    fn all_project_statuses() -> [ProjectStatus; 6] {
        [
            ProjectStatus::Idea,
            ProjectStatus::InDevelopment,
            ProjectStatus::Released,
            ProjectStatus::Maintenance,
            ProjectStatus::Paused,
            ProjectStatus::Discontinued,
        ]
    }

    /// Vertical fractions (0 = top of glow) of direct-on-card text midlines.
    ///
    /// Mirrors the portfolio card stack: padding, title, status badge (own
    /// fill — skipped here), version, id. Badge height ≈ secondary + vertical
    /// badge margins (3 pt each side).
    fn on_glow_text_rows() -> [(Color32, f32); 3] {
        let pad = f32::from(CARD_PADDING);
        let fill_height = CARD_CONTENT_MIN_HEIGHT + 2.0 * pad;
        let glow_height = fill_height * CARD_GLOW_HEIGHT_FRACTION;
        let badge_height = TYPE.secondary + 6.0;
        let title_mid = pad + TYPE.title * 0.5;
        let mut y = pad + TYPE.title + CARD_ITEM_SPACING_Y;
        y += badge_height + CARD_ITEM_SPACING_Y;
        let version_mid = y + TYPE.secondary * 0.5;
        y += TYPE.secondary + CARD_ITEM_SPACING_Y;
        let id_mid = y + TYPE.secondary * 0.5;
        [
            (TEXT, title_mid / glow_height),
            (MUTED, version_mid / glow_height),
            (MUTED, id_mid / glow_height),
        ]
    }

    #[test]
    fn text_colours_meet_wcag_aa_against_surfaces() {
        let backgrounds = [BACKGROUND, CARD, CARD2];
        for foreground in all_text_colours() {
            for background in backgrounds {
                let ratio = contrast_ratio(foreground, background);
                assert!(
                    ratio >= 4.5,
                    "contrast {ratio:.2}:1 is below 4.5:1 for {foreground:?} on {background:?}"
                );
            }
        }
        assert!(contrast_ratio(super::OPEN, CARD2) < 4.5);
        assert_eq!(super::OPEN, Color32::from_rgb(0x6e, 0x7b, 0x8b));
    }

    #[test]
    fn project_status_swatches_match_named_hex() {
        assert_eq!(project_status_colour(ProjectStatus::Idea), STATUS_IDEA);
        assert_eq!(
            project_status_colour(ProjectStatus::InDevelopment),
            STATUS_IN_DEVELOPMENT
        );
        assert_eq!(
            project_status_colour(ProjectStatus::Released),
            STATUS_RELEASED
        );
        assert_eq!(
            project_status_colour(ProjectStatus::Maintenance),
            STATUS_MAINTENANCE
        );
        assert_eq!(project_status_colour(ProjectStatus::Paused), STATUS_PAUSED);
        assert_eq!(
            project_status_colour(ProjectStatus::Discontinued),
            STATUS_DISCONTINUED
        );
        assert_eq!(STATUS_IDEA, Color32::from_rgb(0xa3, 0x71, 0xf7));
        assert_eq!(STATUS_IN_DEVELOPMENT, Color32::from_rgb(0xd2, 0x99, 0x22));
        assert_eq!(STATUS_RELEASED, Color32::from_rgb(0x3f, 0xb9, 0x50));
        assert_eq!(STATUS_MAINTENANCE, Color32::from_rgb(0x58, 0xa6, 0xff));
        assert_eq!(STATUS_PAUSED, Color32::from_rgb(0x8b, 0x98, 0xa7));
        assert_eq!(STATUS_DISCONTINUED, Color32::from_rgb(0xa5, 0x53, 0x4f));
    }

    #[test]
    fn project_status_badge_text_meets_wcag_aa_on_fill() {
        for status in all_project_statuses() {
            let style = project_status_style(status);
            assert_eq!(style.fill.a(), 255, "{status:?}: badge fill must be opaque");
            let ratio = contrast_ratio(style.text, style.fill);
            assert!(
                ratio >= 4.5,
                "{status:?}: badge text contrast {ratio:.2}:1 on fill"
            );
        }
    }

    #[test]
    fn severity_badge_text_meets_wcag_aa_on_opaque_inner_fill() {
        assert_eq!(CARD2.a(), 255);
        for foreground in [
            SEVERITY_CRITICAL,
            SEVERITY_HIGH,
            SEVERITY_MEDIUM,
            SEVERITY_LOW,
            OPEN_TEXT,
            DONE,
            RUNNING,
        ] {
            let ratio = contrast_ratio(foreground, CARD2);
            assert!(
                ratio >= 4.5,
                "{foreground:?}: severity/status badge contrast {ratio:.2}:1 on CARD2"
            );
        }
    }

    #[test]
    fn glow_on_card_text_meets_wcag_aa_at_row_height() {
        const {
            assert!(CARD_GLOW_OPACITY_IDLE < CARD_GLOW_OPACITY_HOVER);
            assert!(CARD_GLOW_OPACITY_HOVER < CARD_GLOW_OPACITY_ACTIVE);
            assert!(CARD_GLOW_OPACITY_IDLE <= 0.40);
            assert!(CARD_GLOW_OPACITY_HOVER <= 0.50);
            assert!(CARD_GLOW_OPACITY_ACTIVE <= 0.60);
        }
        let rows = on_glow_text_rows();
        for status in all_project_statuses() {
            let swatch = project_status_colour(status);
            let mut worst = f64::INFINITY;
            for (foreground, v) in rows {
                let weight = if v >= 1.0 {
                    0.0
                } else {
                    card_glow_weight_at_height_fraction(v)
                };
                let alpha = CARD_GLOW_OPACITY_ACTIVE * weight;
                let background = blend_over(CARD, swatch, alpha);
                let ratio = contrast_ratio(foreground, background);
                worst = worst.min(ratio);
                assert!(
                    ratio >= 4.5,
                    "{status:?}: {foreground:?} at v={v:.3} weight={weight:.3} \
                     contrast {ratio:.2}:1 on {background:?}"
                );
            }
            assert!(
                worst >= 4.5,
                "{status:?}: worst on-glow contrast {worst:.2}:1"
            );
        }
        for foreground in all_text_colours() {
            let ratio = contrast_ratio(foreground, CARD);
            assert!(
                ratio >= 4.5,
                "below-glow {foreground:?} contrast {ratio:.2}:1 on CARD"
            );
        }
    }

    #[test]
    fn glow_texture_is_brightest_at_top_centre() {
        let image = build_card_glow_image();
        let width = image.width();
        let height = image.height();
        let centre = image.pixels[width / 2];
        let corner = image.pixels[width * (height - 1)];
        assert!(centre.a() > corner.a());
        assert!(centre.a() > 200);
        assert!(corner.a() < 40);
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
