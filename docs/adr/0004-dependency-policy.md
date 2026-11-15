# ADR 0004: Dependency policy

- Status: Accepted
- Date: 2026-11-14

## Decision

Own the application code. Use third-party libraries only when necessary, after
review, under a permissive license, and enforce that policy in CI. Accepted
crates are recorded in ADRs (for example ADR 0011 for serde/`serde_json`/
schemars, ADR 0022 for clap in `craftward-cli`, ADR 0023 for eframe in
`craftward-app`). Narrow license or advisory exceptions belong in
`deny.toml` with an ADR rationale; see CONTRIBUTING.md.

## Rationale

Third-party libraries are used only when necessary, reviewed, permissively
licensed, and enforced in CI. Each addition needs an ADR entry and a
`deny.toml` license check.
