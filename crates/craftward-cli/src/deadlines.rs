//! `craftward deadlines`.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::SystemTime;

use craftward_core::{ProcessHost, load_deadlines, render_deadlines, unix_now, utc_day};

use crate::Failure;

/// Arguments of `deadlines`.
pub(crate) struct Request {
    /// Data directory from `--data-dir`.
    pub data_dir: Option<PathBuf>,
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
