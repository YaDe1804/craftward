//! Domain logic shared by the Craftward tools.
//!
//! The command-line interface and the desktop application both depend on this
//! crate. Schema version 1 of a project file is loaded with [`load_from_str`]
//! and [`load_from_path`].

mod cache;
mod date;
mod edit;
mod error;
mod http;
mod i18n;
mod load;
mod location;
mod lockfile;
mod model;
mod osv;
mod paths;
mod radar;
mod roadmap;
#[cfg(feature = "schema")]
mod schema;
mod workspace;

pub use date::{format_unix_timestamp, unix_now, utc_date};
pub use edit::{
    EditError, EditOutcome, NewIssue, NewPhase, NewTask, NewWatch, ProjectChanges, ProjectUpdate,
    add_issue, add_phase, add_task, add_watch, edit_file, id_from_title, remove_issue,
    remove_phase, remove_task, remove_watch, render_project, set_issue_status, set_project,
    set_task_status,
};
pub use error::{LoadError, ValidationError};
pub use http::{HttpClient, HttpError, HttpRequest, HttpResponse, MAX_RESPONSE_BYTES};
pub use i18n::{LABELS, Label, Language, fill, text};
pub use load::{load_from_path, load_from_str};
pub use location::{
    Host, LocateError, OsFamily, ProcessHost, WorkspaceLocation, WorkspaceOrigin, locate_cache,
    locate_workspace,
};
pub use lockfile::{LockFailure, LockKind, PackageQuery, ParsedLock, lock_kind, parse_lock_text};
pub use model::{
    Component, ComponentKind, Issue, IssueSeverity, IssueStatus, Link, Phase, Plan, Project,
    ProjectStatus, Stack, Task, TaskStatus, WatchItem, WatchKind,
};
pub use osv::{Advisory, BATCH_LIMIT};
pub use paths::{
    MissingPath, MissingPathKind, ProjectPaths, is_portable_absolute, missing_paths,
    resolve_project_paths,
};
pub use radar::{
    Finding, LockfileReport, ProjectReport, RadarOptions, render_json, render_text, scan,
};
pub use roadmap::{
    PhaseState, RoadmapError, TaskProgress, escape_html, next_roadmap_version, phase_state,
    render_roadmap, task_progress, write_roadmap,
};
#[cfg(feature = "schema")]
pub use schema::project_schema_json;
pub use workspace::{
    ProjectRow, ProjectSnapshot, Workspace, WorkspaceEntry, WorkspaceError, add_project,
    init_project, list_projects, load_or_empty, load_workspace, project_file, project_file_by_id,
    remove_project, resolve_entry_path, save_workspace, snapshot,
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
