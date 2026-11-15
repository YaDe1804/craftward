# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

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

### Added

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
