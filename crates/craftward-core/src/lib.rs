//! Domain logic shared by the Craftward tools.
//!
//! The command-line interface and the desktop application both depend on this
//! crate. Schema version 1 of a project file is loaded with [`load_from_str`]
//! and [`load_from_path`].

mod date;
mod error;
mod load;
mod location;
mod model;
#[cfg(feature = "schema")]
mod schema;
mod workspace;

pub use error::{LoadError, ValidationError};
pub use load::{load_from_path, load_from_str};
pub use location::{
    Host, LocateError, OsFamily, ProcessHost, WorkspaceLocation, WorkspaceOrigin, locate_workspace,
};
pub use model::{
    Component, ComponentKind, Issue, IssueSeverity, IssueStatus, Link, Phase, Plan, Project,
    ProjectStatus, Stack, Task, TaskStatus, WatchItem, WatchKind,
};
#[cfg(feature = "schema")]
pub use schema::project_schema_json;
pub use workspace::{
    ProjectRow, ProjectSnapshot, Workspace, WorkspaceEntry, WorkspaceError, add_project,
    init_project, list_projects, load_or_empty, load_workspace, remove_project, resolve_entry_path,
    save_workspace, snapshot,
};

/// Returns the version of the Craftward core library.
#[must_use]
pub const fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::version;

    #[test]
    fn version_matches_package_metadata() {
        let value = version();
        assert_ne!(value, "");
        assert_eq!(value, env!("CARGO_PKG_VERSION"));
    }
}
