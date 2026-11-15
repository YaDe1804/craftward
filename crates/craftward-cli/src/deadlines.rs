//! `craftward deadlines`.

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::SystemTime;

use craftward_core::{ProcessHost, load_deadlines, render_deadlines, unix_now, utc_day};

use crate::Failure;

/// Help text or a deadlines invocation.
pub(crate) enum Parsed {
    /// `deadlines --help`.
    Help(&'static str),
    /// Print the deadlines in use.
    Run(Request),
}

/// Arguments of `deadlines`.
pub(crate) struct Request {
    /// Data directory from `--data-dir`.
    pub data_dir: Option<PathBuf>,
}

pub(crate) fn parse(args: &[OsString]) -> Result<Parsed, String> {
    if args.iter().any(|arg| arg == "--help") {
        if args.len() == 1 {
            return Ok(Parsed::Help(HELP));
        }
        return Err("too many arguments".to_owned());
    }
    let mut data_dir = None;
    let mut index = 0;
    while index < args.len() {
        let rendered = args[index].to_string_lossy();
        match rendered.as_ref() {
            "--data-dir" => {
                if data_dir.is_some() {
                    return Err("duplicate option: --data-dir".to_owned());
                }
                let Some(value) = args.get(index + 1) else {
                    return Err("--data-dir needs a directory".to_owned());
                };
                if value.to_string_lossy().starts_with('-') {
                    return Err("--data-dir needs a directory".to_owned());
                }
                data_dir = Some(PathBuf::from(value));
                index += 1;
            }
            other => return Err(format!("unrecognized argument: {other}")),
        }
        index += 1;
    }
    Ok(Parsed::Run(Request { data_dir }))
}

pub(crate) fn run(request: &Request) -> Result<ExitCode, Failure> {
    let data_dir = craftward_core::locate_data(&ProcessHost, request.data_dir.as_deref())
        .map_err(|error| Failure::Data(error.to_string()))?;
    let catalog = load_deadlines(&data_dir).map_err(Failure::Data)?;
    let today = utc_day(unix_now()).unwrap_or_else(|| {
        craftward_core::utc_date(SystemTime::now()).unwrap_or_else(|| "1970-01-01".to_owned())
    });
    print!("{}", render_deadlines(&catalog, &today));
    Ok(ExitCode::SUCCESS)
}

const HELP: &str = "\
Show the Google Play target-API deadlines Craftward will use.

Usage:
  craftward deadlines [--data-dir DIR]

The built-in file is used unless DIR contains platform-deadlines.json with
a newer data_version. An older or invalid file in DIR is reported and is
not used. Exit status is 0 on success, 1 when the built-in file or the data
directory cannot be used, and 2 when the command line is wrong.

Example:
  craftward deadlines
  craftward deadlines --data-dir ./data";
