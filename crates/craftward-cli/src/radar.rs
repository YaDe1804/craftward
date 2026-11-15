//! `craftward radar`.

use std::path::PathBuf;
use std::process::ExitCode;

use craftward_core::{
    DATABASE_FILE, FindingKind, HttpClient, HttpError, HttpRequest, HttpResponse, ProcessHost,
    RadarOptions, apply_reports, list_projects, locate_cache, locate_data, render_ledger_json,
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
    /// Count notices and `eol_soon` toward exit status 3.
    pub strict: bool,
    /// Days ahead that still count as `eol_soon`.
    pub eol_warning_days: u32,
    /// Days ahead that still count as `platform_deadline_soon`.
    pub deadline_warning_days: u32,
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
    let mut strict = false;
    let mut eol_warning_days = None;
    let mut deadline_warning_days = None;
    let mut index = 0;
    while index < args.len() {
        let rendered = args[index].to_string_lossy();
        match rendered.as_ref() {
            "--refresh" => mode = exclusive_mode(mode, ScanMode::Refresh, "--refresh")?,
            "--offline" => mode = exclusive_mode(mode, ScanMode::Offline, "--offline")?,
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
                cache_dir = Some(one_directory(
                    args,
                    &mut index,
                    cache_dir.is_some(),
                    "--cache-dir",
                )?);
            }
            "--data-dir" => {
                data_dir = Some(one_directory(
                    args,
                    &mut index,
                    data_dir.is_some(),
                    "--data-dir",
                )?);
            }
            "--strict" => {
                if strict {
                    return Err("duplicate option: --strict".to_owned());
                }
                strict = true;
            }
            "--eol-warning-days" => {
                eol_warning_days = Some(warning_days(
                    args,
                    &mut index,
                    "--eol-warning-days",
                    eol_warning_days.is_some(),
                )?);
            }
            "--deadline-warning-days" => {
                deadline_warning_days = Some(warning_days(
                    args,
                    &mut index,
                    "--deadline-warning-days",
                    deadline_warning_days.is_some(),
                )?);
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
        strict,
        eol_warning_days: eol_warning_days.unwrap_or(90),
        deadline_warning_days: deadline_warning_days.unwrap_or(180),
    }))
}

fn exclusive_mode(current: ScanMode, next: ScanMode, name: &str) -> Result<ScanMode, String> {
    match (current, next) {
        (ScanMode::Normal, chosen) => Ok(chosen),
        (ScanMode::Refresh, ScanMode::Refresh) | (ScanMode::Offline, ScanMode::Offline) => {
            Err(format!("duplicate option: {name}"))
        }
        (ScanMode::Refresh | ScanMode::Offline, _) => {
            Err("use either --refresh or --offline".to_owned())
        }
    }
}

fn one_directory(
    args: &[std::ffi::OsString],
    index: &mut usize,
    present: bool,
    name: &str,
) -> Result<PathBuf, String> {
    if present {
        return Err(format!("duplicate option: {name}"));
    }
    let Some(value) = args.get(*index + 1) else {
        return Err(format!("{name} needs a directory"));
    };
    if value.to_string_lossy().starts_with('-') {
        return Err(format!("{name} needs a directory"));
    }
    *index += 1;
    Ok(PathBuf::from(value))
}

fn warning_days(
    args: &[std::ffi::OsString],
    index: &mut usize,
    name: &str,
    present: bool,
) -> Result<u32, String> {
    if present {
        return Err(format!("duplicate option: {name}"));
    }
    let Some(value) = args.get(*index + 1) else {
        return Err(format!("{name} needs a number"));
    };
    let text = value.to_string_lossy();
    if text.starts_with('-') {
        return Err(format!("{name} needs a number"));
    }
    let days = text
        .parse::<u32>()
        .map_err(|_| format!("{name} needs a non-negative number"))?;
    *index += 1;
    Ok(days)
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
            .any(|finding| match FindingKind::parse(&finding.kind) {
                Some(kind) => kind.affects_exit(request.strict),
                None => request.strict,
            })
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

const HELP: &str = "\
Look up known vulnerabilities for the packages in project lockfiles.

Usage:
  craftward radar [PROJECT...] [--refresh] [--offline] [--json] [--all]
      [--strict] [--eol-warning-days N] [--deadline-warning-days N]
      [--cache-dir DIR] [--data-dir DIR]

With no PROJECT, every project in the workspace is checked. PROJECT is a
workspace id or a project path, with the same rules as roadmap. --refresh
ignores cached answers. --offline never contacts OSV or endoflife.date.
--all includes ignored findings in the list. --json prints the same report
as JSON. --eol-warning-days sets how far ahead a support end counts as
eol_soon. The default is 90. --deadline-warning-days sets how far ahead a
Play target-API deadline counts as platform_deadline_soon. The default is
180. Findings are stored in the data directory. Play deadlines need no
network.

Exit status is 0 when no actionable finding is open, 3 when a vulnerability,
malicious package, past support end, or past Play deadline is open, 1 when a
project could not be checked, and 2 when the command line is wrong. Notices,
eol_soon, and platform_deadline_soon count toward status 3 only with
--strict. An error takes priority over an open finding. Ignored findings do
not change the status.

Example:
  craftward radar harbor-light
  craftward radar --offline --json";
