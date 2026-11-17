# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

- Desktop date display uses `sys-locale` for the OS language first (then
  `LANG` / `LC_*`), so German Windows and Finder-launched macOS builds show
  `DD.MM.YYYY` without relying on environment variables. See
  [ADR 0026](docs/adr/0026-navigation-and-visual-design.md).

### Added

- Desktop UI overhaul (AP14): left sidebar (Portfolio, Issues), roadmap
  colour palette and status glyphs via egui `monochrome_emoji_fonts`,
  cross-project Issues view with filter/sort/status/undo, locale-aware
  dates (DD.MM.YYYY / YYYY-MM-DD), readable Android labels, relative
  lockfile picks, clearer source-folder fields, and `craftward show`
  stack output. See [ADR 0026](docs/adr/0026-navigation-and-visual-design.md),
  [docs/desktop.md](docs/desktop.md), and [docs/cli.md](docs/cli.md).
- Desktop stack and watch editing: components, lockfiles (with native
  Browse and existence/type checks), Android settings, and watch items
  under Watch items & deadlines. Core and CLI gain `watch set`. See
  [docs/desktop.md](docs/desktop.md) and [docs/cli.md](docs/cli.md).
- Desktop editing with undo toasts (about eight seconds), sticky conflict
  banners, workspace project create/add/remove, overview header edit, phase
  add/edit/remove, task and issue add/edit/remove, and shared
  fingerprint-checked writers for project and workspace files. Core and CLI
  gain `phase set`, `task set`, `issue set`, header tags/links, and restore
  helpers. See [ADR 0024](docs/adr/0024-editing-from-the-desktop-app.md) and
  [docs/desktop.md](docs/desktop.md).
- Native folder/file browse dialogs via rfd (XDG Desktop Portal on Linux),
  OS clipboard through eframe's `clipboard` feature (BSL-1.0 allowed in
  `deny.toml` for Windows clipboard crates), id suggestion on new projects,
  list remove without a dialog, confirmation only when deleting
  `craftward.json`, `craftward remove ID --delete-file`, and compact detail
  action buttons. See
  [ADR 0025](docs/adr/0025-native-dialogs-and-clipboard.md).
- Desktop app foundation in `craftward-app` (eframe 0.37.0, glow renderer,
  accesskit, Wayland and X11): dark theme aligned with the HTML roadmaps,
  portfolio overview sorted by urgency, and a per-project detail view.
  Same workspace and `craftward.db` paths as the CLI. See
  [ADR 0023](docs/adr/0023-desktop-app-with-eframe.md) and
  [docs/desktop.md](docs/desktop.md).
- Repository skeleton with a Cargo workspace, command-line and desktop
  application binaries, CI, dependency policy, and project documentation.
- Schema version 1 of `craftward.json`, with loading and validation in
  `craftward-core` and `craftward validate` in the command-line tool.
- `schema/craftward.schema.json`, generated from the Rust types.
- Example project files and `docs/file-format.md`.
- First third-party dependencies: serde 1.0.229, serde_json 1.0.154, and
  schemars 1.2.3.
- A `workspace.json` registry of project files, with `init`, `add`, `remove`,
  `list`, and `workspace path` commands. `validate` without paths checks that
  registry. `cargo deny` checks all Cargo features, including optional
  `schemars`.
- Optional `source_dir` on a project file. Lockfiles are resolved from that
  directory. `validate` warns when the directory or a lockfile is missing.
- `craftward roadmap`, which writes the next versioned HTML roadmap and does
  not overwrite an existing file. When hard links are unavailable, the file
  is created exclusively instead. Fixed labels are English or German. A
  value that is both a project id and an existing path is rejected.
- `show`, `project set`, `phase`, `task`, `issue`, and `watch`, which edit a
  project file through `craftward-core`. The file is replaced only when it
  is still valid, in one canonical form. Omitted ids are formed from titles,
  and `updated` is set to the UTC date.
- `craftward radar`, which reads the lockfiles named by a project and asks
  OSV which package versions have a known vulnerability. Results are cached
  as JSON. The HTTP client lives in `craftward-net` (ureq 3.5.2, rustls
  0.23.46 with the ring provider, and rustls-native-certs 0.8.5).
- A local SQLite database for radar findings, runs, and history, in
  `craftward-store` (rusqlite 0.43.0 with bundled SQLite 3.54.1). `radar`
  stores each run. `findings` lists, shows, ignores, and reopens findings.
  A finding that disappears is marked resolved. Exit status 3 means a
  finding is open.
- Advisory kinds for OSV records: `vulnerability`, `malicious`,
  `unmaintained`, `unsound`, and `notice`. A withdrawn record is not stored.
  A finding whose record was withdrawn is resolved with a `withdrawn` event.
  Exit status 3 is reserved for vulnerabilities, malicious packages, and a
  support end that has passed. Notices and an approaching support end count
  with `--strict`.
- End-of-support checks through the endoflife.date API v1. A component may
  set `eol_product`. Findings use the ids `EOL:<product>:<cycle>` and the
  same lifecycle as OSV findings. The data is under the MIT License.
- `stack add`, `stack set`, `stack remove`, and `stack lockfile add` /
  `remove`, which edit components and lockfile paths. The database schema
  is version 3 and adds `kind` on findings.
- Google Play target-API deadlines from `data/platform-deadlines.json`,
  compiled into the program. `stack android` records `targetSdk` and the
  files it is read from. `radar` reports `platform_deadline` and
  `platform_deadline_soon`. `deadlines` prints the rows in use. The
  database schema is version 5: version 4 stores those kinds, and version 5
  records `deadline_passed` when a warning becomes a missed deadline. One
  finding per form factor and requirement uses the newest reached row.

### Fixed

- Portfolio card ··· menu opens again: card click sense is registered under
  children so the menu button keeps pointer and keyboard priority; clicking
  the rest of the card still opens the project.
- Portfolio card menu clicks no longer open the project detail page.
- Desktop status chips are the menu button itself (no double frame); titles
  stay column-aligned via a shared chip width.
- Desktop status and stack symbols use only glyphs present in egui's
  bundled fonts; a unit test fails if a new symbol lacks a glyph.
- Portfolio status badges shrink to their text width instead of stretching
  across the card.
- A single card in the last portfolio grid row keeps the normal column
  width instead of spanning the full row.
- SeQu check F1 (SQ-01, SQ-02, SQ-03): file replacement now creates the
  temporary file exclusively, syncs it before publishing, and syncs the
  parent directory on Unix. A migration backup uses SQLite's online backup
  API, including committed WAL frames, and must pass `integrity_check`.
  Writing transactions start with `BEGIN IMMEDIATE`, so a second writer
  waits.
- SeQu check F2 (SQ-06, SQ-15, and the database parts of SQ-14): finding
  kind, severity, event kind, HTTP method, and lockfile kind are enums.
  Their stored text is unchanged. Schema version 6 drops the `CHECK` lists
  for finding state, finding kind, and event kind; an unknown word fails
  when the row is read. `findings list` filters state and project in SQL.
  The busy timeout is set once.
- SeQu check F3 (SQ-05, SQ-07, and the remaining SQ-14 points): shared
  modules for JSON validation helpers, percent-encoding, timed JSON cache,
  path joining, and HTTP body checks. OSV, endoflife.date, and the deadlines
  file use typed serde. `workspace.json` is written through Serialize. Cache
  and project serialization failures are returned instead of writing `{}`.
- SeQu check F4 (SQ-04, SQ-09, SQ-10, SQ-11, SQ-12, and stricter OSV /
  endoflife.date parsing): `targetSdk` ignores strings and
  `testOptions` / `lint` / `managedDevices` blocks, and the lowest value
  wins. Cargo.lock matches only the official crates.io source ids and
  reads strings as UTF-8. Cache names that are only dots or start with `.`
  are percent-encoded (existing entries for those names are rebuilt).
  Relative XDG base directories are ignored. Batch `vulns`, advisory
  `affected`, and EOL fields that raise or suppress findings fail the run
  when present with the wrong shape.
- SeQu check F5 (SQ-13): dependency manifests use caret version
  requirements instead of `=`, CI Clippy, tests, and `cargo deny` run with
  `--locked`, and Dependabot opens weekly grouped update pull requests for
  Cargo and GitHub Actions.
- SeQu check F6 (SQ-08): `craftward-cli` parses arguments with clap 4.6.7
  (Derive API; no default features; `std`, `derive`, `help`, `usage`,
  `error-context`, `suggestions`). The hand-written parser is removed.
