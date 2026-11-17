# Craftward

Craftward is a local-first desktop application and command-line tool for
developers who maintain many projects. It shows status, plans, and bugs across
all projects. Its maintenance radar reports required updates, vulnerabilities,
and platform deadlines.

## Status

Pre-alpha, no usable release yet.

## Features

### Command-line interface (`craftward-cli`)

- Workspace registry: `init`, `add`, `remove` (optional `--delete-file`),
  `list`, `workspace path`, `settings show` / `settings set`, `validate`
- Project edits: `show`, `project set`, `phase` / `task` / `issue` / `watch`,
  `stack` (including Android targetSdk fields)
- Versioned HTML roadmaps (`roadmap`)
- Maintenance radar (`radar`) against OSV and endoflife.date, local findings
  (`findings`), and Google Play target-API deadlines (`deadlines`)

### Desktop application (`craftward-app`)

- First-run setup, Settings for storage locations, left sidebar
  (Portfolio, Issues, Settings), portfolio overview, per-project detail,
  and a cross-project Issues table over the same workspace file and
  `craftward.db` as the CLI
- Create, add, and remove projects; edit header, phases, tasks, issues,
  stack (components, lockfiles, Android), and watch items through the same
  writers as the CLI, with undo toasts and conflict banners
- Native folder/file/lockfile dialogs, OS clipboard, roadmap palette, and
  status glyphs (see [ADR 0025](docs/adr/0025-native-dialogs-and-clipboard.md),
  [ADR 0026](docs/adr/0026-navigation-and-visual-design.md), and
  [ADR 0027](docs/adr/0027-settings-file-and-data-relocation.md))

Target platforms are Windows, Linux, and macOS. Accepted design decisions are
recorded in [`docs/adr/`](docs/adr/README.md).

## Build

Install Rust with [rustup](https://rustup.rs/). This repository pins the
toolchain in `rust-toolchain.toml` (Rust 1.100.0, including rustfmt and clippy).

```console
cargo build --workspace
cargo test --workspace
cargo run -p craftward-cli -- --version
cargo run -p craftward-app -- --version
```

`craftward` prints help with `--help`. The CLI parser is clap (Derive API);
see [ADR 0022](docs/adr/0022-command-line-parsing-with-clap.md).

### Desktop app

```console
cargo run -p craftward-app --release
```

Opens a portfolio window over the same workspace file and `craftward.db`
as the CLI (command line / env / settings / platform defaults). On first
launch a short setup wizard chooses storage locations. The desktop app
edits through the same writers as the CLI, with undo toasts and
fingerprint conflicts. `--version` prints the name and version. See
[docs/desktop.md](docs/desktop.md),
[ADR 0023](docs/adr/0023-desktop-app-with-eframe.md),
[ADR 0024](docs/adr/0024-editing-from-the-desktop-app.md),
[ADR 0025](docs/adr/0025-native-dialogs-and-clipboard.md),
[ADR 0026](docs/adr/0026-navigation-and-visual-design.md), and
[ADR 0027](docs/adr/0027-settings-file-and-data-relocation.md).

`craftward validate` checks project files. With no paths, it checks every
project in the [workspace](docs/workspace-format.md). The process exits 0 when
every file is valid, 1 when a file is invalid or cannot be read, and 2 when
the command line is wrong. A missing source directory or lockfile is a
warning and does not change that status. `init`, `add`, `remove`, `list`,
`roadmap`, `workspace path`, `settings`, `show`, the `project`, `phase`,
`task`, `issue`, and `watch` edits, `stack`, `radar`, `deadlines`, and
`findings` are described in [docs/cli.md](docs/cli.md). An edit is written only when the project file
is still valid. `radar` asks [OSV](https://osv.dev/) about the packages in
the project's lockfiles and [endoflife.date](https://endoflife.date) about
components that have a version. It stores the findings in a local database.
It exits 3 when a vulnerability, a malicious package, a past support end,
or a past Google Play target-API deadline is open. Notices count with
`--strict`. `deadlines` prints the Play deadlines in use. See
[docs/radar.md](docs/radar.md), [docs/platform-deadlines.md](docs/platform-deadlines.md),
and [docs/storage.md](docs/storage.md). The endoflife.date data is under
the MIT License. Play deadlines are copied from the Play Console Help
page into a file in this repository.

```console
cargo run -p craftward-cli -- validate examples/minimal.craftward.json examples/full.craftward.json
```

Checks used by CI:

```console
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
cargo deny --locked check
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
- [Architecture decisions](docs/adr/README.md)
- [Project file format](docs/file-format.md)
- [Workspace file](docs/workspace-format.md)
- [Command-line interface](docs/cli.md)
- [Vulnerability radar](docs/radar.md)
- [Play target-API deadlines](docs/platform-deadlines.md)
- [Local database](docs/storage.md)
- [Desktop application](docs/desktop.md)
- [Roadmap](docs/roadmap/)
