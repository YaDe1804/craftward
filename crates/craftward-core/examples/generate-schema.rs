//! Writes `schema/craftward.schema.json` from the current Rust types.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../schema/craftward.schema.json");
    if let Some(parent) = path.parent()
        && let Err(error) = fs::create_dir_all(parent)
    {
        eprintln!("error: failed to create {}: {error}", parent.display());
        return ExitCode::from(1);
    }
    let schema = craftward_core::project_schema_json();
    if let Err(error) = fs::write(&path, schema) {
        eprintln!("error: failed to write {}: {error}", path.display());
        return ExitCode::from(1);
    }
    println!("wrote {}", path.display());
    ExitCode::SUCCESS
}
