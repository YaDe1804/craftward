//! Commands that read or change a project file.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fmt::Write as _;
use std::process::ExitCode;
use std::time::SystemTime;

use craftward_core::{
    EditError, NewIssue, NewPhase, NewTask, NewWatch, ProjectChanges, ProjectUpdate, add_issue,
    add_phase, add_task, add_watch, edit_file, load_from_path, remove_issue, remove_phase,
    remove_task, remove_watch, render_project, set_issue_status, set_project, set_task_status,
    utc_date,
};

use crate::{Failure, resolve_project};

/// One parsed edit or show command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Request {
    Show {
        target: String,
    },
    SetProject {
        target: String,
        name: Option<String>,
        status: Option<String>,
        version: Update,
        description: Update,
        source_dir: Option<String>,
    },
    AddPhase {
        target: String,
        title: String,
        id: Option<String>,
        goal: Option<String>,
    },
    RemovePhase {
        target: String,
        phase: String,
        force: bool,
    },
    AddTask {
        target: String,
        phase: String,
        title: String,
        id: Option<String>,
        status: Option<String>,
        due: Option<String>,
        note: Option<String>,
    },
    SetTaskStatus {
        target: String,
        phase: String,
        task: String,
        status: String,
    },
    RemoveTask {
        target: String,
        phase: String,
        task: String,
    },
    AddIssue {
        target: String,
        title: String,
        severity: String,
        id: Option<String>,
        url: Option<String>,
        note: Option<String>,
    },
    SetIssueStatus {
        target: String,
        issue: String,
        status: String,
    },
    RemoveIssue {
        target: String,
        issue: String,
    },
    AddWatch {
        target: String,
        kind: String,
        title: String,
        due: String,
        id: Option<String>,
        note: Option<String>,
    },
    RemoveWatch {
        target: String,
        watch: String,
    },
}

/// A header field that stays, is replaced, or is removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Update {
    Keep,
    Set(String),
    Clear,
}

/// Help text or a command ready to run.
pub(crate) enum Parsed {
    Help(&'static str),
    Run(Request),
}

pub(crate) fn parse(name: &str, args: &[OsString]) -> Result<Parsed, String> {
    if let Some(words) = help_words(args)? {
        return help_for(name, &words).map(Parsed::Help);
    }
    let request = match name {
        "show" => parse_show(args)?,
        "project" => parse_project(args)?,
        "phase" => parse_phase(args)?,
        "task" => parse_task(args)?,
        "issue" => parse_issue(args)?,
        "watch" => parse_watch(args)?,
        _ => return Err(format!("unrecognized command: {name}")),
    };
    Ok(Parsed::Run(request))
}

pub(crate) fn run(
    request: Request,
    workspace: Option<&std::path::Path>,
) -> Result<ExitCode, Failure> {
    match request {
        Request::Show { target } => {
            let path = resolve_project(&target, workspace)?;
            let project = load_from_path(&path).map_err(|error| Failure::Data(show_load(error)))?;
            print!("{}", render_project(&project));
            Ok(ExitCode::SUCCESS)
        }
        other => {
            let today = utc_date(SystemTime::now())
                .ok_or_else(|| Failure::Data("cannot read the system clock".to_owned()))?;
            run_change(&other, workspace, &today)
        }
    }
}

fn run_change(
    request: &Request,
    workspace: Option<&std::path::Path>,
    today: &str,
) -> Result<ExitCode, Failure> {
    let path = resolve_project(request.target(), workspace)?;
    let outcome = edit_file(&path, today, |project, updated_on| {
        apply(request, project, updated_on)
    });
    match outcome {
        Ok(done) => {
            println!("{}", confirmation(request, done.created_id.as_deref()));
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => Err(map_edit(&error)),
    }
}

fn apply(
    request: &Request,
    project: &craftward_core::Project,
    updated_on: &str,
) -> Result<craftward_core::EditOutcome, EditError> {
    match request {
        Request::SetProject {
            name,
            status,
            version,
            description,
            source_dir,
            ..
        } => set_project(
            project,
            ProjectChanges {
                name: name.as_deref(),
                status: status.as_deref(),
                version: version.as_update(),
                description: description.as_update(),
                source_dir: source_dir.as_deref(),
            },
            updated_on,
        ),
        Request::AddPhase {
            title, id, goal, ..
        } => add_phase(
            project,
            NewPhase {
                title,
                id: id.as_deref(),
                goal: goal.as_deref(),
            },
            updated_on,
        ),
        Request::RemovePhase { phase, force, .. } => {
            remove_phase(project, phase, *force, updated_on)
        }
        Request::AddTask {
            phase,
            title,
            id,
            status,
            due,
            note,
            ..
        } => add_task(
            project,
            NewTask {
                phase,
                title,
                id: id.as_deref(),
                status: status.as_deref(),
                due: due.as_deref(),
                note: note.as_deref(),
            },
            updated_on,
        ),
        Request::SetTaskStatus {
            phase,
            task,
            status,
            ..
        } => set_task_status(project, phase, task, status, updated_on),
        Request::RemoveTask { phase, task, .. } => remove_task(project, phase, task, updated_on),
        Request::AddIssue { .. }
        | Request::SetIssueStatus { .. }
        | Request::RemoveIssue { .. }
        | Request::AddWatch { .. }
        | Request::RemoveWatch { .. } => apply_record(request, project, updated_on),
        Request::Show { .. } => unreachable!("show does not change the file"),
    }
}

fn apply_record(
    request: &Request,
    project: &craftward_core::Project,
    updated_on: &str,
) -> Result<craftward_core::EditOutcome, EditError> {
    match request {
        Request::AddIssue {
            title,
            severity,
            id,
            url,
            note,
            ..
        } => add_issue(
            project,
            NewIssue {
                title,
                severity,
                id: id.as_deref(),
                url: url.as_deref(),
                note: note.as_deref(),
            },
            updated_on,
        ),
        Request::SetIssueStatus { issue, status, .. } => {
            set_issue_status(project, issue, status, updated_on)
        }
        Request::RemoveIssue { issue, .. } => remove_issue(project, issue, updated_on),
        Request::AddWatch {
            kind,
            title,
            due,
            id,
            note,
            ..
        } => add_watch(
            project,
            NewWatch {
                kind,
                title,
                due,
                id: id.as_deref(),
                note: note.as_deref(),
            },
            updated_on,
        ),
        Request::RemoveWatch { watch, .. } => remove_watch(project, watch, updated_on),
        Request::Show { .. }
        | Request::SetProject { .. }
        | Request::AddPhase { .. }
        | Request::RemovePhase { .. }
        | Request::AddTask { .. }
        | Request::SetTaskStatus { .. }
        | Request::RemoveTask { .. } => unreachable!("handled by apply"),
    }
}

fn confirmation(request: &Request, created: Option<&str>) -> String {
    match request {
        Request::SetProject { .. } => "updated project".to_owned(),
        Request::AddPhase { .. } => format!("added phase {}", created.unwrap_or("?")),
        Request::RemovePhase { phase, .. } => format!("removed phase {phase}"),
        Request::AddTask { .. } => format!("added task {}", created.unwrap_or("?")),
        Request::SetTaskStatus { task, status, .. } => format!("set task {task} to {status}"),
        Request::RemoveTask { task, .. } => format!("removed task {task}"),
        Request::AddIssue { .. } => format!("added issue {}", created.unwrap_or("?")),
        Request::SetIssueStatus { issue, status, .. } => {
            format!("set issue {issue} to {status}")
        }
        Request::RemoveIssue { issue, .. } => format!("removed issue {issue}"),
        Request::AddWatch { .. } => format!("added watch {}", created.unwrap_or("?")),
        Request::RemoveWatch { watch, .. } => format!("removed watch {watch}"),
        Request::Show { .. } => String::new(),
    }
}

fn map_edit(error: &EditError) -> Failure {
    if error.is_usage() {
        Failure::Usage(error.to_string())
    } else {
        Failure::Data(error.to_string())
    }
}

fn show_load(error: craftward_core::LoadError) -> String {
    match error {
        craftward_core::LoadError::Validation(errors) => {
            let mut message = String::from("invalid project file");
            for item in errors {
                let _ = write!(message, "\n  {item}");
            }
            message
        }
        other => other.to_string(),
    }
}

impl Request {
    fn target(&self) -> &str {
        match self {
            Self::Show { target }
            | Self::SetProject { target, .. }
            | Self::AddPhase { target, .. }
            | Self::RemovePhase { target, .. }
            | Self::AddTask { target, .. }
            | Self::SetTaskStatus { target, .. }
            | Self::RemoveTask { target, .. }
            | Self::AddIssue { target, .. }
            | Self::SetIssueStatus { target, .. }
            | Self::RemoveIssue { target, .. }
            | Self::AddWatch { target, .. }
            | Self::RemoveWatch { target, .. } => target,
        }
    }
}

impl Update {
    fn as_update(&self) -> ProjectUpdate<'_> {
        match self {
            Self::Keep => ProjectUpdate::Keep,
            Self::Set(text) => ProjectUpdate::Set(text),
            Self::Clear => ProjectUpdate::Clear,
        }
    }
}

fn parse_show(args: &[OsString]) -> Result<Request, String> {
    let ParsedWords { words, options, .. } = parse_words(args, &[], &[])?;
    if !options.is_empty() {
        return Err(format!(
            "unrecognized argument: {}",
            options.keys().next().unwrap_or(&String::new())
        ));
    }
    let [target] = words_as::<1>(words, "show requires a project id or path")?;
    Ok(Request::Show { target })
}

fn parse_project(args: &[OsString]) -> Result<Request, String> {
    let Some(verb) = args.first() else {
        return Err("project requires a subcommand: set".to_owned());
    };
    if verb != "set" {
        return Err(format!(
            "unrecognized command: project {}",
            verb.to_string_lossy()
        ));
    }
    let ParsedWords { words, options, .. } = parse_words(
        &args[1..],
        &[
            "--name",
            "--status",
            "--version",
            "--description",
            "--source-dir",
        ],
        &[],
    )?;
    let [target] = words_as::<1>(words, "project set requires a project id or path")?;
    if options.is_empty() {
        return Err("project set requires at least one option".to_owned());
    }
    Ok(Request::SetProject {
        target,
        name: options.get("--name").cloned(),
        status: options.get("--status").cloned(),
        version: clearable(options.get("--version")),
        description: clearable(options.get("--description")),
        source_dir: options.get("--source-dir").cloned(),
    })
}

fn parse_phase(args: &[OsString]) -> Result<Request, String> {
    let Some(verb) = args.first() else {
        return Err("phase requires a subcommand: add, remove".to_owned());
    };
    match verb.to_string_lossy().as_ref() {
        "add" => {
            let ParsedWords { words, options, .. } =
                parse_words(&args[1..], &["--title", "--id", "--goal"], &[])?;
            let [target] = words_as::<1>(words, "phase add requires a project id or path")?;
            let title = required(&options, "--title")?;
            Ok(Request::AddPhase {
                target,
                title,
                id: options.get("--id").cloned(),
                goal: options.get("--goal").cloned(),
            })
        }
        "remove" => {
            let ParsedWords {
                words, switches, ..
            } = parse_words(&args[1..], &[], &["--force"])?;
            let [target, phase] =
                words_as::<2>(words, "phase remove requires a project and a phase id")?;
            Ok(Request::RemovePhase {
                target,
                phase,
                force: switches.contains("--force"),
            })
        }
        other => Err(format!("unrecognized command: phase {other}")),
    }
}

fn parse_task(args: &[OsString]) -> Result<Request, String> {
    let Some(verb) = args.first() else {
        return Err("task requires a subcommand: add, status, done, remove".to_owned());
    };
    match verb.to_string_lossy().as_ref() {
        "add" => {
            let ParsedWords { words, options, .. } = parse_words(
                &args[1..],
                &["--title", "--id", "--status", "--due", "--note"],
                &[],
            )?;
            let [target, phase] =
                words_as::<2>(words, "task add requires a project and a phase id")?;
            Ok(Request::AddTask {
                target,
                phase,
                title: required(&options, "--title")?,
                id: options.get("--id").cloned(),
                status: options.get("--status").cloned(),
                due: options.get("--due").cloned(),
                note: options.get("--note").cloned(),
            })
        }
        "status" => {
            let ParsedWords { words, .. } = parse_words(&args[1..], &[], &[])?;
            let [target, phase, task, status] = words_as::<4>(
                words,
                "task status requires a project, phase id, task id, and status",
            )?;
            Ok(Request::SetTaskStatus {
                target,
                phase,
                task,
                status,
            })
        }
        "done" => {
            let ParsedWords { words, .. } = parse_words(&args[1..], &[], &[])?;
            let [target, phase, task] =
                words_as::<3>(words, "task done requires a project, phase id, and task id")?;
            Ok(Request::SetTaskStatus {
                target,
                phase,
                task,
                status: "done".to_owned(),
            })
        }
        "remove" => {
            let ParsedWords { words, .. } = parse_words(&args[1..], &[], &[])?;
            let [target, phase, task] = words_as::<3>(
                words,
                "task remove requires a project, phase id, and task id",
            )?;
            Ok(Request::RemoveTask {
                target,
                phase,
                task,
            })
        }
        other => Err(format!("unrecognized command: task {other}")),
    }
}

fn parse_issue(args: &[OsString]) -> Result<Request, String> {
    let Some(verb) = args.first() else {
        return Err("issue requires a subcommand: add, status, remove".to_owned());
    };
    match verb.to_string_lossy().as_ref() {
        "add" => {
            let ParsedWords { words, options, .. } = parse_words(
                &args[1..],
                &["--title", "--severity", "--id", "--url", "--note"],
                &[],
            )?;
            let [target] = words_as::<1>(words, "issue add requires a project id or path")?;
            Ok(Request::AddIssue {
                target,
                title: required(&options, "--title")?,
                severity: required(&options, "--severity")?,
                id: options.get("--id").cloned(),
                url: options.get("--url").cloned(),
                note: options.get("--note").cloned(),
            })
        }
        "status" => {
            let ParsedWords { words, .. } = parse_words(&args[1..], &[], &[])?;
            let [target, issue, status] = words_as::<3>(
                words,
                "issue status requires a project, issue id, and status",
            )?;
            Ok(Request::SetIssueStatus {
                target,
                issue,
                status,
            })
        }
        "remove" => {
            let ParsedWords { words, .. } = parse_words(&args[1..], &[], &[])?;
            let [target, issue] =
                words_as::<2>(words, "issue remove requires a project and an issue id")?;
            Ok(Request::RemoveIssue { target, issue })
        }
        other => Err(format!("unrecognized command: issue {other}")),
    }
}

fn parse_watch(args: &[OsString]) -> Result<Request, String> {
    let Some(verb) = args.first() else {
        return Err("watch requires a subcommand: add, remove".to_owned());
    };
    match verb.to_string_lossy().as_ref() {
        "add" => {
            let ParsedWords { words, options, .. } = parse_words(
                &args[1..],
                &["--kind", "--title", "--due", "--id", "--note"],
                &[],
            )?;
            let [target] = words_as::<1>(words, "watch add requires a project id or path")?;
            Ok(Request::AddWatch {
                target,
                kind: required(&options, "--kind")?,
                title: required(&options, "--title")?,
                due: required(&options, "--due")?,
                id: options.get("--id").cloned(),
                note: options.get("--note").cloned(),
            })
        }
        "remove" => {
            let ParsedWords { words, .. } = parse_words(&args[1..], &[], &[])?;
            let [target, watch] =
                words_as::<2>(words, "watch remove requires a project and a watch id")?;
            Ok(Request::RemoveWatch { target, watch })
        }
        other => Err(format!("unrecognized command: watch {other}")),
    }
}

fn clearable(value: Option<&String>) -> Update {
    match value {
        None => Update::Keep,
        Some(text) if text.is_empty() => Update::Clear,
        Some(text) => Update::Set(text.clone()),
    }
}

fn required(options: &BTreeMap<String, String>, flag: &str) -> Result<String, String> {
    options
        .get(flag)
        .cloned()
        .ok_or_else(|| format!("{flag} is required"))
}

fn words_as<const N: usize>(words: Vec<String>, message: &str) -> Result<[String; N], String> {
    if words.len() != N {
        return Err(message.to_owned());
    }
    words.try_into().map_err(|_| message.to_owned())
}

struct ParsedWords {
    words: Vec<String>,
    options: BTreeMap<String, String>,
    switches: BTreeSet<String>,
}

fn parse_words(
    args: &[OsString],
    options: &[&str],
    switches: &[&str],
) -> Result<ParsedWords, String> {
    let mut words = Vec::new();
    let mut values = BTreeMap::new();
    let mut flags = BTreeSet::new();
    let mut index = 0;
    while index < args.len() {
        let rendered = args[index].to_string_lossy();
        if options.contains(&rendered.as_ref()) {
            let Some(value) = args.get(index + 1) else {
                return Err(format!("{rendered} needs a value"));
            };
            let text = value.to_string_lossy();
            if text.starts_with('-') {
                return Err(format!("{rendered} needs a value"));
            }
            if values.contains_key(rendered.as_ref()) {
                return Err(format!("duplicate option: {rendered}"));
            }
            values.insert(rendered.into_owned(), text.into_owned());
            index += 2;
        } else if switches.contains(&rendered.as_ref()) {
            if flags.contains(rendered.as_ref()) {
                return Err(format!("duplicate option: {rendered}"));
            }
            flags.insert(rendered.into_owned());
            index += 1;
        } else if rendered.starts_with('-') {
            return Err(format!("unrecognized argument: {rendered}"));
        } else {
            words.push(rendered.into_owned());
            index += 1;
        }
    }
    Ok(ParsedWords {
        words,
        options: values,
        switches: flags,
    })
}

fn help_words(args: &[OsString]) -> Result<Option<Vec<String>>, String> {
    if !args.iter().any(|arg| arg == "--help") {
        return Ok(None);
    }
    if args.iter().filter(|arg| *arg == "--help").count() != 1 {
        return Err("too many arguments".to_owned());
    }
    let mut words = Vec::new();
    for arg in args {
        if arg == "--help" {
            continue;
        }
        let text = arg.to_string_lossy();
        if text.starts_with('-') {
            return Err("too many arguments".to_owned());
        }
        words.push(text.into_owned());
    }
    Ok(Some(words))
}

fn help_for(name: &str, words: &[String]) -> Result<&'static str, String> {
    let key = words.join(" ");
    let text = match (name, key.as_str()) {
        ("show", "") => SHOW_HELP,
        ("project", "") => PROJECT_HELP,
        ("project", "set") => PROJECT_SET_HELP,
        ("phase", "") => PHASE_HELP,
        ("phase", "add") => PHASE_ADD_HELP,
        ("phase", "remove") => PHASE_REMOVE_HELP,
        ("task", "") => TASK_HELP,
        ("task", "add") => TASK_ADD_HELP,
        ("task", "status") => TASK_STATUS_HELP,
        ("task", "done") => TASK_DONE_HELP,
        ("task", "remove") => TASK_REMOVE_HELP,
        ("issue", "") => ISSUE_HELP,
        ("issue", "add") => ISSUE_ADD_HELP,
        ("issue", "status") => ISSUE_STATUS_HELP,
        ("issue", "remove") => ISSUE_REMOVE_HELP,
        ("watch", "") => WATCH_HELP,
        ("watch", "add") => WATCH_ADD_HELP,
        ("watch", "remove") => WATCH_REMOVE_HELP,
        (_, "") => return Err(format!("unrecognized command: {name}")),
        _ => return Err(format!("unrecognized command: {name} {key}")),
    };
    Ok(text)
}

const SHOW_HELP: &str = "\
Print one project, including the ids used by later commands.

Usage:
  craftward show <PROJECT>

PROJECT is a workspace project id, a craftward.json file, or a directory
that contains one. The same rules as roadmap apply, including the error
when the value is both an id and an existing path.

Example:
  craftward show harbor-light";

const PROJECT_HELP: &str = "\
Change header fields of a project file.

Usage:
  craftward project set <PROJECT> [--name TEXT] [--status STATUS]
      [--version TEXT] [--description TEXT] [--source-dir PATH]

At least one option is required. An empty --version or --description
removes that field.

Example:
  craftward project --help
  craftward project set --help";

const PROJECT_SET_HELP: &str = "\
Change header fields of a project file.

Usage:
  craftward project set <PROJECT> [--name TEXT] [--status STATUS]
      [--version TEXT] [--description TEXT] [--source-dir PATH]

At least one option is required. STATUS is idea, in_development, released,
maintenance, paused, or discontinued. An empty --version or --description
removes that field. The file is rewritten in canonical form and updated is
set to today's UTC date.

Example:
  craftward project set harbor-light --version 0.5.0
  craftward project set harbor-light --description \"\"";

const PHASE_HELP: &str = "\
Add or remove a phase.

Usage:
  craftward phase add <PROJECT> --title TEXT [--id ID] [--goal TEXT]
  craftward phase remove <PROJECT> <PHASE> [--force]

Example:
  craftward phase add --help
  craftward phase remove --help";

const PHASE_ADD_HELP: &str = "\
Append a phase. The plan is created when the project has none.

Usage:
  craftward phase add <PROJECT> --title TEXT [--id ID] [--goal TEXT]

Without --id, the id is formed from the title. The command prints that id.

Example:
  craftward phase add harbor-light --title Foundation";

const PHASE_REMOVE_HELP: &str = "\
Remove a phase.

Usage:
  craftward phase remove <PROJECT> <PHASE> [--force]

The phase must have no tasks. --force removes the phase and its tasks.

Example:
  craftward phase remove harbor-light foundation
  craftward phase remove harbor-light foundation --force";

const TASK_HELP: &str = "\
Add, update, or remove a task.

Usage:
  craftward task add <PROJECT> <PHASE> --title TEXT [--id ID]
      [--status STATUS] [--due YYYY-MM-DD] [--note TEXT]
  craftward task status <PROJECT> <PHASE> <TASK> <STATUS>
  craftward task done <PROJECT> <PHASE> <TASK>
  craftward task remove <PROJECT> <PHASE> <TASK>

STATUS is open, in_progress, done, or dropped. The default for add is open.

Example:
  craftward task add --help
  craftward task done --help";

const TASK_ADD_HELP: &str = "\
Append a task to a phase.

Usage:
  craftward task add <PROJECT> <PHASE> --title TEXT [--id ID]
      [--status STATUS] [--due YYYY-MM-DD] [--note TEXT]

Without --id, the id is formed from the title. STATUS defaults to open.

Example:
  craftward task add harbor-light foundation --title \"Write the schema\"";

const TASK_STATUS_HELP: &str = "\
Set a task's status.

Usage:
  craftward task status <PROJECT> <PHASE> <TASK> <STATUS>

STATUS is open, in_progress, done, or dropped.

Example:
  craftward task status harbor-light foundation schema in_progress";

const TASK_DONE_HELP: &str = "\
Mark a task done.

Usage:
  craftward task done <PROJECT> <PHASE> <TASK>

This is task status with status done.

Example:
  craftward task done harbor-light foundation schema";

const TASK_REMOVE_HELP: &str = "\
Remove a task from a phase.

Usage:
  craftward task remove <PROJECT> <PHASE> <TASK>

Example:
  craftward task remove harbor-light foundation schema";

const ISSUE_HELP: &str = "\
Add, update, or remove an issue.

Usage:
  craftward issue add <PROJECT> --title TEXT --severity SEVERITY
      [--id ID] [--url URL] [--note TEXT]
  craftward issue status <PROJECT> <ISSUE> <STATUS>
  craftward issue remove <PROJECT> <ISSUE>

SEVERITY is low, medium, high, or critical. A new issue is open.
STATUS is open, in_progress, resolved, or wont_fix.

Example:
  craftward issue add --help";

const ISSUE_ADD_HELP: &str = "\
Append an issue. Its status is open.

Usage:
  craftward issue add <PROJECT> --title TEXT --severity SEVERITY
      [--id ID] [--url URL] [--note TEXT]

SEVERITY is low, medium, high, or critical. URL must start with http://
or https://. Without --id, the id is formed from the title.

Example:
  craftward issue add harbor-light --title \"Contrast\" --severity medium";

const ISSUE_STATUS_HELP: &str = "\
Set an issue's status.

Usage:
  craftward issue status <PROJECT> <ISSUE> <STATUS>

STATUS is open, in_progress, resolved, or wont_fix.

Example:
  craftward issue status harbor-light theme-contrast resolved";

const ISSUE_REMOVE_HELP: &str = "\
Remove an issue.

Usage:
  craftward issue remove <PROJECT> <ISSUE>

Example:
  craftward issue remove harbor-light theme-contrast";

const WATCH_HELP: &str = "\
Add or remove a watch item.

Usage:
  craftward watch add <PROJECT> --kind KIND --title TEXT --due YYYY-MM-DD
      [--id ID] [--note TEXT]
  craftward watch remove <PROJECT> <WATCH>

KIND is deadline, expiry, review, or other.

Example:
  craftward watch add --help";

const WATCH_ADD_HELP: &str = "\
Append a watch item.

Usage:
  craftward watch add <PROJECT> --kind KIND --title TEXT --due YYYY-MM-DD
      [--id ID] [--note TEXT]

KIND is deadline, expiry, review, or other. Without --id, the id is formed
from the title.

Example:
  craftward watch add harbor-light --kind expiry --title \"Renew domain\" --due 2027-06-30";

const WATCH_REMOVE_HELP: &str = "\
Remove a watch item.

Usage:
  craftward watch remove <PROJECT> <WATCH>

Example:
  craftward watch remove harbor-light domain-renewal";
