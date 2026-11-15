# Contributing to Craftward

Thank you for contributing. Craftward is pre-alpha.

## Language

Code, documentation, command-line output, and commit messages are written in
English.

## Line endings

Text files use LF on every operating system. `.gitattributes` sets
`text=auto eol=lf`, so a checkout does not turn them into CRLF on Windows.
Generated output is compared with the checked-in files byte for byte, and a
converted line ending would fail that comparison. Images, fonts, and other
binary formats are marked `binary` and are left unchanged.

## Setup

Install Rust with [rustup](https://rustup.rs/). Opening this repository selects
the pinned toolchain in `rust-toolchain.toml` (Rust 1.100.0, with rustfmt and
clippy).

Install the same cargo-deny release that CI runs:

```console
cargo install cargo-deny --version 0.20.2 --locked
```

## Checks

Run these before opening a pull request:

```console
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
cargo deny --locked check
```

CI runs Clippy and the tests with `--all-features` and `--locked` so the
checked-in JSON Schema is compared with the schema generated from the Rust
types, and so a stale or missing `Cargo.lock` fails the job instead of being
rewritten. Formatting can be applied with `cargo fmt --all`.

`serde` and `serde_json` are normal dependencies of `craftward-core`.
`schemars` is optional. The `schema` feature enables it, and that feature is
not a default feature. The command-line tool and the desktop app leave it
off, because they validate project files and do not generate a JSON Schema.
Regenerate `schema/craftward.schema.json` after a change to the project types:

```console
cargo run -p craftward-core --features schema --example generate-schema
```

## Dependencies

Manifest version requirements use ordinary Cargo caret ranges (for example
`1.0.229`). Exact resolved versions live in `Cargo.lock`, which is committed
and must stay in sync with the manifests. Do not run `cargo update` unless the
change is intentional and reviewed. CI Clippy, tests, and `cargo deny` use
`--locked`.

Dependabot (`.github/dependabot.yml`) opens weekly pull requests for Cargo and
GitHub Actions. Minor and patch updates are grouped per ecosystem; major
updates stay in their own pull requests. Commit messages use the
`chore(deps)` prefix. Actions pinned to a full commit SHA with a trailing
version comment (for example `# v7.0.1`) are updated together: Dependabot
rewrites both the SHA and that comment when the comment matches the pinned
version.

Application code stays in this repository. Add a third-party library only when
it is necessary, reviewed under [ADR 0004](docs/adr/0004-dependency-policy.md),
licensed under a license allowed by `deny.toml`, and recorded in a new or
updated ADR that names the crate and the reason it was accepted. CI enforces
the license and ban policy.

## Adding a CLI command

Command-line parsing lives in `crates/craftward-cli/src/cli.rs` and uses clap
Derive ([ADR 0022](docs/adr/0022-command-line-parsing-with-clap.md)). To add a
command:

1. Add a variant (or nested `Subcommand` enum) on `Command` in `cli.rs`, with
   `about` / `long_about` text and typed fields for options and arguments.
2. Wire the variant in `run` in `main.rs` (or the matching module's `run`) so
   it calls into `craftward-core`.
3. Keep clap out of `craftward-core` and `craftward-app`. Prefer core enums and
   `value_parser` helpers for allowed values.
4. Run `Cli::command().debug_assert()` coverage (the unit test in `cli.rs`) and
   the CLI integration tests.

## Pull requests

Keep changes focused. Describe why the change is needed. All CI jobs must pass.

Architecture decisions belong in `docs/adr/`. Copy `docs/adr/template.md`, use
the next number, and record only the rationale that was actually agreed.

## License

By contributing, you agree that your contributions are licensed under the
Apache License, Version 2.0. See [LICENSE](LICENSE).

## Security

Report vulnerabilities privately. See [SECURITY.md](SECURITY.md). Do not open a
public issue for a security report.
