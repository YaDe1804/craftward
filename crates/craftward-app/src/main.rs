//! Desktop application for Craftward.

use std::env;
use std::process::ExitCode;

/// Process exit status for a usage error.
const USAGE_ERROR: u8 = 2;

fn main() -> ExitCode {
    match parse_args() {
        Ok(()) => {
            println!("{} {}", env!("CARGO_BIN_NAME"), craftward_core::version());
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::from(USAGE_ERROR)
        }
    }
}

/// Accepts no arguments, or exactly `--version`.
///
/// # Errors
///
/// Returns an error when an argument is not recognized, or when more than one
/// argument is present.
fn parse_args() -> Result<(), String> {
    let mut arguments = env::args_os().skip(1);
    let Some(argument) = arguments.next() else {
        return Ok(());
    };
    if arguments.next().is_some() {
        return Err("too many arguments".to_owned());
    }
    if argument == "--version" {
        return Ok(());
    }
    let rendered = argument.to_string_lossy();
    Err(format!("unrecognized argument: {rendered}"))
}
