# ADR 0025: Native dialogs (rfd) and clipboard

- Status: Accepted
- Date: 2026-11-16

## Decision

### Folder and file pickers

`craftward-app` uses [rfd](https://crates.io/crates/rfd) 0.18.0 for native
folder and file dialogs on the New project and Add existing forms. Default
features are off. Enabled features are `xdg-portal` and `wayland` so Linux
uses the XDG Desktop Portal instead of GTK. Windows and macOS use the
platform dialogs that rfd already selects without those features.

Dialogs run on a background thread. The chosen path is sent through an
`mpsc` channel into a `PickerInbox` that the UI drains each frame and then
requests a repaint. The text field remains editable. Tests inject a
`FakePicker` through the same `PathPicker` trait.

### Clipboard

eframe's `clipboard` feature is enabled. It turns on egui-winit → arboard so
Ctrl/Cmd+C, V, X, and A work in text fields and selected labels use the OS
clipboard on Windows, Linux (X11 and Wayland), and macOS.

### BSL-1.0 in `deny.toml`

On Windows, arboard depends on `clipboard-win` and `error-code`, both
**BSL-1.0** (Boost Software License 1.0). That identifier is listed under
`licenses.allow` in `deny.toml` (not as a per-crate exception). BSL-1.0 is
permissive, OSI-approved, and FSF-recognised as free; for binary
distribution it does not require a licence notice. That matches the other
permissive licences already allowed for Craftward.

## Package count and licences

rfd with `xdg-portal` + `wayland` added three packages to the lockfile
used when this ADR was written (403 → 406). Enabling `clipboard` adds the
arboard / smithay-clipboard stack; the lockfile measured after AP12b is 412
packages. rfd is MIT. `clipboard-win` and `error-code` are BSL-1.0 as above.

## Alternatives considered

GTK backends for rfd (`gtk3` / `gtk4`) were rejected to avoid linking GTK
and to keep CI's Linux dependency set unchanged. A pure typed-path UI
without a dialog was rejected because the hand test asked for a native
chooser while keeping the text field. Skipping OS clipboard was rejected
after BSL-1.0 was accepted into the allow list.

## Rationale

rfd matches the existing egui stack and keeps the UI responsive through a
threaded pick. The eframe `clipboard` feature is the supported path for
OS cut/copy/paste; allowing BSL-1.0 generally is preferable to a narrow
exception because the licence is already suitable for this project's
policy.
