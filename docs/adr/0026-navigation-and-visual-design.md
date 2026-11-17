# ADR 0026: Navigation and visual design

- Status: Accepted
- Date: 2026-11-17

## Decision

### Navigation

`craftward-app` uses a fixed left sidebar with **Portfolio**, **Issues**,
and **Settings** (Radar can still be added later). The project detail page
belongs to Portfolio; the top breadcrumb is `Portfolio › <name>` with
ordinary word spacing and **Portfolio** clickable (Esc returns there). A
separate Back button is not shown when that breadcrumb is available.
Workspace and database paths sit at the bottom of the sidebar, not in the
footer.

### Colour palette

Named constants in `theme.rs` match the dark HTML roadmaps: background
`#0e1116`, card `#161b22`, card2 `#1c232c`, line `#2a323d`, text `#e6edf3`,
muted `#8b98a7`, done `#3fb950`, in progress `#d29922`, open `#6e7b8b`,
accent `#58a6ff`. Severity colours are derived from that set. Open-status
*text* uses a lightened swatch so every text colour meets WCAG AA 4.5:1 on
background, card, and card2. Call sites must use the named constants.

Project lifecycle statuses have their own named swatches (`idea`
`#a371f7`, `in_development` `#d29922`, `released` `#3fb950`, `maintenance`
`#58a6ff`, `paused` `#8b98a7`, `discontinued` `#a5534f`), mapped in one
place (`project_status_colour` / `project_status_style`). Project-status
badges use text, stroke, and a tinted fill from that mapping; task and
issue badges keep the open / running / done colours.

### Card status glow

Portfolio cards and the project Overview header show a soft radial glow in
the project status colour (upper half, fading into the card fill). One
grayscale elliptical texture is built at startup and tinted per card; glow
opacity animates with egui (`CARD_GLOW_OPACITY_IDLE` / `_HOVER` /
`_ACTIVE`, caps 40 / 50 / 60 %, ~120 ms). WCAG AA checks only direct card
text that sits on the glow, using the texture falloff at each row’s height;
status and severity badges keep opaque fills so the glow does not affect
their contrast; text below the glow is checked on the card surface. The
status badge (text + glyph) remains the primary signal; colour alone is not.

### Symbols

egui's `monochrome_emoji_fonts` feature is enabled (via the workspace `egui`
dependency; eframe 0.37 does not forward the feature). Status glyphs are
U+2714 done, U+25D1 in progress, U+25CB open, U+2716 dropped; U+26A0 warns
for high and critical severity. U+25D0 (spec preference for in progress) is
missing from Ubuntu-Light and the monochrome emoji fonts, so U+25D1 is used
instead. Project statuses use a glyph only when a clear one exists (idea /
in development). Every badge still shows English text beside the symbol.
Noto Emoji and emoji-icon-font ship inside `epaint_default_fonts`; their
OFL-1.1 licence is already covered by the existing `deny.toml` exception for
that crate. Enabling the feature added no packages to the lockfile measured
for this ADR (412).

### Dates in the app

Calendar dates are stored as ISO `YYYY-MM-DD`. The desktop UI shows and
accepts the system-locale form: German locales use `DD.MM.YYYY`, others use
ISO; both input forms are accepted. Locale resolution uses
[sys-locale](https://crates.io/crates/sys-locale) 0.3.2 (MIT OR Apache-2.0)
for the OS setting first (so Windows and Finder-launched macOS apps work
without `LANG`), then `LC_TIME` / `LC_ALL` / `LANG`, then ISO. The source is
injectable for tests. Calendar widgets are out of scope for this decision.

### Issues view

The Issues sidebar entry lists issues across all projects (project,
severity, status, title) with filters (status default open-only, severity,
project) and sort severity descending, then project, then title. Changing
status uses the same writers and undo toast as the project page. Clicking a
row opens that project with the issue marked.

## Consequences

- Visual tokens stay in `theme.rs` / `symbols.rs`; no hard-coded colours or
  ad-hoc glyphs elsewhere in the app crate.
- Future nav destinations are sidebar entries, not a second navigation
  pattern.
- CLI `show` remains the text mirror for stack fields; the app does not
  invent a parallel stack vocabulary.
