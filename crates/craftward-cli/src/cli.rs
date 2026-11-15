//! Clap command-line definition for `craftward`.

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use craftward_core::{FindingKind, Language, ProjectStatus, StateFilter};

/// Track project files and print a summary of the workspace.
#[derive(Debug, Parser)]
#[command(
    name = "craftward",
    version,
    about = "Track project files and print a summary of the workspace.",
    long_about = LONG_ABOUT,
    after_help = AFTER_HELP,
    disable_help_subcommand = true,
    infer_subcommands = false
)]
pub(crate) struct Cli {
    /// Workspace file path.
    #[arg(long, global = true, value_name = "FILE")]
    pub workspace: Option<PathBuf>,

    /// Directory for craftward.db and related data.
    #[arg(long, global = true, value_name = "DIR")]
    pub data_dir: Option<PathBuf>,

    /// Directory for cached OSV and endoflife.date responses.
    #[arg(long, global = true, value_name = "DIR")]
    pub cache_dir: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Create DIR/craftward.json with the required fields.
    #[command(long_about = INIT_ABOUT)]
    Init {
        /// Directory for the new project file. Defaults to the current directory.
        #[arg(value_name = "DIR")]
        directory: Option<PathBuf>,
        /// Project id.
        #[arg(long)]
        id: String,
        /// Project display name.
        #[arg(long)]
        name: String,
        /// Project status. Defaults to idea.
        #[arg(long, value_parser = parse_status, default_value = "idea")]
        status: ProjectStatus,
    },
    /// Add a craftward.json file, or a directory that contains one.
    #[command(long_about = ADD_ABOUT)]
    Add {
        /// Project file or directory.
        #[arg(value_name = "PATH")]
        path: PathBuf,
    },
    /// Remove a project from the workspace.
    #[command(long_about = REMOVE_ABOUT)]
    Remove {
        /// Project id.
        #[arg(value_name = "ID")]
        id: String,
    },
    /// Show every project in the workspace.
    #[command(long_about = LIST_ABOUT)]
    List,
    /// Check project files. craftward validate [PATH]... An error names its location, for example plan.phases[0].tasks[1].due.
    #[command(long_about = VALIDATE_ABOUT)]
    Validate {
        /// Project files to check. Empty means every workspace project.
        #[arg(value_name = "PATH")]
        paths: Vec<PathBuf>,
    },
    /// Show which workspace file is in use.
    #[command(long_about = WORKSPACE_ABOUT)]
    Workspace {
        #[command(subcommand)]
        command: WorkspaceCommand,
    },
    /// Write a versioned HTML roadmap for one project.
    #[command(long_about = ROADMAP_ABOUT)]
    Roadmap {
        /// Workspace id or project path.
        #[arg(value_name = "ID|PATH")]
        target: String,
        /// Output directory. Defaults to the current directory.
        #[arg(long = "dir", value_name = "DIR")]
        directory: Option<PathBuf>,
        /// Language for fixed labels.
        #[arg(long = "lang", value_parser = parse_language, default_value = "en")]
        language: Language,
    },
    /// Print one project, including the ids used by later commands.
    #[command(long_about = SHOW_ABOUT)]
    Show {
        /// Workspace id or project path.
        #[arg(value_name = "PROJECT")]
        project: String,
    },
    /// Change header fields of a project file.
    #[command(long_about = PROJECT_ABOUT)]
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
    /// Add or remove a phase.
    #[command(long_about = PHASE_ABOUT)]
    Phase {
        #[command(subcommand)]
        command: PhaseCommand,
    },
    /// Add, update, or remove a task.
    #[command(long_about = TASK_ABOUT)]
    Task {
        #[command(subcommand)]
        command: TaskCommand,
    },
    /// Add, update, or remove an issue.
    #[command(long_about = ISSUE_ABOUT)]
    Issue {
        #[command(subcommand)]
        command: IssueCommand,
    },
    /// Add or remove a watch item.
    #[command(long_about = WATCH_ABOUT)]
    Watch {
        #[command(subcommand)]
        command: WatchCommand,
    },
    /// Look up known vulnerabilities, support ends, and Play deadlines.
    #[command(long_about = RADAR_ABOUT)]
    Radar {
        /// Workspace ids or project paths. Empty means every workspace project.
        #[arg(value_name = "PROJECT")]
        projects: Vec<String>,
        /// Ignore fresh cache entries and ask again.
        #[arg(long, conflicts_with = "offline")]
        refresh: bool,
        /// Never contact OSV or endoflife.date.
        #[arg(long, conflicts_with = "refresh")]
        offline: bool,
        /// Print JSON instead of the text report.
        #[arg(long)]
        json: bool,
        /// Include ignored findings in the list.
        #[arg(long = "all")]
        show_all: bool,
        /// Count notices and soon-findings toward exit status 3.
        #[arg(long)]
        strict: bool,
        /// Days ahead that still count as `eol_soon`.
        #[arg(long, default_value_t = 90, value_name = "N")]
        eol_warning_days: u32,
        /// Days ahead that still count as `platform_deadline_soon`.
        #[arg(long, default_value_t = 180, value_name = "N")]
        deadline_warning_days: u32,
    },
    /// List and change stored vulnerability findings.
    #[command(long_about = FINDINGS_ABOUT)]
    Findings {
        #[command(subcommand)]
        command: Option<FindingsCommand>,
        /// Project ids or paths for the default list.
        #[arg(value_name = "PROJECT", global = false)]
        projects: Vec<String>,
        /// Which states to print.
        #[arg(long, value_enum, default_value_t = CliStateFilter::Open)]
        state: CliStateFilter,
        /// Which kind to print.
        #[arg(long, value_parser = parse_kind)]
        kind: Option<String>,
        /// Print JSON.
        #[arg(long)]
        json: bool,
    },
    /// Change the stack recorded in a project file.
    #[command(long_about = STACK_ABOUT)]
    Stack {
        #[command(subcommand)]
        command: StackCommand,
    },
    /// Show the Google Play target-API deadlines in use.
    #[command(long_about = DEADLINES_ABOUT)]
    Deadlines,
}

#[derive(Debug, Subcommand)]
pub(crate) enum WorkspaceCommand {
    /// Print the workspace file in use.
    #[command(long_about = WORKSPACE_PATH_ABOUT)]
    Path,
}

#[derive(Debug, Subcommand)]
pub(crate) enum ProjectCommand {
    /// Change header fields.
    #[command(long_about = PROJECT_SET_ABOUT)]
    Set {
        #[arg(value_name = "PROJECT")]
        project: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long, value_parser = parse_status_string)]
        status: Option<String>,
        #[arg(long)]
        version: Option<String>,
        #[arg(long)]
        description: Option<String>,
        #[arg(long = "source-dir")]
        source_dir: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum PhaseCommand {
    #[command(long_about = PHASE_ADD_ABOUT)]
    Add {
        #[arg(value_name = "PROJECT")]
        project: String,
        #[arg(long)]
        title: String,
        #[arg(long)]
        id: Option<String>,
        #[arg(long)]
        goal: Option<String>,
    },
    #[command(long_about = PHASE_REMOVE_ABOUT)]
    Remove {
        #[arg(value_name = "PROJECT")]
        project: String,
        #[arg(value_name = "PHASE")]
        phase: String,
        #[arg(long)]
        force: bool,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum TaskCommand {
    #[command(long_about = TASK_ADD_ABOUT)]
    Add {
        #[arg(value_name = "PROJECT")]
        project: String,
        #[arg(value_name = "PHASE")]
        phase: String,
        #[arg(long)]
        title: String,
        #[arg(long)]
        id: Option<String>,
        #[arg(long)]
        status: Option<String>,
        #[arg(long)]
        due: Option<String>,
        #[arg(long)]
        note: Option<String>,
    },
    #[command(long_about = TASK_STATUS_ABOUT)]
    Status {
        #[arg(value_name = "PROJECT")]
        project: String,
        #[arg(value_name = "PHASE")]
        phase: String,
        #[arg(value_name = "TASK")]
        task: String,
        #[arg(value_name = "STATUS")]
        status: String,
    },
    #[command(long_about = TASK_DONE_ABOUT)]
    Done {
        #[arg(value_name = "PROJECT")]
        project: String,
        #[arg(value_name = "PHASE")]
        phase: String,
        #[arg(value_name = "TASK")]
        task: String,
    },
    #[command(long_about = TASK_REMOVE_ABOUT)]
    Remove {
        #[arg(value_name = "PROJECT")]
        project: String,
        #[arg(value_name = "PHASE")]
        phase: String,
        #[arg(value_name = "TASK")]
        task: String,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum IssueCommand {
    #[command(long_about = ISSUE_ADD_ABOUT)]
    Add {
        #[arg(value_name = "PROJECT")]
        project: String,
        #[arg(long)]
        title: String,
        #[arg(long)]
        severity: String,
        #[arg(long)]
        id: Option<String>,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        note: Option<String>,
    },
    #[command(long_about = ISSUE_STATUS_ABOUT)]
    Status {
        #[arg(value_name = "PROJECT")]
        project: String,
        #[arg(value_name = "ISSUE")]
        issue: String,
        #[arg(value_name = "STATUS")]
        status: String,
    },
    #[command(long_about = ISSUE_REMOVE_ABOUT)]
    Remove {
        #[arg(value_name = "PROJECT")]
        project: String,
        #[arg(value_name = "ISSUE")]
        issue: String,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum WatchCommand {
    #[command(long_about = WATCH_ADD_ABOUT)]
    Add {
        #[arg(value_name = "PROJECT")]
        project: String,
        #[arg(long)]
        kind: String,
        #[arg(long)]
        title: String,
        #[arg(long)]
        due: String,
        #[arg(long)]
        id: Option<String>,
        #[arg(long)]
        note: Option<String>,
    },
    #[command(long_about = WATCH_REMOVE_ABOUT)]
    Remove {
        #[arg(value_name = "PROJECT")]
        project: String,
        #[arg(value_name = "WATCH")]
        watch: String,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum FindingsCommand {
    #[command(long_about = FINDINGS_SHOW_ABOUT)]
    Show {
        #[arg(value_name = "ID", value_parser = parse_finding_id)]
        id: i64,
        #[arg(long)]
        json: bool,
    },
    #[command(long_about = FINDINGS_IGNORE_ABOUT)]
    Ignore {
        #[arg(value_name = "ID", value_parser = parse_finding_id)]
        id: i64,
        #[arg(long)]
        reason: String,
        #[arg(long)]
        until: Option<String>,
    },
    #[command(long_about = FINDINGS_REOPEN_ABOUT)]
    Reopen {
        #[arg(value_name = "ID", value_parser = parse_finding_id)]
        id: i64,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum StackCommand {
    #[command(long_about = STACK_ADD_ABOUT)]
    Add {
        #[arg(value_name = "PROJECT")]
        project: String,
        #[arg(long)]
        kind: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        version: Option<String>,
        #[arg(long)]
        note: Option<String>,
        #[arg(long = "eol-product")]
        eol_product: Option<String>,
    },
    #[command(long_about = STACK_SET_ABOUT)]
    Set {
        #[arg(value_name = "PROJECT")]
        project: String,
        #[arg(value_name = "NAME")]
        name: String,
        #[arg(long)]
        version: Option<String>,
        #[arg(long)]
        note: Option<String>,
        #[arg(long = "eol-product")]
        eol_product: Option<String>,
    },
    #[command(long_about = STACK_REMOVE_ABOUT)]
    Remove {
        #[arg(value_name = "PROJECT")]
        project: String,
        #[arg(value_name = "NAME")]
        name: String,
    },
    #[command(long_about = STACK_LOCKFILE_ABOUT)]
    Lockfile {
        #[command(subcommand)]
        command: StackLockfileCommand,
    },
    #[command(long_about = STACK_ANDROID_ABOUT)]
    Android {
        #[arg(value_name = "PROJECT")]
        project: String,
        #[arg(long = "target-sdk")]
        target_sdk: Option<String>,
        #[arg(long = "min-sdk")]
        min_sdk: Option<String>,
        #[arg(long = "form-factor")]
        form_factor: Option<String>,
        #[arg(long)]
        store: Option<String>,
        #[arg(long = "detect-from", action = clap::ArgAction::Append)]
        detect_from: Vec<String>,
        #[arg(long = "clear-detect-from")]
        clear_detect_from: bool,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum StackLockfileCommand {
    Add {
        #[arg(value_name = "PROJECT")]
        project: String,
        #[arg(value_name = "PATH")]
        path: String,
    },
    Remove {
        #[arg(value_name = "PROJECT")]
        project: String,
        #[arg(value_name = "PATH")]
        path: String,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum CliStateFilter {
    Open,
    Ignored,
    Resolved,
    All,
}

impl From<CliStateFilter> for StateFilter {
    fn from(value: CliStateFilter) -> Self {
        match value {
            CliStateFilter::Open => Self::Open,
            CliStateFilter::Ignored => Self::Ignored,
            CliStateFilter::Resolved => Self::Resolved,
            CliStateFilter::All => Self::All,
        }
    }
}

fn parse_status(text: &str) -> Result<ProjectStatus, String> {
    ProjectStatus::parse(text).ok_or_else(|| {
        format!(
            "unknown status: {text}; expected idea, in_development, released, maintenance, paused, or discontinued"
        )
    })
}

fn parse_status_string(text: &str) -> Result<String, String> {
    parse_status(text)?;
    Ok(text.to_owned())
}

fn parse_language(text: &str) -> Result<Language, String> {
    Language::parse(text).ok_or_else(|| format!("unknown language: {text}"))
}

fn parse_kind(value: &str) -> Result<String, String> {
    value.parse::<FindingKind>().map(|kind| kind.as_str().to_owned()).map_err(|_| {
        "--kind must be vulnerability, malicious, unmaintained, unsound, notice, eol, eol_soon, platform_deadline, or platform_deadline_soon"
            .to_owned()
    })
}

fn parse_finding_id(text: &str) -> Result<i64, String> {
    let id = text
        .parse::<i64>()
        .map_err(|_| "finding id must be a positive number".to_owned())?;
    if id <= 0 {
        return Err("finding id must be a positive number".to_owned());
    }
    Ok(id)
}

const LONG_ABOUT: &str = "\
Track project files and print a summary of the workspace.

Usage:
  craftward
  craftward --help
  craftward --version
  craftward [--workspace <FILE>] <COMMAND> [--help]

Commands include init, add, remove, list, craftward validate [PATH]...,
roadmap, show, project, phase, task, issue, watch, radar, findings, stack,
deadlines, and workspace path. An error names its location, for example
plan.phases[0].tasks[1].due.";

const AFTER_HELP: &str = "\
--workspace selects the workspace file. Otherwise CRAFTWARD_WORKSPACE is used,
then the default directory for this system.

Exit status:
  0  success, or radar found nothing
  1  a file or request failed
  2  the command line is wrong
  3  radar has an open vulnerability, malicious package, past support end,
     or past Play deadline. Notices count only with --strict";

const INIT_ABOUT: &str = "\
Create a project file with the required fields.

STATUS is idea, in_development, released, maintenance, paused, or
discontinued. The default is idea. DIR defaults to the current directory.
An existing craftward.json is not overwritten. The workspace list is not
changed.";

const ADD_ABOUT: &str = "\
Add a project file to the workspace.

PATH is a craftward.json file or a directory that contains one. The file
must be valid. A path or project id that is already listed is rejected.";

const REMOVE_ABOUT: &str = "\
Remove a project from the workspace. The project file is not deleted.";

const LIST_ABOUT: &str = "\
Show every project in the workspace.

Open tasks and open issues are those marked open or in_progress. The next
due date is the earliest task or watch-item date still outstanding. Done
and dropped tasks are left out. A missing or invalid project file is shown
with a note, and the command exits 1.";

const VALIDATE_ABOUT: &str = "\
Check project files.

Usage form: craftward validate [PATH]...

With no paths, every project in the workspace is checked. Each file is
reported as \"ok\" or as a list of errors. An error names its location, for
example plan.phases[0].tasks[1].due. A missing source directory or
lockfile is reported as a warning and does not change the exit status.";

const WORKSPACE_ABOUT: &str = "\
Show which workspace file is in use.";

const WORKSPACE_PATH_ABOUT: &str = "\
Print the workspace file in use and whether it came from --workspace,
CRAFTWARD_WORKSPACE, or the default location.";

const ROADMAP_ABOUT: &str = "\
Write a versioned HTML roadmap for one project.

ID selects a project from the workspace. PATH is a craftward.json file or
a directory that contains one. DIR defaults to the current directory.
--lang is en or de, and defaults to en. An existing file is never
overwritten.";

const SHOW_ABOUT: &str = "\
Print one project, including the ids used by later commands.

PROJECT is a workspace project id, a craftward.json file, or a directory
that contains one.";

const PROJECT_ABOUT: &str = "\
Change header fields of a project file.";

const PROJECT_SET_ABOUT: &str = "\
Change header fields of a project file.

At least one option is required. An empty --version or --description
removes that field.";

const PHASE_ABOUT: &str = "\
Add or remove a phase.";

const PHASE_ADD_ABOUT: &str = "\
Append a phase. The plan is created when the project has none.

Without --id, the id is formed from the title.";

const PHASE_REMOVE_ABOUT: &str = "\
Remove a phase. The phase must have no tasks. --force removes the phase
and its tasks.";

const TASK_ABOUT: &str = "\
Add, update, or remove a task.";

const TASK_ADD_ABOUT: &str = "\
Append a task to a phase. Without --id, the id is formed from the title.
STATUS defaults to open.";

const TASK_STATUS_ABOUT: &str = "\
Set a task's status. STATUS is open, in_progress, done, or dropped.";

const TASK_DONE_ABOUT: &str = "\
Mark a task done. This is task status with status done.";

const TASK_REMOVE_ABOUT: &str = "\
Remove a task from a phase.";

const ISSUE_ABOUT: &str = "\
Add, update, or remove an issue.";

const ISSUE_ADD_ABOUT: &str = "\
Append an issue. Its status is open. SEVERITY is low, medium, high, or
critical.";

const ISSUE_STATUS_ABOUT: &str = "\
Set an issue's status. STATUS is open, in_progress, resolved, or wont_fix.";

const ISSUE_REMOVE_ABOUT: &str = "\
Remove an issue.";

const WATCH_ABOUT: &str = "\
Add or remove a watch item.";

const WATCH_ADD_ABOUT: &str = "\
Append a watch item. KIND is deadline, expiry, review, or other.";

const WATCH_REMOVE_ABOUT: &str = "\
Remove a watch item.";

const RADAR_ABOUT: &str = "\
Look up known vulnerabilities for the packages in project lockfiles.

With no PROJECT, every project in the workspace is checked. --refresh
ignores cached answers. --offline never contacts OSV or endoflife.date.
--all includes ignored findings. --eol-warning-days defaults to 90.
--deadline-warning-days defaults to 180.";

const FINDINGS_ABOUT: &str = "\
List and change stored vulnerability findings.

With no PROJECT, every stored finding is listed. The default state is open.";

const FINDINGS_SHOW_ABOUT: &str = "\
Print one finding and its history.";

const FINDINGS_IGNORE_ABOUT: &str = "\
Ignore a finding. --reason is required. --until is the last UTC day the
ignore stays in force.";

const FINDINGS_REOPEN_ABOUT: &str = "\
Mark a finding open again.";

const STACK_ABOUT: &str = "\
Change the stack recorded in a project file.

Components are matched by name, ignoring letter case. An empty --version,
--note, --eol-product, --target-sdk, or --min-sdk removes that field.";

const STACK_ADD_ABOUT: &str = "\
Append a stack component.";

const STACK_SET_ABOUT: &str = "\
Change one stack component. An empty optional flag clears that field.";

const STACK_REMOVE_ABOUT: &str = "\
Remove one stack component.";

const STACK_LOCKFILE_ABOUT: &str = "\
Add or remove a lockfile path.";

const STACK_ANDROID_ABOUT: &str = "\
Create or change stack.android. --detect-from may be repeated.
--clear-detect-from drops the current list first.";

const DEADLINES_ABOUT: &str = "\
Show the Google Play target-API deadlines Craftward will use.

The built-in file is used unless the data directory contains
platform-deadlines.json with a newer data_version.";

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::Cli;

    #[test]
    fn clap_definition_is_consistent() {
        Cli::command().debug_assert();
    }
}
