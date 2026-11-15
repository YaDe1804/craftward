# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
