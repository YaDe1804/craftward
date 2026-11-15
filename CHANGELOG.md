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
