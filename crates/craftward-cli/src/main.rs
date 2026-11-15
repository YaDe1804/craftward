//! Command-line interface for Craftward.

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use craftward_core::LoadError;

/// Process exit status for a usage error.
const USAGE_ERROR: u8 = 2;

/// Process exit status when a file is invalid or unreadable.
const DATA_ERROR: u8 = 1;

fn main() -> ExitCode {
    match parse_args() {
        Ok(Command::Version) => {
            println!("{} {}", env!("CARGO_BIN_NAME"), craftward_core::version());
            ExitCode::SUCCESS
        }
        Ok(Command::Help) => {
            print_help();
            ExitCode::SUCCESS
        }
        Ok(Command::Validate(paths)) => validate_files(&paths),
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::from(USAGE_ERROR)
        }
    }
}

enum Command {
    Version,
    Help,
    Validate(Vec<PathBuf>),
}

/// Accepts `--help`, `--version`, or `validate` with one or more file paths.
///
/// With no arguments, the program prints its name and version.
///
/// # Errors
///
/// Returns an error when the command line is not one of those forms.
fn parse_args() -> Result<Command, String> {
    let mut arguments = env::args_os().skip(1);
    let Some(argument) = arguments.next() else {
        return Ok(Command::Version);
    };
    if argument == "--version" {
        if arguments.next().is_some() {
            return Err("too many arguments".to_owned());
        }
        return Ok(Command::Version);
    }
    if argument == "--help" {
        if arguments.next().is_some() {
            return Err("too many arguments".to_owned());
        }
        return Ok(Command::Help);
    }
    if argument == "validate" {
        let mut paths = Vec::new();
        for argument in arguments {
            let rendered = argument.to_string_lossy();
            if rendered.starts_with('-') {
                return Err(format!("unrecognized argument: {rendered}"));
            }
            paths.push(PathBuf::from(argument));
        }
        if paths.is_empty() {
            return Err("validate requires at least one file path".to_owned());
        }
        return Ok(Command::Validate(paths));
    }
    let rendered = argument.to_string_lossy();
    Err(format!("unrecognized argument: {rendered}"))
}

fn print_help() {
    let version = craftward_core::version();
    println!(
        "\
craftward {version}

Check project files and print this program's version.

Usage:
  craftward
  craftward --help
  craftward --version
  craftward validate <PATH>...

Commands:
  validate <PATH>...
      Check one or more craftward.json files. Each file is reported as
      \"ok\" or as a list of errors. An error names its location, for
      example plan.phases[0].tasks[1].due.

With no arguments, craftward prints its name and version.

Exit status:
  0  every file is valid
  1  a file is invalid or cannot be read
  2  the command line is wrong"
    );
}

fn validate_files(paths: &[PathBuf]) -> ExitCode {
    let mut failed = false;
    for path in paths {
        let rendered = path.to_string_lossy();
        match craftward_core::load_from_path(path) {
            Ok(_) => println!("{rendered}: ok"),
            Err(error) => {
                failed = true;
                println!("{rendered}:");
                print_load_error(&error);
            }
        }
    }
    if failed {
        ExitCode::from(DATA_ERROR)
    } else {
        ExitCode::SUCCESS
    }
}

fn print_load_error(error: &LoadError) {
    match error {
        LoadError::Read(source) => println!("  failed to read file: {source}"),
        LoadError::Syntax {
            line,
            column,
            message,
        } => println!("  syntax error at line {line}, column {column}: {message}"),
        LoadError::UnsupportedSchema { .. } => {
            println!("  schema_version: file was written by a newer Craftward version");
        }
        LoadError::Validation(errors) => {
            for error in errors {
                println!("  {}: {}", error.path, error.message);
            }
        }
    }
}
