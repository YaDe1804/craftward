//! Domain logic shared by the Craftward tools.
//!
//! The command-line interface and the desktop application both depend on this
//! crate. Schema version 1 of a project file is loaded with [`load_from_str`]
//! and [`load_from_path`].

mod date;
mod error;
mod load;
mod model;
#[cfg(feature = "schema")]
mod schema;

pub use error::{LoadError, ValidationError};
pub use load::{load_from_path, load_from_str};
pub use model::{
    Component, ComponentKind, Issue, IssueSeverity, IssueStatus, Link, Phase, Plan, Project,
    ProjectStatus, Stack, Task, TaskStatus, WatchItem, WatchKind,
};
#[cfg(feature = "schema")]
pub use schema::project_schema_json;

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
