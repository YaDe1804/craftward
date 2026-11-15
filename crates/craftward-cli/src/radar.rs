//! `craftward radar`.

use std::path::PathBuf;
use std::process::ExitCode;

use craftward_core::{
    HttpClient, HttpError, HttpRequest, HttpResponse, ProcessHost, RadarOptions, list_projects,
    locate_cache, render_json, render_text, scan, unix_now,
};

use crate::{Failure, resolve_project};

/// Help text or a radar invocation.
pub(crate) enum Parsed {
    /// `radar --help`.
    Help(&'static str),
    /// A radar run.
    Run(Request),
}

/// Arguments of one radar run.
pub(crate) struct Request {
    /// Workspace ids or project paths. Empty means every workspace project.
    pub projects: Vec<String>,
    /// Ignore fresh cache entries.
    pub refresh: bool,
    /// Do not contact OSV.
    pub offline: bool,
    /// Print JSON instead of the text report.
    pub json: bool,
    /// Cache directory from `--cache-dir`.
    pub cache_dir: Option<PathBuf>,
}

pub(crate) fn parse(args: &[std::ffi::OsString]) -> Result<Parsed, String> {
    if args.iter().any(|arg| arg == "--help") {
        if args.len() == 1 {
            return Ok(Parsed::Help(HELP));
        }
        return Err("too many arguments".to_owned());
    }
    let mut projects = Vec::new();
    let mut refresh = false;
    let mut offline = false;
    let mut json = false;
    let mut cache_dir = None;
    let mut index = 0;
    while index < args.len() {
        let rendered = args[index].to_string_lossy();
        match rendered.as_ref() {
            "--refresh" => {
                if refresh {
                    return Err("duplicate option: --refresh".to_owned());
                }
                refresh = true;
            }
            "--offline" => {
                if offline {
                    return Err("duplicate option: --offline".to_owned());
                }
                offline = true;
            }
            "--json" => {
                if json {
                    return Err("duplicate option: --json".to_owned());
                }
                json = true;
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
            other if other.starts_with('-') => {
                return Err(format!("unrecognized argument: {other}"));
            }
            _ => projects.push(rendered.into_owned()),
        }
        index += 1;
    }
    if refresh && offline {
        return Err("use either --refresh or --offline".to_owned());
    }
    Ok(Parsed::Run(Request {
        projects,
        refresh,
        offline,
        json,
        cache_dir,
    }))
}

pub(crate) fn run(
    request: &Request,
    workspace: Option<&std::path::Path>,
) -> Result<ExitCode, Failure> {
    let cache_dir = locate_cache(&ProcessHost, request.cache_dir.as_deref())
        .map_err(|error| Failure::Data(error.to_string()))?;
    let files = project_files(&request.projects, workspace)?;
    let options = RadarOptions {
        refresh: request.refresh,
        offline: request.offline,
        now: unix_now(),
    };
    let reports = if request.offline {
        scan(&files, &OfflineClient, &cache_dir, &options)
    } else {
        let client =
            craftward_net::UreqClient::new(&format!("craftward/{}", craftward_core::version()))
                .map_err(Failure::Data)?;
        scan(&files, &client, &cache_dir, &options)
    };
    if request.json {
        println!("{}", render_json(&reports).map_err(Failure::Data)?);
    } else {
        print!("{}", render_text(&reports));
    }
    if reports.iter().any(|report| report.error.is_some()) {
        return Ok(ExitCode::from(1));
    }
    if reports.iter().any(|report| !report.findings.is_empty()) {
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
  craftward radar [PROJECT...] [--refresh] [--offline] [--json]
      [--cache-dir DIR]

With no PROJECT, every project in the workspace is checked. PROJECT is a
workspace id or a project path, with the same rules as roadmap. --refresh
ignores cached answers. --offline never contacts OSV. --json prints the
same report as JSON.

Exit status is 0 when nothing was found, 3 when a vulnerability was found,
1 when a project could not be checked, and 2 when the command line is wrong.
An error takes priority over a finding.

Example:
  craftward radar harbor-light
  craftward radar --offline --json";
