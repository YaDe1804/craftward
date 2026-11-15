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

/// Help text or a radar invocation.
pub(crate) enum Parsed {
    /// `radar --help`.
    Help(&'static str),
    /// A radar run.
    Run(Request),
}

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
}

pub(crate) fn parse(args: &[std::ffi::OsString]) -> Result<Parsed, String> {
    if args.iter().any(|arg| arg == "--help") {
        if args.len() == 1 {
            return Ok(Parsed::Help(HELP));
        }
        return Err("too many arguments".to_owned());
    }
    let mut projects = Vec::new();
    let mut mode = ScanMode::Normal;
    let mut json = false;
    let mut show_all = false;
    let mut cache_dir = None;
    let mut data_dir = None;
    let mut index = 0;
    while index < args.len() {
        let rendered = args[index].to_string_lossy();
        match rendered.as_ref() {
            "--refresh" => {
                mode = match mode {
                    ScanMode::Normal => ScanMode::Refresh,
                    ScanMode::Refresh => {
                        return Err("duplicate option: --refresh".to_owned());
                    }
                    ScanMode::Offline => {
                        return Err("use either --refresh or --offline".to_owned());
                    }
                };
            }
            "--offline" => {
                mode = match mode {
                    ScanMode::Normal => ScanMode::Offline,
                    ScanMode::Offline => {
                        return Err("duplicate option: --offline".to_owned());
                    }
                    ScanMode::Refresh => {
                        return Err("use either --refresh or --offline".to_owned());
                    }
                };
            }
            "--json" => {
                if json {
                    return Err("duplicate option: --json".to_owned());
                }
                json = true;
            }
            "--all" => {
                if show_all {
                    return Err("duplicate option: --all".to_owned());
                }
                show_all = true;
            }
            "--cache-dir" => {
                let Some(value) = args.get(index + 1) else {
                    return Err("--cache-dir needs a directory".to_owned());
                };
                if value.to_string_lossy().starts_with('-') {
                    return Err("--cache-dir needs a directory".to_owned());
                }
                if cache_dir.is_some() {
                    return Err("duplicate option: --cache-dir".to_owned());
                }
                cache_dir = Some(PathBuf::from(value));
                index += 1;
            }
            "--data-dir" => {
                let Some(value) = args.get(index + 1) else {
                    return Err("--data-dir needs a directory".to_owned());
                };
                if value.to_string_lossy().starts_with('-') {
                    return Err("--data-dir needs a directory".to_owned());
                }
                if data_dir.is_some() {
                    return Err("duplicate option: --data-dir".to_owned());
                }
                data_dir = Some(PathBuf::from(value));
                index += 1;
            }
            other if other.starts_with('-') => {
                return Err(format!("unrecognized argument: {other}"));
            }
            _ => projects.push(rendered.into_owned()),
        }
        index += 1;
    }
    Ok(Parsed::Run(Request {
        projects,
        mode,
        json,
        show_all,
        cache_dir,
        data_dir,
    }))
}

pub(crate) fn run(
    request: &Request,
    workspace: Option<&std::path::Path>,
) -> Result<ExitCode, Failure> {
    let cache_dir = locate_cache(&ProcessHost, request.cache_dir.as_deref())
        .map_err(|error| Failure::Data(error.to_string()))?;
    let files = project_files(&request.projects, workspace)?;
    let started = unix_now();
    let refresh = matches!(request.mode, ScanMode::Refresh);
    let offline = matches!(request.mode, ScanMode::Offline);
    let options = RadarOptions {
        refresh,
        offline,
        now: started,
    };
    let reports = if offline {
        scan(&files, &OfflineClient, &cache_dir, &options)
    } else {
        let client =
            craftward_net::UreqClient::new(&format!("craftward/{}", craftward_core::version()))
                .map_err(Failure::Data)?;
        scan(&files, &client, &cache_dir, &options)
    };
    let data_dir = locate_data(&ProcessHost, request.data_dir.as_deref())
        .map_err(|error| Failure::Data(error.to_string()))?;
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
    if ledgers.iter().any(|ledger| !ledger.open.is_empty()) {
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

const HELP: &str = "\
Look up known vulnerabilities for the packages in project lockfiles.

Usage:
  craftward radar [PROJECT...] [--refresh] [--offline] [--json] [--all]
      [--cache-dir DIR] [--data-dir DIR]

With no PROJECT, every project in the workspace is checked. PROJECT is a
workspace id or a project path, with the same rules as roadmap. --refresh
ignores cached answers. --offline never contacts OSV. --all includes
ignored findings in the list. --json prints the same report as JSON.
Findings are stored in the data directory.

Exit status is 0 when no finding is open, 3 when a finding is open,
1 when a project could not be checked, and 2 when the command line is wrong.
An error takes priority over an open finding. Ignored findings do not
change the status.

Example:
  craftward radar harbor-light
  craftward radar --offline --json";
