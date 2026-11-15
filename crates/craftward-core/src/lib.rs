//! Domain logic shared by the Craftward tools.
//!
//! The command-line interface and the desktop application both depend on this
//! crate. Schema version 1 of a project file is loaded with [`load_from_str`]
//! and [`load_from_path`].

mod cache;
mod date;
mod deadlines;
mod edit;
mod eol;
mod error;
mod http;
mod i18n;
mod lifecycle;
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
mod store;
mod target_sdk;
mod workspace;

pub use date::{
    days_between, format_iso8601, format_unix_timestamp, is_calendar_date, unix_now, utc_date,
    utc_day,
};
pub use deadlines::{
    Catalog, CatalogOrigin, Deadline, DeadlineHit, PLAY_HELP, Parsed, compare,
    load as load_deadlines, page_url, parse_document, render as render_deadlines, stale_warning,
};
pub use edit::{
    AndroidChanges, ComponentChanges, EditError, EditOutcome, NewComponent, NewIssue, NewPhase,
    NewTask, NewWatch, ProjectChanges, ProjectUpdate, add_component, add_issue, add_lockfile,
    add_phase, add_task, add_watch, edit_file, id_from_title, remove_component, remove_issue,
    remove_lockfile, remove_phase, remove_task, remove_watch, render_project, set_android,
    set_component, set_issue_status, set_project, set_task_status,
};
pub use eol::{
    CatalogProduct, Cycle, EolHit, EolOptions, EolReport, check_components, normalize_name,
};
pub use error::{LoadError, ValidationError};
pub use http::{HttpClient, HttpError, HttpRequest, HttpResponse, MAX_RESPONSE_BYTES};
pub use i18n::{LABELS, Label, Language, fill, text};
pub use lifecycle::{
    LedgerFinding, ProjectLedger, apply_reports, ignore_finding, list_findings,
    render_finding_list, render_finding_show, render_findings_json, render_ledger_json,
    render_ledger_text, render_show_json, reopen_finding, show_finding,
};
pub use load::{load_from_path, load_from_str};
pub use location::{
    Host, LocateError, OsFamily, ProcessHost, WorkspaceLocation, WorkspaceOrigin, locate_cache,
    locate_data, locate_workspace,
};
pub use lockfile::{LockFailure, LockKind, PackageQuery, ParsedLock, lock_kind, parse_lock_text};
pub use model::{
    AndroidConfig, AndroidStore, Component, ComponentKind, FormFactor, Issue, IssueSeverity,
    IssueStatus, Link, Phase, Plan, Project, ProjectStatus, Stack, Task, TaskStatus, WatchItem,
    WatchKind,
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
pub use store::{
    DATABASE_FILE, FindingKind, FindingState, NewFinding, NewRun, SCHEMA_VERSION, StateFilter,
    StateStore, StoreError, StoreSession, StoredEvent, StoredFinding,
};
pub use target_sdk::{Detection, detect as detect_target_sdk};
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
