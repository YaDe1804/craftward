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
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo deny check
```

CI runs Clippy and the tests with `--all-features` so the checked-in JSON
Schema is compared with the schema generated from the Rust types. Formatting
can be applied with `cargo fmt --all`.

`serde` and `serde_json` are normal dependencies of `craftward-core`.
`schemars` is optional. The `schema` feature enables it, and that feature is
not a default feature. The command-line tool and the desktop app leave it
off, because they validate project files and do not generate a JSON Schema.
Regenerate `schema/craftward.schema.json` after a change to the project types:

```console
cargo run -p craftward-core --features schema --example generate-schema
```

Application code stays in this repository. Add a third-party library only when
it is necessary, reviewed, and licensed under a license allowed by `deny.toml`.
CI enforces that policy.

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
