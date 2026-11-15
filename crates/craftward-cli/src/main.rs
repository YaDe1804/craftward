//! Command-line interface for Craftward.

mod editing;
mod findings;
mod radar;
mod stack;

use std::env;
use std::fmt::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::SystemTime;

use craftward_core::{
    Language, LoadError, ProcessHost, ProjectRow, ProjectStatus, WorkspaceError, add_project,
    init_project, is_portable_absolute, list_projects, load_from_path, locate_workspace,
    missing_paths, project_file, project_file_by_id, remove_project, utc_date, write_roadmap,
};

/// Process exit status for a usage error.
const USAGE_ERROR: u8 = 2;

/// Process exit status when a file or request fails.
const DATA_ERROR: u8 = 1;

fn main() -> ExitCode {
    match parse_args() {
        Ok(invocation) => match run(invocation) {
            Ok(code) => code,
            Err(Failure::Data(message)) => {
                eprintln!("error: {message}");
                ExitCode::from(DATA_ERROR)
            }
            Err(Failure::Usage(message)) => {
                eprintln!("error: {message}");
                ExitCode::from(USAGE_ERROR)
            }
        },
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::from(USAGE_ERROR)
        }
    }
}

/// Why a command stopped. Usage errors exit 2; everything else exits 1.
pub(crate) enum Failure {
    Usage(String),
    Data(String),
}

impl From<String> for Failure {
    fn from(message: String) -> Self {
        Self::Data(message)
    }
}

struct Invocation {
    workspace: Option<PathBuf>,
    command: Command,
}

#[derive(Clone, Copy)]
enum Topic {
    Init,
    Add,
    Remove,
    List,
    Validate,
    Workspace,
    Roadmap,
}

enum Command {
    Version,
    Help,
    Init {
        directory: Option<PathBuf>,
        id: String,
        name: String,
        status: ProjectStatus,
    },
    Add {
        path: PathBuf,
    },
    Remove {
        id: String,
    },
    List,
    Validate {
        paths: Vec<PathBuf>,
    },
    WorkspacePath,
    Roadmap {
        target: String,
        directory: Option<PathBuf>,
        language: Language,
    },
    Edit(editing::Request),
    Radar(radar::Request),
    Findings(findings::Request),
    Stack(stack::Request),
    HelpText(&'static str),
    Usage(Topic),
}

fn run(invocation: Invocation) -> Result<ExitCode, Failure> {
    match invocation.command {
        Command::Version => {
            println!("{} {}", env!("CARGO_BIN_NAME"), craftward_core::version());
            Ok(ExitCode::SUCCESS)
        }
        Command::Help => {
            print_help();
            Ok(ExitCode::SUCCESS)
        }
        Command::Usage(topic) => {
            print_command_help(topic);
            Ok(ExitCode::SUCCESS)
        }
        Command::Init {
            directory,
            id,
            name,
            status,
        } => run_init(directory.as_deref(), &id, &name, status),
        Command::Add { path } => {
            let workspace = workspace_file(invocation.workspace.as_deref())?;
            let input = absolute_from_cwd(&path)?;
            match add_project(&workspace, &input) {
                Ok(id) => {
                    println!("added {id}");
                    Ok(ExitCode::SUCCESS)
                }
                Err(error) => Err(Failure::Data(explain_workspace(&error))),
            }
        }
        Command::Remove { id } => {
            let workspace = workspace_file(invocation.workspace.as_deref())?;
            match remove_project(&workspace, &id) {
                Ok(()) => {
                    println!("removed {id}");
                    Ok(ExitCode::SUCCESS)
                }
                Err(error) => Err(Failure::Data(error.to_string())),
            }
        }
        Command::List => {
            let workspace = workspace_file(invocation.workspace.as_deref())?;
            match list_projects(&workspace) {
                Ok(rows) => Ok(print_list(&rows)),
                Err(error) => Err(Failure::Data(explain_workspace(&error))),
            }
        }
        Command::Validate { paths } => {
            if paths.is_empty() {
                let workspace = workspace_file(invocation.workspace.as_deref())?;
                Ok(validate_workspace(&workspace)?)
            } else {
                Ok(validate_files(&paths))
            }
        }
        Command::WorkspacePath => {
            let location = locate_workspace(&ProcessHost, invocation.workspace.as_deref())
                .map_err(|error| error.to_string())?;
            let path = absolute_from_cwd(&location.path)?;
            println!("{} ({})", path.display(), location.origin.label());
            Ok(ExitCode::SUCCESS)
        }
        Command::Roadmap {
            target,
            directory,
            language,
        } => run_roadmap(
            &target,
            directory.as_deref(),
            language,
            invocation.workspace.as_deref(),
        ),
        Command::Edit(request) => editing::run(request, invocation.workspace.as_deref()),
        Command::Radar(request) => radar::run(&request, invocation.workspace.as_deref()),
        Command::Findings(request) => findings::run(&request, invocation.workspace.as_deref()),
        Command::Stack(request) => stack::run(&request, invocation.workspace.as_deref()),
        Command::HelpText(text) => {
            println!("{text}");
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn run_init(
    directory: Option<&Path>,
    id: &str,
    name: &str,
    status: ProjectStatus,
) -> Result<ExitCode, Failure> {
    let directory = match directory {
        Some(path) => absolute_from_cwd(path)?,
        None => env::current_dir()
            .map_err(|error| format!("cannot read the current directory: {error}"))?,
    };
    match init_project(&directory, id, name, status) {
        Ok(path) => {
            println!("created {}", path.display());
            Ok(ExitCode::SUCCESS)
        }
        Err(WorkspaceError::Validation(errors)) => {
            let mut message = String::from("invalid project file");
            for error in errors {
                let _ = write!(message, "\n  {}: {}", error.path, error.message);
            }
            Err(Failure::Data(message))
        }
        Err(error) => Err(Failure::Data(error.to_string())),
    }
}

fn run_roadmap(
    target: &str,
    directory: Option<&Path>,
    language: Language,
    workspace: Option<&Path>,
) -> Result<ExitCode, Failure> {
    let project = resolve_project(target, workspace)?;
    let directory = match directory {
        Some(path) => absolute_from_cwd(path)?,
        None => env::current_dir()
            .map_err(|error| format!("cannot read the current directory: {error}"))?,
    };
    let generated_on =
        utc_date(SystemTime::now()).ok_or_else(|| "cannot read the system clock".to_owned())?;
    match write_roadmap(&project, &directory, language, &generated_on) {
        Ok(path) => {
            println!("{}", path.display());
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => Err(Failure::Data(error.to_string())),
    }
}

pub(crate) fn resolve_project(target: &str, workspace: Option<&Path>) -> Result<PathBuf, Failure> {
    let path = platform_path(Path::new(target));
    let exists = path.exists();
    let registered = match registered_project_id(target, workspace) {
        Ok(found) => found,
        Err(_) if is_path_target(target, &path) => false,
        Err(error) => return Err(Failure::Data(error)),
    };
    if exists && registered {
        let kind = if path.is_dir() { "folder" } else { "path" };
        let separator = std::path::MAIN_SEPARATOR;
        return Err(Failure::Usage(format!(
            "`{target}` is both a workspace project id and an existing {kind}. \
             Pass `.{separator}{target}` to use the {kind}, or pass the path of \
             the project file to use the registered project."
        )));
    }
    if is_path_target(target, &path) {
        let absolute = absolute_from_cwd(&path)?;
        return project_file(&absolute).map_err(|error| Failure::Data(error.to_string()));
    }
    let workspace = workspace_file(workspace)?;
    project_file_by_id(&workspace, target).map_err(|error| Failure::Data(error.to_string()))
}

/// Whether `target` is the id of a readable project in the workspace.
///
/// A missing workspace file means the id is not registered. A workspace that
/// cannot be located or read is returned as an error so the caller can decide
/// whether a path argument still stands on its own.
fn registered_project_id(target: &str, workspace: Option<&Path>) -> Result<bool, String> {
    let file = workspace_file(workspace)?;
    match project_file_by_id(&file, target) {
        Ok(_) => Ok(true),
        Err(WorkspaceError::UnknownId(_)) => Ok(false),
        Err(error) => Err(error.to_string()),
    }
}

fn is_path_target(raw: &str, path: &Path) -> bool {
    path.exists()
        || Path::new(raw)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
        || raw.contains('/')
        || raw.contains('\\')
        || is_portable_absolute(raw)
}

pub(crate) fn workspace_file(explicit: Option<&Path>) -> Result<PathBuf, String> {
    let location = locate_workspace(&ProcessHost, explicit).map_err(|error| error.to_string())?;
    absolute_from_cwd(&location.path)
}

fn absolute_from_cwd(path: &Path) -> Result<PathBuf, String> {
    let path = platform_path(path);
    if path.is_absolute() {
        Ok(path)
    } else {
        let current = env::current_dir()
            .map_err(|error| format!("cannot read the current directory: {error}"))?;
        Ok(platform_path(&current.join(path)))
    }
}

/// Rebuilds `path` so every separator is the one this operating system uses.
///
/// A `Path` keeps `/` that were already in the text. On Windows, joining such a
/// path produces a mixed result such as `C:\dir\app/craftward.json`.
fn platform_path(path: &Path) -> PathBuf {
    path.components().collect()
}

fn validate_workspace(path: &Path) -> Result<ExitCode, String> {
    let rows = list_projects(path).map_err(|error| explain_workspace(&error))?;
    if rows.is_empty() {
        println!("no projects");
        return Ok(ExitCode::SUCCESS);
    }
    let mut failed = false;
    for row in rows {
        if !report_project(&row.path) {
            failed = true;
        }
    }
    Ok(exit_from_failure(failed))
}

fn validate_files(paths: &[PathBuf]) -> ExitCode {
    let mut failed = false;
    for path in paths {
        if !report_project(path) {
            failed = true;
        }
    }
    exit_from_failure(failed)
}

fn report_project(path: &Path) -> bool {
    let rendered = path.to_string_lossy();
    match load_from_path(path) {
        Ok(project) => {
            println!("{rendered}: ok");
            for missing in missing_paths(path, &project) {
                eprintln!("warning: {missing}");
            }
            true
        }
        Err(error) => {
            println!("{rendered}:");
            print_load_error(&error);
            false
        }
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

fn print_list(rows: &[ProjectRow]) -> ExitCode {
    if rows.is_empty() {
        println!("no projects");
        return ExitCode::SUCCESS;
    }
    let mut failed = false;
    let mut lines = Vec::new();
    for row in rows {
        lines.push(match &row.detail {
            Ok(snapshot) => ListLine {
                id: snapshot.id.clone(),
                name: snapshot.name.clone(),
                status: snapshot.status.as_str().to_owned(),
                version: snapshot
                    .current_version
                    .clone()
                    .unwrap_or_else(|| "-".to_owned()),
                open_tasks: snapshot.open_tasks.to_string(),
                open_issues: snapshot.open_issues.to_string(),
                next_due: snapshot.next_due.clone().unwrap_or_else(|| "-".to_owned()),
                note: String::new(),
            },
            Err(message) => {
                failed = true;
                ListLine {
                    id: "-".to_owned(),
                    name: "-".to_owned(),
                    status: "-".to_owned(),
                    version: "-".to_owned(),
                    open_tasks: "-".to_owned(),
                    open_issues: "-".to_owned(),
                    next_due: "-".to_owned(),
                    note: format!("{}: {message}", row.path.display()),
                }
            }
        });
    }
    print_table(&lines);
    exit_from_failure(failed)
}

struct ListLine {
    id: String,
    name: String,
    status: String,
    version: String,
    open_tasks: String,
    open_issues: String,
    next_due: String,
    note: String,
}

fn print_table(lines: &[ListLine]) {
    let headers = [
        "ID",
        "NAME",
        "STATUS",
        "VERSION",
        "OPEN_TASKS",
        "OPEN_ISSUES",
        "NEXT_DUE",
        "NOTE",
    ];
    let mut widths = headers.map(str::len);
    for line in lines {
        let cells = line.cells();
        for (width, cell) in widths.iter_mut().zip(cells) {
            *width = (*width).max(cell.len());
        }
    }
    print_row(&headers, &widths);
    for line in lines {
        let cells = line.cells();
        print_row(&cells, &widths);
    }
}

impl ListLine {
    fn cells(&self) -> [&str; 8] {
        [
            &self.id,
            &self.name,
            &self.status,
            &self.version,
            &self.open_tasks,
            &self.open_issues,
            &self.next_due,
            &self.note,
        ]
    }
}

fn print_row(cells: &[&str], widths: &[usize]) {
    let mut first = true;
    for (cell, width) in cells.iter().zip(widths) {
        if first {
            first = false;
        } else {
            print!("  ");
        }
        print!("{cell:<width$}");
    }
    println!();
}

fn exit_from_failure(failed: bool) -> ExitCode {
    if failed {
        ExitCode::from(DATA_ERROR)
    } else {
        ExitCode::SUCCESS
    }
}

fn explain_workspace(error: &WorkspaceError) -> String {
    match error {
        WorkspaceError::InvalidProject { path, source } => {
            let mut message = format!("invalid project file {}", path.display());
            match source {
                LoadError::Validation(errors) => {
                    for error in errors {
                        let _ = write!(message, "\n  {}: {}", error.path, error.message);
                    }
                }
                other => {
                    let _ = write!(message, "\n  {other}");
                }
            }
            message
        }
        WorkspaceError::Validation(errors) => {
            let mut message = String::from("invalid workspace file");
            for error in errors {
                let _ = write!(message, "\n  {}: {}", error.path, error.message);
            }
            message
        }
        other => other.to_string(),
    }
}

/// Accepts the global options and one command.
///
/// With no arguments, the program prints its name and version.
///
/// # Errors
///
/// Returns an error when the command line is not a supported form.
fn parse_args() -> Result<Invocation, String> {
    let arguments: Vec<_> = env::args_os().skip(1).collect();
    let (workspace, rest) = extract_workspace(&arguments)?;
    if rest.is_empty() {
        if workspace.is_some() {
            return Err("--workspace needs a command".to_owned());
        }
        return Ok(Invocation {
            workspace: None,
            command: Command::Version,
        });
    }
    let command_name = rest[0].to_string_lossy();
    if command_name == "--version" || command_name == "--help" {
        if rest.len() != 1 || workspace.is_some() {
            return Err("too many arguments".to_owned());
        }
        let command = if command_name == "--version" {
            Command::Version
        } else {
            Command::Help
        };
        return Ok(Invocation {
            workspace: None,
            command,
        });
    }
    if command_name.starts_with('-') {
        return Err(format!("unrecognized argument: {command_name}"));
    }
    let command_args = &rest[1..];
    let command = parse_command(&command_name, command_args)?;
    Ok(Invocation { workspace, command })
}

fn extract_workspace(
    arguments: &[std::ffi::OsString],
) -> Result<(Option<PathBuf>, Vec<std::ffi::OsString>), String> {
    let mut workspace = None;
    let mut rest = Vec::new();
    let mut index = 0;
    while index < arguments.len() {
        if arguments[index] == "--workspace" {
            let Some(value) = arguments.get(index + 1) else {
                return Err("--workspace needs a file path".to_owned());
            };
            if value.to_string_lossy().starts_with('-') {
                return Err("--workspace needs a file path".to_owned());
            }
            if workspace.is_some() {
                return Err("duplicate option: --workspace".to_owned());
            }
            workspace = Some(platform_path(Path::new(value)));
            index += 2;
        } else {
            rest.push(arguments[index].clone());
            index += 1;
        }
    }
    Ok((workspace, rest))
}

fn parse_command(name: &str, args: &[std::ffi::OsString]) -> Result<Command, String> {
    let topic = match name {
        "init" => Topic::Init,
        "add" => Topic::Add,
        "remove" => Topic::Remove,
        "list" => Topic::List,
        "validate" => Topic::Validate,
        "workspace" => Topic::Workspace,
        "roadmap" => Topic::Roadmap,
        "radar" => {
            return match radar::parse(args)? {
                radar::Parsed::Help(text) => Ok(Command::HelpText(text)),
                radar::Parsed::Run(request) => Ok(Command::Radar(request)),
            };
        }
        "findings" => {
            return match findings::parse(args)? {
                findings::Parsed::Help(text) => Ok(Command::HelpText(text)),
                findings::Parsed::Run(request) => Ok(Command::Findings(request)),
            };
        }
        "stack" => {
            return match stack::parse(args)? {
                stack::Parsed::Help(text) => Ok(Command::HelpText(text)),
                stack::Parsed::Run(request) => Ok(Command::Stack(request)),
            };
        }
        "show" | "project" | "phase" | "task" | "issue" | "watch" => {
            return match editing::parse(name, args)? {
                editing::Parsed::Help(text) => Ok(Command::HelpText(text)),
                editing::Parsed::Run(request) => Ok(Command::Edit(request)),
            };
        }
        _ => return Err(format!("unrecognized command: {name}")),
    };
    if help_only(name, args)? {
        return Ok(Command::Usage(topic));
    }
    match name {
        "init" => parse_init(args),
        "add" => parse_one_path(args).map(|path| Command::Add { path }),
        "remove" => parse_one_word(args, "remove").map(|id| Command::Remove { id }),
        "list" => {
            if args.is_empty() {
                Ok(Command::List)
            } else {
                Err(unexpected(&args[0]))
            }
        }
        "validate" => {
            let mut paths = Vec::new();
            for arg in args {
                let rendered = arg.to_string_lossy();
                if rendered.starts_with('-') {
                    return Err(format!("unrecognized argument: {rendered}"));
                }
                paths.push(platform_path(Path::new(arg)));
            }
            Ok(Command::Validate { paths })
        }
        "workspace" => parse_workspace_command(args),
        "roadmap" => parse_roadmap(args),
        _ => Err(format!("unrecognized command: {name}")),
    }
}

fn help_only(name: &str, args: &[std::ffi::OsString]) -> Result<bool, String> {
    if !args.iter().any(|arg| arg == "--help") {
        return Ok(false);
    }
    let workspace_path_help = name == "workspace"
        && args.len() == 2
        && args.iter().any(|arg| arg == "path")
        && args.iter().any(|arg| arg == "--help");
    if args.len() == 1 || workspace_path_help {
        Ok(true)
    } else {
        Err("too many arguments".to_owned())
    }
}

fn parse_workspace_command(args: &[std::ffi::OsString]) -> Result<Command, String> {
    if args.len() == 1 && args[0] == "path" {
        Ok(Command::WorkspacePath)
    } else if args.is_empty() {
        Err("workspace requires a subcommand: path".to_owned())
    } else if args[0] == "path" {
        Err("too many arguments".to_owned())
    } else {
        Err(unexpected(&args[0]))
    }
}

fn parse_one_path(args: &[std::ffi::OsString]) -> Result<PathBuf, String> {
    if args.len() != 1 {
        return Err("add requires one project file or directory".to_owned());
    }
    let rendered = args[0].to_string_lossy();
    if rendered.starts_with('-') {
        return Err(format!("unrecognized argument: {rendered}"));
    }
    Ok(platform_path(Path::new(&args[0])))
}

fn parse_one_word(args: &[std::ffi::OsString], command: &str) -> Result<String, String> {
    if args.len() != 1 {
        return Err(format!("{command} requires one project id"));
    }
    let rendered = args[0].to_string_lossy();
    if rendered.starts_with('-') {
        return Err(format!("unrecognized argument: {rendered}"));
    }
    Ok(rendered.into_owned())
}

fn parse_init(args: &[std::ffi::OsString]) -> Result<Command, String> {
    let mut directory = None;
    let mut id = None;
    let mut name = None;
    let mut status = None;
    let mut index = 0;
    while index < args.len() {
        let rendered = args[index].to_string_lossy();
        if rendered == "--id" || rendered == "--name" || rendered == "--status" {
            let Some(value) = args.get(index + 1) else {
                return Err(format!("{rendered} needs a value"));
            };
            if value.to_string_lossy().starts_with('-') {
                return Err(format!("{rendered} needs a value"));
            }
            let text = value.to_string_lossy().into_owned();
            match rendered.as_ref() {
                "--id" => assign(&mut id, "--id", text)?,
                "--name" => assign(&mut name, "--name", text)?,
                "--status" => assign(&mut status, "--status", text)?,
                _ => {}
            }
            index += 2;
        } else if rendered.starts_with('-') {
            return Err(format!("unrecognized argument: {rendered}"));
        } else if directory.is_some() {
            return Err("init accepts only one directory".to_owned());
        } else {
            directory = Some(platform_path(Path::new(&args[index])));
            index += 1;
        }
    }
    let Some(id) = id else {
        return Err("init requires --id".to_owned());
    };
    let Some(name) = name else {
        return Err("init requires --name".to_owned());
    };
    let status = match status {
        None => ProjectStatus::Idea,
        Some(text) => {
            ProjectStatus::parse(&text).ok_or_else(|| format!("unknown status: {text}"))?
        }
    };
    Ok(Command::Init {
        directory,
        id,
        name,
        status,
    })
}

fn parse_roadmap(args: &[std::ffi::OsString]) -> Result<Command, String> {
    let mut target = None;
    let mut directory = None;
    let mut language = None;
    let mut index = 0;
    while index < args.len() {
        let rendered = args[index].to_string_lossy();
        if rendered == "--dir" || rendered == "--lang" {
            let Some(value) = args.get(index + 1) else {
                return Err(format!("{rendered} needs a value"));
            };
            if value.to_string_lossy().starts_with('-') {
                return Err(format!("{rendered} needs a value"));
            }
            match rendered.as_ref() {
                "--dir" => {
                    if directory.is_some() {
                        return Err("duplicate option: --dir".to_owned());
                    }
                    directory = Some(platform_path(Path::new(value)));
                }
                "--lang" => {
                    if language.is_some() {
                        return Err("duplicate option: --lang".to_owned());
                    }
                    let text = value.to_string_lossy();
                    language = Some(
                        Language::parse(&text)
                            .ok_or_else(|| format!("unknown language: {text}"))?,
                    );
                }
                _ => {}
            }
            index += 2;
        } else if rendered.starts_with('-') {
            return Err(format!("unrecognized argument: {rendered}"));
        } else if target.is_some() {
            return Err("roadmap accepts only one project".to_owned());
        } else {
            target = Some(rendered.into_owned());
            index += 1;
        }
    }
    let Some(target) = target else {
        return Err("roadmap requires a project id or path".to_owned());
    };
    Ok(Command::Roadmap {
        target,
        directory,
        language: language.unwrap_or(Language::En),
    })
}

fn assign(slot: &mut Option<String>, flag: &str, value: String) -> Result<(), String> {
    if slot.is_some() {
        Err(format!("duplicate option: {flag}"))
    } else {
        *slot = Some(value);
        Ok(())
    }
}

fn unexpected(argument: &std::ffi::OsString) -> String {
    format!("unrecognized argument: {}", argument.to_string_lossy())
}

fn print_help() {
    let version = craftward_core::version();
    println!(
        "\
craftward {version}

Track project files and print a summary of the workspace.

Usage:
  craftward
  craftward --help
  craftward --version
  craftward [--workspace <FILE>] <COMMAND> [--help]

Commands:
  init [DIR] --id <ID> --name <NAME> [--status <STATUS>]
      Create DIR/craftward.json with the required fields. DIR defaults to
      the current directory. STATUS defaults to idea. An existing file is
      left unchanged, and the project is not added to the workspace.
  add <PATH>
      Add a craftward.json file, or a directory that contains one. The file
      must be valid. The same path and the same project id are rejected.
  remove <ID>
      Remove a project from the workspace. The project file stays where it is.
  list
      Show id, name, status, version, open tasks, open issues, and the next
      due date. A missing or invalid file is still listed, and the command
      then exits 1.
  craftward validate [PATH]...
      Check the given files, or every project in the workspace when no path
      is given. An error names its location, for example
      plan.phases[0].tasks[1].due. A missing source directory or lockfile is
      a warning and does not change the exit status.
  roadmap <ID|PATH> [--dir DIR] [--lang en|de]
      Write the next <id>_roadmap_vN.html for a workspace id or a project
      file. DIR defaults to the current directory. The language defaults to
      en. An existing file is never overwritten.
  show <PROJECT>
      Print a project, including the ids later commands use.
  project set <PROJECT> [--name ..] [--status ..] [--version ..]
      [--description ..] [--source-dir ..]
      Change header fields. An empty --version or --description removes it.
  phase add <PROJECT> --title .. | phase remove <PROJECT> <PHASE> [--force]
  task add|status|done|remove
  issue add|status|remove
  watch add|remove
  radar [PROJECT...] [--refresh] [--offline] [--json] [--all] [--strict]
      [--eol-warning-days N] [--cache-dir DIR] [--data-dir DIR]
      Look up known vulnerabilities, support ends, and store them.
  findings [PROJECT...] [--state ..] [--kind ..] [--json] [--data-dir DIR]
      List, show, ignore, or reopen stored findings.
  stack add|set|remove | stack lockfile add|remove
      Change stack components and lockfile paths.
  workspace path
      Print the workspace file in use and whether it came from --workspace,
      CRAFTWARD_WORKSPACE, or the default location.

--workspace selects the workspace file. Otherwise CRAFTWARD_WORKSPACE is used,
then the default directory for this system.

Exit status:
  0  success, or radar found nothing
  1  a file or request failed
  2  the command line is wrong
  3  radar has an open finding"
    );
}

fn print_command_help(topic: Topic) {
    let text = match topic {
        Topic::Init => {
            "\
Create a project file with the required fields.

Usage:
  craftward init [DIR] --id <ID> --name <NAME> [--status <STATUS>]

STATUS is idea, in_development, released, maintenance, paused, or
discontinued. The default is idea. DIR defaults to the current directory.
An existing craftward.json is not overwritten. The workspace list is not
changed.

Example:
  craftward init --id harbor-light --name \"Harbor Light\""
        }
        Topic::Add => {
            "\
Add a project file to the workspace.

Usage:
  craftward add <PATH>

PATH is a craftward.json file or a directory that contains one. The file
must be valid. A path or project id that is already listed is rejected.

Example:
  craftward add ./harbor-light"
        }
        Topic::Remove => {
            "\
Remove a project from the workspace.

Usage:
  craftward remove <ID>

The project file is not deleted.

Example:
  craftward remove harbor-light"
        }
        Topic::List => {
            "\
Show every project in the workspace.

Usage:
  craftward list

Open tasks and open issues are those marked open or in_progress. The next
due date is the earliest task or watch-item date still outstanding. Done
and dropped tasks are left out. A missing or invalid project file is shown
with a note, and the command exits 1.

Example:
  craftward list"
        }
        Topic::Validate => {
            "\
Check project files.

Usage:
  craftward validate [PATH]...

With no paths, every project in the workspace is checked. Each file is
reported as \"ok\" or as a list of errors. A missing source directory or
lockfile is reported as a warning and does not change the exit status.

Example:
  craftward validate
  craftward validate ./harbor-light/craftward.json"
        }
        Topic::Workspace => {
            "\
Show which workspace file is in use.

Usage:
  craftward workspace path

The line names the file and the source: command-line option, environment
variable CRAFTWARD_WORKSPACE, or default location.

Example:
  craftward workspace path"
        }
        Topic::Roadmap => {
            "\
Write a versioned HTML roadmap for one project.

Usage:
  craftward roadmap <ID|PATH> [--dir DIR] [--lang en|de]

ID selects a project from the workspace. PATH is a craftward.json file or
a directory that contains one. A path is recognized when it exists, ends
with .json, contains a separator, or is absolute. If the argument is both a
registered id and an existing path, the command exits 2. Pass .\\name or
./name to use the folder, or pass the project file path to use the
registered project. DIR defaults to the
current directory. --lang is en or de, and defaults to en. The file is
named <id>_roadmap_vN.html. N is one higher than the highest existing
number in DIR, starting at 1. An existing file is never overwritten.

Example:
  craftward roadmap harbor-light --dir ./docs/roadmap --lang de"
        }
    };
    println!("{text}");
}
