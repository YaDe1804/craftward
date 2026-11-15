//! `craftward findings`.

use std::path::PathBuf;
use std::process::ExitCode;

use craftward_core::{
    DATABASE_FILE, ProcessHost, StateFilter, ignore_finding, is_calendar_date, list_findings,
    load_from_path, locate_data, render_finding_list, render_finding_show, render_findings_json,
    render_show_json, reopen_finding, show_finding, unix_now,
};
use craftward_store::SqliteStore;

use crate::{Failure, resolve_project};

/// One findings command.
pub(crate) enum Request {
    /// List stored findings.
    List(ListRequest),
    /// Print one finding and its history.
    Show(IdRequest),
    /// Ignore a finding.
    Ignore(IgnoreRequest),
    /// Mark a finding open again.
    Reopen(IdRequest),
}

/// Arguments of `findings` with no subcommand.
pub(crate) struct ListRequest {
    /// Project ids or paths. Empty means every stored finding.
    pub projects: Vec<String>,
    /// Which states to print. The default is open.
    pub state: StateFilter,
    /// Which kind to print. Absent means every kind.
    pub kind: Option<String>,
    /// Print JSON.
    pub json: bool,
    /// Data directory from `--data-dir`.
    pub data_dir: Option<PathBuf>,
}

/// Arguments that only need a finding id.
pub(crate) struct IdRequest {
    /// Database id.
    pub id: i64,
    /// Print JSON. Ignored by reopen.
    pub json: bool,
    /// Data directory from `--data-dir`.
    pub data_dir: Option<PathBuf>,
}

/// Arguments of `findings ignore`.
pub(crate) struct IgnoreRequest {
    /// Database id.
    pub id: i64,
    /// Required reason.
    pub reason: String,
    /// Last day the ignore stays in force.
    pub until: Option<String>,
    /// Data directory from `--data-dir`.
    pub data_dir: Option<PathBuf>,
}

pub(crate) fn run(
    request: &Request,
    workspace: Option<&std::path::Path>,
) -> Result<ExitCode, Failure> {
    let data_dir = locate_data(&ProcessHost, data_dir_of(request))
        .map_err(|error| Failure::Data(error.to_string()))?;
    let mut store = SqliteStore::open(&data_dir.join(DATABASE_FILE))
        .map_err(|error| Failure::Data(error.to_string()))?;
    match request {
        Request::List(request) => list(request, &mut store, workspace),
        Request::Show(request) => show(request, &mut store),
        Request::Ignore(request) => ignore(request, &mut store),
        Request::Reopen(request) => reopen(request, &mut store),
    }
}

fn list(
    request: &ListRequest,
    store: &mut SqliteStore,
    workspace: Option<&std::path::Path>,
) -> Result<ExitCode, Failure> {
    let mut projects = Vec::new();
    for project in &request.projects {
        let path = resolve_project(project, workspace)?;
        let loaded = load_from_path(&path).map_err(|error| Failure::Data(error.to_string()))?;
        projects.push(loaded.id);
    }
    let findings = list_findings(store, &projects, request.state, request.kind.as_deref())
        .map_err(|error| Failure::Data(error.to_string()))?;
    if request.json {
        println!(
            "{}",
            render_findings_json(&findings).map_err(Failure::Data)?
        );
    } else {
        print!("{}", render_finding_list(&findings));
    }
    Ok(ExitCode::SUCCESS)
}

fn show(request: &IdRequest, store: &mut SqliteStore) -> Result<ExitCode, Failure> {
    let (finding, events) =
        show_finding(store, request.id).map_err(|error| Failure::Data(error.to_string()))?;
    if request.json {
        println!(
            "{}",
            render_show_json(&finding, &events).map_err(Failure::Data)?
        );
    } else {
        print!("{}", render_finding_show(&finding, &events));
    }
    Ok(ExitCode::SUCCESS)
}

fn ignore(request: &IgnoreRequest, store: &mut SqliteStore) -> Result<ExitCode, Failure> {
    if let Some(until) = &request.until
        && !is_calendar_date(until)
    {
        return Err(Failure::Usage("--until must be YYYY-MM-DD".to_owned()));
    }
    ignore_finding(
        store,
        request.id,
        &request.reason,
        request.until.as_deref(),
        unix_now(),
    )
    .map_err(|error| Failure::Data(error.to_string()))?;
    println!("ignored finding {}", request.id);
    Ok(ExitCode::SUCCESS)
}

fn reopen(request: &IdRequest, store: &mut SqliteStore) -> Result<ExitCode, Failure> {
    reopen_finding(store, request.id, unix_now())
        .map_err(|error| Failure::Data(error.to_string()))?;
    println!("reopened finding {}", request.id);
    Ok(ExitCode::SUCCESS)
}

fn data_dir_of(request: &Request) -> Option<&std::path::Path> {
    match request {
        Request::List(request) => request.data_dir.as_deref(),
        Request::Show(request) | Request::Reopen(request) => request.data_dir.as_deref(),
        Request::Ignore(request) => request.data_dir.as_deref(),
    }
}
