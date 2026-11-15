# ADR 0022: Command line parsing with clap

- Status: Accepted
- Date: 2026-11-15

## Decision

`craftward-cli` parses its command line with clap 4.6.7 using the Derive
API. Default features are off. Enabled features are `std`, `derive`,
`help`, `usage`, `error-context`, and `suggestions`. The `color` feature
is not enabled: coloured errors would pull roughly ten extra crates for
little gain in a tool that already prints plain `error:` lines. Clap is
a dependency of `craftward-cli` only. `craftward-core` and `craftward-app`
do not use it.

The lockfile gains seven packages that were not already present:
`clap`, `clap_builder`, `clap_derive`, `clap_lex`, `anstyle`, `heck`, and
`strsim`. Their licenses are MIT or Apache-2.0 (or both) and are allowed
by `deny.toml`.

Subcommands, options, and value sets are declared on Rust types. Global
options are `--workspace`, `--data-dir`, and `--cache-dir`. Mutually
exclusive flags such as `--refresh` and `--offline` use clap attributes.
Usage errors keep exit status 2.

## Alternatives considered

Keeping the hand-written parser was rejected. It duplicated help text,
option rules, and value checks across modules, and SQ-08 showed that
approach as a maintenance risk once the command surface grew. A smaller
custom parser library was not considered worth a second review when clap
already matches the dependency policy.

## Rationale

One declaration drives help, usage errors, and suggestions. New commands
are added as enum variants and fields instead of new parsing loops. The
feature set stays small so the CLI does not grow a terminal-colour stack
it does not need.
