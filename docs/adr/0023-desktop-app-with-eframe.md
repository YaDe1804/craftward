# ADR 0023: Desktop app with eframe

- Status: Accepted
- Date: 2026-11-15

## Decision

`craftward-app` is a native desktop UI built with eframe 0.37.0 (egui).
Default features are off. Enabled features are `accesskit`, `default_fonts`,
`glow`, `wayland`, and `x11`. Persistence is not enabled. The app loads the
same workspace file, project files, and `craftward.db` as the CLI, using
`locate_workspace` / `locate_data`. Editing is introduced in ADR 0024; this
package still does not run radar or open the network.

### Renderer

glow (OpenGL via glutin) is preferred over wgpu. On this lockfile the glow
path adds far fewer crates than the wgpu stack, which pulls a large GPU
abstraction graph. Wayland and X11 stay available through eframe's
`wayland` and `x11` features. accesskit stays on so screen readers work on
each desktop platform.

### Package count and licenses

Adding eframe with the features above grows `Cargo.lock` from 80 to 403
packages. That increase is expected for a native GUI. Licenses stay within
`deny.toml`, with two narrow exceptions documented below.

### Layering inside `craftward-app`

1. **Load** (`load.rs`): calls `craftward-core` and `craftward-store` only.
2. **View model** (`view.rs`): pure functions that build overview and detail
   rows. No egui types.
3. **Surface** (`ui.rs`): egui code that renders the view model.

Theme colours and spacing live in `theme.rs`. Load failures and a missing
database are shown per project or as status text; the process does not
abort. Reload runs at startup and from a Reload button. There is no file
watcher in this package.

### `deny.toml` exceptions

Bundled egui fonts (`epaint_default_fonts`) use OFL-1.1 and Ubuntu-font-1.0
in addition to MIT OR Apache-2.0. Those two identifiers are allowed only
through `[[licenses.exceptions]]` for that crate. They are not added to
`licenses.allow`.

accesskit on Linux still depends on zbus 5, which pulls `zvariant`,
`zbus_names`, and `zvariant_derive`. RustSec marks those crates
unmaintained after they merged into zbus 6 (`RUSTSEC-2026-0370`,
`RUSTSEC-2026-0371`, `RUSTSEC-2026-0372`). They are listed under
`[advisories].ignore` with reasons. There is no vulnerability, and
`unmaintained = "all"` stays in force for every other crate. Remove the
three ignores when Dependabot (or a manual update) moves accesskit onto
zbus 6.

## Alternatives considered

wgpu was rejected for package size on a first desktop milestone. Skipping
accesskit would avoid the zbus advisories but would drop screen-reader
support required by this package. System fonts alone (`system_fonts`
without `default_fonts`) would avoid OFL/Ubuntu font licenses but would
drop the bundled egui fonts this package specifies.

## Rationale

eframe matches ADR 0003, keeps the UI in Rust, and reuses the existing
path and data model. glow plus a small feature set limits the first GUI
dependency spike while still covering Windows, Linux (X11 and Wayland),
and macOS.
