# Contributing to Craftward

Thank you for contributing. Craftward is pre-alpha. The repository currently
contains the project skeleton only.

## Language

Code, documentation, command-line output, and commit messages are written in
English.

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
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check
```

Formatting can be applied with `cargo fmt --all`.

The workspace has no third-party runtime dependencies. Application code stays
in this repository. Add a third-party library only when it is necessary,
reviewed, and licensed under a license allowed by `deny.toml`. CI enforces that
policy.

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
