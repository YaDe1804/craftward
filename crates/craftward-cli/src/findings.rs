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

/// Help text or a findings invocation.
pub(crate) enum Parsed {
    /// `findings --help`.
    Help(&'static str),
    /// A findings command.
    Run(Request),
}

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

pub(crate) fn parse(args: &[std::ffi::OsString]) -> Result<Parsed, String> {
    if args.is_empty() {
        return Ok(Parsed::Run(Request::List(ListRequest {
            projects: Vec::new(),
            state: StateFilter::Open,
            kind: None,
            json: false,
            data_dir: None,
        })));
    }
    if args.iter().any(|arg| arg == "--help") {
        let words = match args.first().map(|arg| arg.to_string_lossy()) {
            Some(text) if text == "show" || text == "ignore" || text == "reopen" => 2,
            _ => 1,
        };
        if args.len() == words {
            return Ok(Parsed::Help(HELP));
        }
        return Err("too many arguments".to_owned());
    }
    match args[0].to_string_lossy().as_ref() {
        "show" => parse_show(&args[1..]).map(|request| Parsed::Run(Request::Show(request))),
        "ignore" => parse_ignore(&args[1..]).map(|request| Parsed::Run(Request::Ignore(request))),
        "reopen" => parse_reopen(&args[1..]).map(|request| Parsed::Run(Request::Reopen(request))),
        _ => parse_list(args).map(|request| Parsed::Run(Request::List(request))),
    }
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

fn parse_list(args: &[std::ffi::OsString]) -> Result<ListRequest, String> {
    let mut projects = Vec::new();
    let mut state = StateFilter::Open;
    let mut kind = None;
    let mut json = false;
    let mut data_dir = None;
    let mut index = 0;
    while index < args.len() {
        let rendered = args[index].to_string_lossy();
        match rendered.as_ref() {
            "--json" => flag_once(&mut json, "--json")?,
            "--state" => {
                let value = required_value(args, &mut index, "--state")?;
                state = parse_state(&value)?;
            }
            "--kind" => {
                let value = required_value(args, &mut index, "--kind")?;
                if kind.is_some() {
                    return Err("duplicate option: --kind".to_owned());
                }
                kind = Some(parse_kind(&value)?);
            }
            "--data-dir" => data_dir = Some(directory(args, &mut index, &mut data_dir)?),
            other if other.starts_with('-') => {
                return Err(format!("unrecognized argument: {other}"));
            }
            _ => projects.push(rendered.into_owned()),
        }
        index += 1;
    }
    Ok(ListRequest {
        projects,
        state,
        kind,
        json,
        data_dir,
    })
}

fn parse_show(args: &[std::ffi::OsString]) -> Result<IdRequest, String> {
    let (id, json, data_dir) = parse_id_command(args, "show")?;
    Ok(IdRequest { id, json, data_dir })
}

fn parse_reopen(args: &[std::ffi::OsString]) -> Result<IdRequest, String> {
    let (id, _, data_dir) = parse_id_command(args, "reopen")?;
    Ok(IdRequest {
        id,
        json: false,
        data_dir,
    })
}

fn parse_ignore(args: &[std::ffi::OsString]) -> Result<IgnoreRequest, String> {
    let mut id = None;
    let mut reason = None;
    let mut until = None;
    let mut data_dir = None;
    let mut index = 0;
    while index < args.len() {
        let rendered = args[index].to_string_lossy();
        match rendered.as_ref() {
            "--reason" => reason = Some(required_value(args, &mut index, "--reason")?),
            "--until" => {
                let value = required_value(args, &mut index, "--until")?;
                if !is_calendar_date(&value) {
                    return Err("--until must be YYYY-MM-DD".to_owned());
                }
                until = Some(value);
            }
            "--data-dir" => data_dir = Some(directory(args, &mut index, &mut data_dir)?),
            other if other.starts_with('-') => {
                return Err(format!("unrecognized argument: {other}"));
            }
            _ if id.is_some() => return Err("too many arguments".to_owned()),
            _ => id = Some(finding_id(&rendered)?),
        }
        index += 1;
    }
    let Some(id) = id else {
        return Err("findings ignore needs a finding id".to_owned());
    };
    let Some(reason) = reason else {
        return Err("--reason needs a value".to_owned());
    };
    Ok(IgnoreRequest {
        id,
        reason,
        until,
        data_dir,
    })
}

fn parse_id_command(
    args: &[std::ffi::OsString],
    name: &str,
) -> Result<(i64, bool, Option<PathBuf>), String> {
    let mut id = None;
    let mut json = false;
    let mut data_dir = None;
    let mut index = 0;
    while index < args.len() {
        let rendered = args[index].to_string_lossy();
        match rendered.as_ref() {
            "--json" => flag_once(&mut json, "--json")?,
            "--data-dir" => data_dir = Some(directory(args, &mut index, &mut data_dir)?),
            other if other.starts_with('-') => {
                return Err(format!("unrecognized argument: {other}"));
            }
            _ if id.is_some() => return Err("too many arguments".to_owned()),
            _ => id = Some(finding_id(&rendered)?),
        }
        index += 1;
    }
    let Some(id) = id else {
        return Err(format!("findings {name} needs a finding id"));
    };
    Ok((id, json, data_dir))
}

fn parse_kind(value: &str) -> Result<String, String> {
    match value {
        "vulnerability" | "malicious" | "unmaintained" | "unsound" | "notice" | "eol"
        | "eol_soon" => Ok(value.to_owned()),
        _ => Err(
            "--kind must be vulnerability, malicious, unmaintained, unsound, notice, eol, or eol_soon"
                .to_owned(),
        ),
    }
}

fn parse_state(value: &str) -> Result<StateFilter, String> {
    match value {
        "open" => Ok(StateFilter::Open),
        "ignored" => Ok(StateFilter::Ignored),
        "resolved" => Ok(StateFilter::Resolved),
        "all" => Ok(StateFilter::All),
        _ => Err("--state must be open, ignored, resolved, or all".to_owned()),
    }
}

fn finding_id(text: &str) -> Result<i64, String> {
    let id = text
        .parse::<i64>()
        .map_err(|_| "finding id must be a positive number".to_owned())?;
    if id <= 0 {
        return Err("finding id must be a positive number".to_owned());
    }
    Ok(id)
}

fn flag_once(slot: &mut bool, name: &str) -> Result<(), String> {
    if *slot {
        return Err(format!("duplicate option: {name}"));
    }
    *slot = true;
    Ok(())
}

fn required_value(
    args: &[std::ffi::OsString],
    index: &mut usize,
    name: &str,
) -> Result<String, String> {
    let Some(value) = args.get(*index + 1) else {
        return Err(format!("{name} needs a value"));
    };
    let text = value.to_string_lossy();
    if text.starts_with('-') {
        return Err(format!("{name} needs a value"));
    }
    *index += 1;
    Ok(text.into_owned())
}

fn directory(
    args: &[std::ffi::OsString],
    index: &mut usize,
    current: &mut Option<PathBuf>,
) -> Result<PathBuf, String> {
    if current.is_some() {
        return Err("duplicate option: --data-dir".to_owned());
    }
    Ok(PathBuf::from(required_value(args, index, "--data-dir")?))
}

const HELP: &str = "\
List and change stored vulnerability findings.

Usage:
  craftward findings [PROJECT...] [--state open|ignored|resolved|all]
      [--kind KIND] [--json] [--data-dir DIR]
  craftward findings show <ID> [--json] [--data-dir DIR]
  craftward findings ignore <ID> --reason TEXT [--until YYYY-MM-DD]
      [--data-dir DIR]
  craftward findings reopen <ID> [--data-dir DIR]

With no PROJECT, every stored finding is listed. The default state is open.
show prints one finding and its history. ignore requires a reason. until is
the last UTC day the ignore stays in force.

Exit status is 0 on success, 1 when the finding id is unknown or the reason
is empty, and 2 when the command line is wrong.

Example:
  craftward findings
  craftward findings ignore 1 --reason \"not affected\" --until 2026-12-31";
