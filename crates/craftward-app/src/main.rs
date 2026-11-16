//! Desktop application for Craftward.

mod actions;
mod load;
mod picker;
mod symbols;
mod theme;
mod ui;
mod view;

use std::env;
use std::process::ExitCode;

use eframe::egui::ViewportBuilder;
use ui::{CraftwardApp, default_window_size, min_window_size};

/// Process exit status for a usage error.
const USAGE_ERROR: u8 = 2;

fn main() -> ExitCode {
    match parse_args() {
        Ok(Action::Version) => {
            println!("{} {}", env!("CARGO_BIN_NAME"), craftward_core::version());
            ExitCode::SUCCESS
        }
        Ok(Action::Run) => match run_app() {
            Ok(()) => ExitCode::SUCCESS,
            Err(message) => {
                eprintln!("error: {message}");
                ExitCode::from(1)
            }
        },
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::from(USAGE_ERROR)
        }
    }
}

enum Action {
    Version,
    Run,
}

/// Accepts no arguments, or exactly `--version`.
fn parse_args() -> Result<Action, String> {
    let mut arguments = env::args_os().skip(1);
    let Some(argument) = arguments.next() else {
        return Ok(Action::Run);
    };
    if arguments.next().is_some() {
        return Err("too many arguments".to_owned());
    }
    if argument == "--version" {
        return Ok(Action::Version);
    }
    let rendered = argument.to_string_lossy();
    Err(format!("unrecognized argument: {rendered}"))
}

fn run_app() -> Result<(), String> {
    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_title("Craftward")
            .with_inner_size(default_window_size())
            .with_min_inner_size(min_window_size()),
        centered: true,
        ..Default::default()
    };
    eframe::run_native(
        "Craftward",
        options,
        Box::new(|cc| Ok(Box::new(CraftwardApp::new(cc)))),
    )
    .map_err(|error| error.to_string())
}
