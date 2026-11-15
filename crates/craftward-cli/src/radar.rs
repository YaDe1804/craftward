//! `craftward radar`.

use std::path::PathBuf;
use std::process::ExitCode;

use craftward_core::{
    DATABASE_FILE, HttpClient, HttpError, HttpRequest, HttpResponse, ProcessHost, RadarOptions,
    apply_reports, list_projects, locate_cache, locate_data, render_ledger_json,
    render_ledger_text, scan, unix_now,
};
use craftward_store::SqliteStore;

use crate::{Failure, resolve_project};

/// Whether a scan may use the network.
#[derive(Clone, Copy)]
pub(crate) enum ScanMode {
    /// Use the cache, then the network.
    Normal,
    /// Ignore fresh cache entries and ask again.
    Refresh,
    /// Never contact OSV.
    Offline,
}

/// Arguments of one radar run.
pub(crate) struct Request {
    /// Workspace ids or project paths. Empty means every workspace project.
    pub projects: Vec<String>,
    /// Whether the scan may use the network.
    pub mode: ScanMode,
    /// Print JSON instead of the text report.
    pub json: bool,
    /// Include ignored findings in the list.
    pub show_all: bool,
    /// Cache directory from `--cache-dir`.
    pub cache_dir: Option<PathBuf>,
    /// Data directory from `--data-dir`.
    pub data_dir: Option<PathBuf>,
    /// Count notices and `eol_soon` toward exit status 3.
    pub strict: bool,
    /// Days ahead that still count as `eol_soon`.
    pub eol_warning_days: u32,
    /// Days ahead that still count as `platform_deadline_soon`.
    pub deadline_warning_days: u32,
}

pub(crate) fn run(
    request: &Request,
    workspace: Option<&std::path::Path>,
) -> Result<ExitCode, Failure> {
    let cache_dir = locate_cache(&ProcessHost, request.cache_dir.as_deref())
        .map_err(|error| Failure::Data(error.to_string()))?;
    let data_dir = locate_data(&ProcessHost, request.data_dir.as_deref())
        .map_err(|error| Failure::Data(error.to_string()))?;
    let files = project_files(&request.projects, workspace)?;
    let started = unix_now();
    let refresh = matches!(request.mode, ScanMode::Refresh);
    let offline = matches!(request.mode, ScanMode::Offline);
    let options = RadarOptions {
        refresh,
        offline,
        now: started,
        eol_warning_days: request.eol_warning_days,
        deadline_warning_days: request.deadline_warning_days,
    };
    let reports = if offline {
        scan(&files, &OfflineClient, &cache_dir, &data_dir, &options)
    } else {
        let client =
            craftward_net::UreqClient::new(&format!("craftward/{}", craftward_core::version()))
                .map_err(Failure::Data)?;
        scan(&files, &client, &cache_dir, &data_dir, &options)
    };
    let mut store = SqliteStore::open(&data_dir.join(DATABASE_FILE))
        .map_err(|error| Failure::Data(error.to_string()))?;
    let ledgers = apply_reports(&mut store, &reports, started, unix_now(), offline)
        .map_err(|error| Failure::Data(error.to_string()))?;
    if request.json {
        println!(
            "{}",
            render_ledger_json(&ledgers, request.show_all).map_err(Failure::Data)?
        );
    } else {
        print!("{}", render_ledger_text(&ledgers, request.show_all));
    }
    if ledgers.iter().any(|ledger| ledger.error.is_some()) {
        return Ok(ExitCode::from(1));
    }
    if ledgers.iter().any(|ledger| {
        ledger
            .open
            .iter()
            .any(|finding| finding.kind.affects_exit(request.strict))
    }) {
        return Ok(ExitCode::from(3));
    }
    Ok(ExitCode::SUCCESS)
}

fn project_files(
    projects: &[String],
    workspace: Option<&std::path::Path>,
) -> Result<Vec<PathBuf>, Failure> {
    if projects.is_empty() {
        let file = crate::workspace_file(workspace)?;
        return list_projects(&file)
            .map(|rows| rows.into_iter().map(|row| row.path).collect())
            .map_err(|error| Failure::Data(error.to_string()));
    }
    let mut files = Vec::new();
    for project in projects {
        files.push(resolve_project(project, workspace)?);
    }
    Ok(files)
}

struct OfflineClient;

impl HttpClient for OfflineClient {
    fn execute(&self, _request: &HttpRequest<'_>) -> Result<HttpResponse, HttpError> {
        Err(HttpError {
            message: "offline mode does not use the network".to_owned(),
        })
    }
}
