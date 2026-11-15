# Craftward

Craftward is a local-first desktop application and command-line tool for
developers who maintain many projects. It shows status, plans, and bugs across
all projects. Its maintenance radar reports required updates, vulnerabilities,
and platform deadlines.

Target platforms are Windows, Linux, and macOS.

## Status

Pre-alpha, no usable release yet.

## Planned features

- A cross-project view of status, plans, and bugs
- A maintenance radar for required updates, vulnerabilities, and platform deadlines
- A desktop application and a command-line interface
- Windows, Linux, and macOS from the first release

Accepted design decisions are recorded in [`docs/adr/`](docs/adr/).

## Build

Install Rust with [rustup](https://rustup.rs/). This repository pins the
toolchain in `rust-toolchain.toml` (Rust 1.100.0, including rustfmt and clippy).

```console
cargo build --workspace
cargo test --workspace
cargo run -p craftward-cli -- --version
cargo run -p craftward-app -- --version
```

Both executables print their name and version. `--version` prints the same
line. `craftward --help` describes the commands.

`craftward validate` checks project files. With no paths, it checks every
project in the [workspace](docs/workspace-format.md). The process exits 0 when
every file is valid, 1 when a file is invalid or cannot be read, and 2 when
the command line is wrong. A missing source directory or lockfile is a
warning and does not change that status. `init`, `add`, `remove`, `list`,
`roadmap`, and `workspace path` are described in [docs/cli.md](docs/cli.md).

```console
cargo run -p craftward-cli -- validate examples/minimal.craftward.json examples/full.craftward.json
```

Checks used by CI:

```console
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo deny check
```

`cargo deny` is not part of the Rust toolchain. CI runs cargo-deny 0.20.2.
Install the same version locally with:

```console
cargo install cargo-deny --version 0.20.2 --locked
```

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE).

## Project documents

- [Contributing](CONTRIBUTING.md)
- [Security](SECURITY.md)
- [Changelog](CHANGELOG.md)
- [Architecture decisions](docs/adr/)
- [Project file format](docs/file-format.md)
- [Workspace file](docs/workspace-format.md)
- [Command-line interface](docs/cli.md)
- [Roadmap](docs/roadmap/)
