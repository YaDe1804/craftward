//! `craftward stack`.

use std::ffi::OsString;
use std::process::ExitCode;
use std::time::SystemTime;

use craftward_core::{
    ComponentChanges, EditError, NewComponent, ProjectUpdate, add_component, add_lockfile,
    edit_file, remove_component, remove_lockfile, set_component, utc_date,
};

use crate::{Failure, resolve_project};

/// Help text or a stack invocation.
pub(crate) enum Parsed {
    /// `stack --help` or a subcommand's `--help`.
    Help(&'static str),
    /// A stack change.
    Run(Request),
}

/// One stack command.
pub(crate) enum Request {
    /// Append a component.
    Add(AddRequest),
    /// Change one component.
    Set(SetRequest),
    /// Remove one component.
    Remove(NameRequest),
    /// Append a lockfile path.
    AddLockfile(PathRequest),
    /// Remove a lockfile path.
    RemoveLockfile(PathRequest),
}

/// Arguments of `stack add`.
pub(crate) struct AddRequest {
    /// Project id or path.
    pub project: String,
    /// Component role.
    pub kind: String,
    /// Component name.
    pub name: String,
    /// Version, when given.
    pub version: Option<String>,
    /// Note, when given.
    pub note: Option<String>,
    /// endoflife.date product id, when given.
    pub eol_product: Option<String>,
}

/// Arguments of `stack set`.
pub(crate) struct SetRequest {
    /// Project id or path.
    pub project: String,
    /// Component name.
    pub name: String,
    /// Version update.
    pub version: Update,
    /// Note update.
    pub note: Update,
    /// Product id update.
    pub eol_product: Update,
}

/// Arguments of `stack remove`.
pub(crate) struct NameRequest {
    /// Project id or path.
    pub project: String,
    /// Component name.
    pub name: String,
}

/// Arguments of a lockfile command.
pub(crate) struct PathRequest {
    /// Project id or path.
    pub project: String,
    /// Lockfile path.
    pub path: String,
}

/// Whether an optional field stays, changes, or is removed.
#[derive(Clone)]
pub(crate) enum Update {
    /// The flag was absent.
    Keep,
    /// Store this text.
    Set(String),
    /// The flag was an empty string.
    Clear,
}

pub(crate) fn parse(args: &[OsString]) -> Result<Parsed, String> {
    if args.is_empty() {
        return Err("stack needs a subcommand".to_owned());
    }
    if args.iter().any(|arg| arg == "--help") {
        let words: Vec<&str> = args
            .iter()
            .map(|arg| arg.to_str().unwrap_or(""))
            .filter(|text| *text != "--help")
            .collect();
        let known = matches!(
            words.as_slice(),
            [] | ["add" | "set" | "remove" | "lockfile"] | ["lockfile", "add" | "remove"]
        );
        if known && args.len() == words.len() + 1 {
            return Ok(Parsed::Help(HELP));
        }
        return Err("too many arguments".to_owned());
    }
    match args[0].to_string_lossy().as_ref() {
        "add" => parse_add(&args[1..]).map(|request| Parsed::Run(Request::Add(request))),
        "set" => parse_set(&args[1..]).map(|request| Parsed::Run(Request::Set(request))),
        "remove" => parse_remove(&args[1..]).map(|request| Parsed::Run(Request::Remove(request))),
        "lockfile" => parse_lockfile(&args[1..]),
        other => Err(format!("unrecognized argument: {other}")),
    }
}

pub(crate) fn run(
    request: &Request,
    workspace: Option<&std::path::Path>,
) -> Result<ExitCode, Failure> {
    let today = utc_date(SystemTime::now())
        .ok_or_else(|| Failure::Data("cannot read the system clock".to_owned()))?;
    match request {
        Request::Add(request) => {
            let path = resolve_project(&request.project, workspace)?;
            edit_file(&path, &today, |project, updated| {
                add_component(
                    project,
                    NewComponent {
                        kind: &request.kind,
                        name: &request.name,
                        version: request.version.as_deref(),
                        note: request.note.as_deref(),
                        eol_product: request.eol_product.as_deref(),
                    },
                    updated,
                )
            })
            .map_err(|error| map_edit(&error))?;
            println!("added component {}", request.name);
        }
        Request::Set(request) => {
            let path = resolve_project(&request.project, workspace)?;
            edit_file(&path, &today, |project, updated| {
                set_component(
                    project,
                    &request.name,
                    ComponentChanges {
                        version: to_update(&request.version),
                        note: to_update(&request.note),
                        eol_product: to_update(&request.eol_product),
                    },
                    updated,
                )
            })
            .map_err(|error| map_edit(&error))?;
            println!("updated component {}", request.name);
        }
        Request::Remove(request) => {
            let path = resolve_project(&request.project, workspace)?;
            edit_file(&path, &today, |project, updated| {
                remove_component(project, &request.name, updated)
            })
            .map_err(|error| map_edit(&error))?;
            println!("removed component {}", request.name);
        }
        Request::AddLockfile(request) => {
            let path = resolve_project(&request.project, workspace)?;
            edit_file(&path, &today, |project, updated| {
                add_lockfile(project, &request.path, updated)
            })
            .map_err(|error| map_edit(&error))?;
            println!("added lockfile {}", request.path);
        }
        Request::RemoveLockfile(request) => {
            let path = resolve_project(&request.project, workspace)?;
            edit_file(&path, &today, |project, updated| {
                remove_lockfile(project, &request.path, updated)
            })
            .map_err(|error| map_edit(&error))?;
            println!("removed lockfile {}", request.path);
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn to_update(update: &Update) -> ProjectUpdate<'_> {
    match update {
        Update::Keep => ProjectUpdate::Keep,
        Update::Set(text) => ProjectUpdate::Set(text),
        Update::Clear => ProjectUpdate::Clear,
    }
}

fn map_edit(error: &EditError) -> Failure {
    if error.is_usage() {
        Failure::Usage(error.to_string())
    } else {
        Failure::Data(error.to_string())
    }
}

fn parse_add(args: &[OsString]) -> Result<AddRequest, String> {
    let mut project = None;
    let mut kind = None;
    let mut name = None;
    let mut version = None;
    let mut note = None;
    let mut eol_product = None;
    let mut index = 0;
    while index < args.len() {
        let rendered = args[index].to_string_lossy();
        match rendered.as_ref() {
            "--kind" => kind = Some(required(args, &mut index, "--kind")?),
            "--name" => name = Some(required(args, &mut index, "--name")?),
            "--version" => version = Some(required(args, &mut index, "--version")?),
            "--note" => note = Some(required(args, &mut index, "--note")?),
            "--eol-product" => eol_product = Some(required(args, &mut index, "--eol-product")?),
            other if other.starts_with('-') => {
                return Err(format!("unrecognized argument: {other}"));
            }
            _ if project.is_some() => return Err("too many arguments".to_owned()),
            _ => project = Some(rendered.into_owned()),
        }
        index += 1;
    }
    Ok(AddRequest {
        project: project.ok_or_else(|| "stack add needs a project".to_owned())?,
        kind: kind.ok_or_else(|| "--kind needs a value".to_owned())?,
        name: name.ok_or_else(|| "--name needs a value".to_owned())?,
        version,
        note,
        eol_product,
    })
}

fn parse_set(args: &[OsString]) -> Result<SetRequest, String> {
    let mut words = Vec::new();
    let mut version = Update::Keep;
    let mut note = Update::Keep;
    let mut eol_product = Update::Keep;
    let mut index = 0;
    while index < args.len() {
        let rendered = args[index].to_string_lossy();
        match rendered.as_ref() {
            "--version" => version = update_flag(args, &mut index, "--version", &version)?,
            "--note" => note = update_flag(args, &mut index, "--note", &note)?,
            "--eol-product" => {
                eol_product = update_flag(args, &mut index, "--eol-product", &eol_product)?;
            }
            other if other.starts_with('-') => {
                return Err(format!("unrecognized argument: {other}"));
            }
            _ => words.push(rendered.into_owned()),
        }
        index += 1;
    }
    if words.len() != 2 {
        return Err("stack set needs a project and a component name".to_owned());
    }
    Ok(SetRequest {
        project: words[0].clone(),
        name: words[1].clone(),
        version,
        note,
        eol_product,
    })
}

fn parse_remove(args: &[OsString]) -> Result<NameRequest, String> {
    if args.len() != 2
        || args
            .iter()
            .any(|arg| arg.to_string_lossy().starts_with('-'))
    {
        return Err("stack remove needs a project and a component name".to_owned());
    }
    Ok(NameRequest {
        project: args[0].to_string_lossy().into_owned(),
        name: args[1].to_string_lossy().into_owned(),
    })
}

fn parse_lockfile(args: &[OsString]) -> Result<Parsed, String> {
    if args.is_empty() {
        return Err("stack lockfile needs add or remove".to_owned());
    }
    let action = args[0].to_string_lossy();
    let request = parse_path(&args[1..])?;
    match action.as_ref() {
        "add" => Ok(Parsed::Run(Request::AddLockfile(request))),
        "remove" => Ok(Parsed::Run(Request::RemoveLockfile(request))),
        other => Err(format!("unrecognized argument: {other}")),
    }
}

fn parse_path(args: &[OsString]) -> Result<PathRequest, String> {
    if args.len() != 2
        || args
            .iter()
            .any(|arg| arg.to_string_lossy().starts_with('-'))
    {
        return Err("stack lockfile needs a project and a path".to_owned());
    }
    Ok(PathRequest {
        project: args[0].to_string_lossy().into_owned(),
        path: args[1].to_string_lossy().into_owned(),
    })
}

fn update_flag(
    args: &[OsString],
    index: &mut usize,
    name: &str,
    current: &Update,
) -> Result<Update, String> {
    if !matches!(current, Update::Keep) {
        return Err(format!("duplicate option: {name}"));
    }
    let value = required(args, index, name)?;
    if value.is_empty() {
        Ok(Update::Clear)
    } else {
        Ok(Update::Set(value))
    }
}

fn required(args: &[OsString], index: &mut usize, name: &str) -> Result<String, String> {
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

const HELP: &str = "\
Change the stack recorded in a project file.

Usage:
  craftward stack add <PROJECT> --kind <KIND> --name <NAME>
      [--version TEXT] [--note TEXT] [--eol-product TEXT]
  craftward stack set <PROJECT> <NAME> [--version TEXT] [--note TEXT]
      [--eol-product TEXT]
  craftward stack remove <PROJECT> <NAME>
  craftward stack lockfile add <PROJECT> <PATH>
  craftward stack lockfile remove <PROJECT> <PATH>

Components are matched by name, ignoring letter case. An empty --version,
--note, or --eol-product removes that field. The file is written only when
the result is valid, and updated is set to today.

Exit status is 0 on success, 1 when the name or file is wrong, and 2 when
the command line is wrong.

Example:
  craftward stack add harbor-light --kind runtime --name Node.js --version 22
  craftward stack lockfile add harbor-light package-lock.json";
