//! Clap command-line definition for `craftward`.

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use craftward_core::{Language, ProjectStatus, StateFilter};

/// Track project files and print a summary of the workspace.
#[derive(Debug, Parser)]
#[command(
    name = "craftward",
    bin_name = "craftward",
    version,
    about = "Track project files and print a summary of the workspace.",
    after_help = AFTER_HELP,
    disable_help_subcommand = true,
    infer_subcommands = false
)]
pub(crate) struct Cli {
    /// Workspace file path (`CRAFTWARD_WORKSPACE` or the platform default when omitted).
    #[arg(
        long,
        global = true,
        value_name = "FILE",
        help_heading = "Global options"
    )]
    pub workspace: Option<PathBuf>,

    /// Directory for craftward.db and related data (`CRAFTWARD_DATA_DIR` or the platform default).
    #[arg(
        long,
        global = true,
        value_name = "DIR",
        help_heading = "Global options"
    )]
    pub data_dir: Option<PathBuf>,

    /// Directory for cached OSV and endoflife.date responses (`CRAFTWARD_CACHE_DIR` or the platform default).
    #[arg(
        long,
        global = true,
        value_name = "DIR",
        help_heading = "Global options"
    )]
    pub cache_dir: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Create DIR/craftward.json with the required fields.
    #[command(long_about = INIT_ABOUT)]
    Init {
        /// Directory for the new project file; defaults to the current directory.
        #[arg(value_name = "DIR")]
        directory: Option<PathBuf>,
        /// Project id (short name stored in craftward.json).
        #[arg(long)]
        id: String,
        /// Project display name.
        #[arg(long)]
        name: String,
        /// Project lifecycle status; defaults to idea.
        #[arg(long, value_enum, default_value_t = CliProjectStatus::Idea)]
        status: CliProjectStatus,
    },
    /// Add a craftward.json file, or a directory that contains one.
    #[command(long_about = ADD_ABOUT)]
    Add {
        /// Project file or directory that contains craftward.json.
        #[arg(value_name = "PATH")]
        path: PathBuf,
    },
    /// Remove a project from the workspace.
    #[command(long_about = REMOVE_ABOUT)]
    Remove {
        /// Workspace project id to remove from the registry.
        #[arg(value_name = "ID")]
        id: String,
        /// Also delete that project's craftward.json (never the folder).
        #[arg(long = "delete-file")]
        delete_file: bool,
    },
    /// Show every project in the workspace.
    #[command(long_about = LIST_ABOUT)]
    List,
    /// Check project files.
    #[command(long_about = VALIDATE_ABOUT)]
    Validate {
        /// Project files to check; empty means every workspace project.
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
        /// Workspace id, craftward.json path, or directory that contains one.
        #[arg(value_name = "ID|PATH")]
        target: String,
        /// Output directory; defaults to the current directory.
        #[arg(long = "dir", value_name = "DIR")]
        directory: Option<PathBuf>,
        /// Language for fixed labels (en or de).
        #[arg(long = "lang", value_parser = parse_language, default_value = "en")]
        language: Language,
    },
    /// Print one project, including the ids used by later commands.
    #[command(long_about = SHOW_ABOUT)]
    Show {
        /// Workspace id, craftward.json path, or directory that contains one.
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
    /// Add, update, or remove a watch item.
    #[command(long_about = WATCH_ABOUT)]
    Watch {
        #[command(subcommand)]
        command: WatchCommand,
    },
    /// Look up known vulnerabilities, support ends, and Play deadlines.
    #[command(long_about = RADAR_ABOUT)]
    Radar {
        /// Workspace ids or project paths; empty means every workspace project.
        #[arg(value_name = "PROJECT")]
        projects: Vec<String>,
        /// Ignore fresh cache entries and ask OSV / endoflife.date again.
        #[arg(long, conflicts_with = "offline")]
        refresh: bool,
        /// Never contact OSV or endoflife.date; use cache only.
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
        /// Days ahead that still count as `eol_soon` (default 90).
        #[arg(long, default_value_t = 90, value_name = "N")]
        eol_warning_days: u32,
        /// Days ahead that still count as `platform_deadline_soon` (default 180).
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
        /// Which finding states to print.
        #[arg(long, value_enum, default_value_t = CliStateFilter::Open)]
        state: CliStateFilter,
        /// Restrict the list to one finding kind.
        #[arg(long, value_enum)]
        kind: Option<CliFindingKind>,
        /// Print JSON instead of text.
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
    /// Show or change storage locations in the settings file.
    #[command(long_about = SETTINGS_ABOUT)]
    Settings {
        #[command(subcommand)]
        command: SettingsCommand,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum SettingsCommand {
    /// Print workspace, data, and cache paths with their origin.
    #[command(long_about = SETTINGS_SHOW_ABOUT)]
    Show,
    /// Change a storage location and relocate existing data when needed.
    #[command(long_about = SETTINGS_SET_ABOUT)]
    Set {
        #[command(subcommand)]
        command: SettingsSetCommand,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum SettingsSetCommand {
    /// Set the workspace.json path in settings and relocate the file.
    #[command(long_about = SETTINGS_SET_WORKSPACE_ABOUT)]
    Workspace {
        /// New workspace.json path.
        #[arg(value_name = "FILE")]
        path: PathBuf,
    },
    /// Set the data directory in settings and relocate craftward.db.
    #[command(name = "data-dir", long_about = SETTINGS_SET_DATA_ABOUT)]
    DataDir {
        /// New directory for craftward.db.
        #[arg(value_name = "DIR")]
        path: PathBuf,
    },
    /// Set the cache directory in settings (created empty).
    #[command(name = "cache-dir", long_about = SETTINGS_SET_CACHE_ABOUT)]
    CacheDir {
        /// New directory for cached HTTP responses.
        #[arg(value_name = "DIR")]
        path: PathBuf,
    },
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
        /// Workspace id, craftward.json path, or directory that contains one.
        #[arg(value_name = "PROJECT")]
        project: String,
        /// New display name.
        #[arg(long)]
        name: Option<String>,
        /// New lifecycle status.
        #[arg(long, value_enum)]
        status: Option<CliProjectStatus>,
        /// New version string; pass an empty value to clear.
        #[arg(long)]
        version: Option<String>,
        /// New description; pass an empty value to clear.
        #[arg(long)]
        description: Option<String>,
        /// New source directory; pass an empty value to clear.
        #[arg(long = "source-dir")]
        source_dir: Option<String>,
        /// Comma-separated tags; pass an empty value to clear.
        #[arg(long)]
        tags: Option<String>,
        /// Remove all tags.
        #[arg(long = "clear-tags")]
        clear_tags: bool,
        /// Link as LABEL=URL; repeat for several links (replaces the list).
        #[arg(long = "link", value_name = "LABEL=URL")]
        links: Vec<String>,
        /// Remove all links.
        #[arg(long = "clear-links")]
        clear_links: bool,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum PhaseCommand {
    /// Append a phase.
    #[command(long_about = PHASE_ADD_ABOUT)]
    Add {
        /// Workspace id, craftward.json path, or directory that contains one.
        #[arg(value_name = "PROJECT")]
        project: String,
        /// Phase title.
        #[arg(long)]
        title: String,
        /// Explicit phase id; formed from the title when omitted.
        #[arg(long)]
        id: Option<String>,
        /// Optional phase goal.
        #[arg(long)]
        goal: Option<String>,
    },
    /// Change a phase title or goal.
    #[command(long_about = PHASE_SET_ABOUT)]
    Set {
        /// Workspace id, craftward.json path, or directory that contains one.
        #[arg(value_name = "PROJECT")]
        project: String,
        /// Phase id to update.
        #[arg(value_name = "PHASE")]
        phase: String,
        /// New phase title.
        #[arg(long)]
        title: Option<String>,
        /// New goal; pass an empty value to clear.
        #[arg(long)]
        goal: Option<String>,
    },
    /// Remove a phase.
    #[command(long_about = PHASE_REMOVE_ABOUT)]
    Remove {
        /// Workspace id, craftward.json path, or directory that contains one.
        #[arg(value_name = "PROJECT")]
        project: String,
        /// Phase id to remove.
        #[arg(value_name = "PHASE")]
        phase: String,
        /// Remove the phase even when it still has tasks.
        #[arg(long)]
        force: bool,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum TaskCommand {
    /// Append a task to a phase.
    #[command(long_about = TASK_ADD_ABOUT)]
    Add {
        /// Workspace id, craftward.json path, or directory that contains one.
        #[arg(value_name = "PROJECT")]
        project: String,
        /// Phase id that receives the task.
        #[arg(value_name = "PHASE")]
        phase: String,
        /// Task title.
        #[arg(long)]
        title: String,
        /// Explicit task id; formed from the title when omitted.
        #[arg(long)]
        id: Option<String>,
        /// Task status; defaults to open.
        #[arg(long, value_enum)]
        status: Option<CliTaskStatus>,
        /// Due day as YYYY-MM-DD.
        #[arg(long, value_name = "YYYY-MM-DD")]
        due: Option<String>,
        /// Optional free-form note.
        #[arg(long)]
        note: Option<String>,
    },
    /// Change a task title, due day, or note.
    #[command(long_about = TASK_SET_ABOUT)]
    Set {
        /// Workspace id, craftward.json path, or directory that contains one.
        #[arg(value_name = "PROJECT")]
        project: String,
        /// Phase id that owns the task.
        #[arg(value_name = "PHASE")]
        phase: String,
        /// Task id to update.
        #[arg(value_name = "TASK")]
        task: String,
        /// New task title.
        #[arg(long)]
        title: Option<String>,
        /// Due day as YYYY-MM-DD; pass an empty value to clear.
        #[arg(long, value_name = "YYYY-MM-DD")]
        due: Option<String>,
        /// Note; pass an empty value to clear.
        #[arg(long)]
        note: Option<String>,
    },
    /// Set a task's status.
    #[command(long_about = TASK_STATUS_ABOUT)]
    Status {
        /// Workspace id, craftward.json path, or directory that contains one.
        #[arg(value_name = "PROJECT")]
        project: String,
        /// Phase id that owns the task.
        #[arg(value_name = "PHASE")]
        phase: String,
        /// Task id to update.
        #[arg(value_name = "TASK")]
        task: String,
        /// New task status.
        #[arg(value_name = "STATUS", value_enum)]
        status: CliTaskStatus,
    },
    /// Mark a task done.
    #[command(long_about = TASK_DONE_ABOUT)]
    Done {
        /// Workspace id, craftward.json path, or directory that contains one.
        #[arg(value_name = "PROJECT")]
        project: String,
        /// Phase id that owns the task.
        #[arg(value_name = "PHASE")]
        phase: String,
        /// Task id to mark done.
        #[arg(value_name = "TASK")]
        task: String,
    },
    /// Remove a task from a phase.
    #[command(long_about = TASK_REMOVE_ABOUT)]
    Remove {
        /// Workspace id, craftward.json path, or directory that contains one.
        #[arg(value_name = "PROJECT")]
        project: String,
        /// Phase id that owns the task.
        #[arg(value_name = "PHASE")]
        phase: String,
        /// Task id to remove.
        #[arg(value_name = "TASK")]
        task: String,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum IssueCommand {
    /// Append an issue.
    #[command(long_about = ISSUE_ADD_ABOUT)]
    Add {
        /// Workspace id, craftward.json path, or directory that contains one.
        #[arg(value_name = "PROJECT")]
        project: String,
        /// Issue title.
        #[arg(long)]
        title: String,
        /// Issue severity.
        #[arg(long, value_enum)]
        severity: CliIssueSeverity,
        /// Explicit issue id; formed from the title when omitted.
        #[arg(long)]
        id: Option<String>,
        /// Tracker URL starting with http:// or https://.
        #[arg(long)]
        url: Option<String>,
        /// Optional free-form note.
        #[arg(long)]
        note: Option<String>,
    },
    /// Change an issue title, severity, URL, or note.
    #[command(long_about = ISSUE_SET_ABOUT)]
    Set {
        /// Workspace id, craftward.json path, or directory that contains one.
        #[arg(value_name = "PROJECT")]
        project: String,
        /// Issue id to update.
        #[arg(value_name = "ISSUE")]
        issue: String,
        /// New issue title.
        #[arg(long)]
        title: Option<String>,
        /// New severity.
        #[arg(long, value_enum)]
        severity: Option<CliIssueSeverity>,
        /// Tracker URL; pass an empty value to clear.
        #[arg(long)]
        url: Option<String>,
        /// Note; pass an empty value to clear.
        #[arg(long)]
        note: Option<String>,
    },
    /// Set an issue's status.
    #[command(long_about = ISSUE_STATUS_ABOUT)]
    Status {
        /// Workspace id, craftward.json path, or directory that contains one.
        #[arg(value_name = "PROJECT")]
        project: String,
        /// Issue id to update.
        #[arg(value_name = "ISSUE")]
        issue: String,
        /// New issue status.
        #[arg(value_name = "STATUS", value_enum)]
        status: CliIssueStatus,
    },
    /// Remove an issue.
    #[command(long_about = ISSUE_REMOVE_ABOUT)]
    Remove {
        /// Workspace id, craftward.json path, or directory that contains one.
        #[arg(value_name = "PROJECT")]
        project: String,
        /// Issue id to remove.
        #[arg(value_name = "ISSUE")]
        issue: String,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum WatchCommand {
    /// Append a watch item.
    #[command(long_about = WATCH_ADD_ABOUT)]
    Add {
        /// Workspace id, craftward.json path, or directory that contains one.
        #[arg(value_name = "PROJECT")]
        project: String,
        /// Kind of dated reminder.
        #[arg(long, value_enum)]
        kind: CliWatchKind,
        /// Reminder title.
        #[arg(long)]
        title: String,
        /// Due day as YYYY-MM-DD.
        #[arg(long, value_name = "YYYY-MM-DD")]
        due: String,
        /// Explicit watch id; formed from the title when omitted.
        #[arg(long)]
        id: Option<String>,
        /// Optional free-form note.
        #[arg(long)]
        note: Option<String>,
    },
    /// Change a watch item's title, kind, due day, or note.
    #[command(long_about = WATCH_SET_ABOUT)]
    Set {
        /// Workspace id, craftward.json path, or directory that contains one.
        #[arg(value_name = "PROJECT")]
        project: String,
        /// Watch item id to update.
        #[arg(value_name = "WATCH")]
        watch: String,
        /// New reminder title.
        #[arg(long)]
        title: Option<String>,
        /// New kind of dated reminder.
        #[arg(long, value_enum)]
        kind: Option<CliWatchKind>,
        /// Due day as YYYY-MM-DD.
        #[arg(long, value_name = "YYYY-MM-DD")]
        due: Option<String>,
        /// Note; pass an empty value to clear.
        #[arg(long)]
        note: Option<String>,
    },
    /// Remove a watch item.
    #[command(long_about = WATCH_REMOVE_ABOUT)]
    Remove {
        /// Workspace id, craftward.json path, or directory that contains one.
        #[arg(value_name = "PROJECT")]
        project: String,
        /// Watch item id to remove.
        #[arg(value_name = "WATCH")]
        watch: String,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum FindingsCommand {
    /// Print one finding and its history.
    #[command(long_about = FINDINGS_SHOW_ABOUT)]
    Show {
        /// Stored finding id (positive integer).
        #[arg(value_name = "ID", value_parser = parse_finding_id)]
        id: i64,
        /// Print JSON instead of text.
        #[arg(long)]
        json: bool,
    },
    /// Ignore a finding.
    #[command(long_about = FINDINGS_IGNORE_ABOUT)]
    Ignore {
        /// Stored finding id (positive integer).
        #[arg(value_name = "ID", value_parser = parse_finding_id)]
        id: i64,
        /// Why the finding is ignored.
        #[arg(long)]
        reason: String,
        /// Last UTC day the ignore stays in force (YYYY-MM-DD).
        #[arg(long, value_name = "YYYY-MM-DD")]
        until: Option<String>,
    },
    /// Mark a finding open again.
    #[command(long_about = FINDINGS_REOPEN_ABOUT)]
    Reopen {
        /// Stored finding id (positive integer).
        #[arg(value_name = "ID", value_parser = parse_finding_id)]
        id: i64,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum StackCommand {
    /// Append a stack component.
    #[command(long_about = STACK_ADD_ABOUT)]
    Add {
        /// Workspace id, craftward.json path, or directory that contains one.
        #[arg(value_name = "PROJECT")]
        project: String,
        /// Component role in the stack.
        #[arg(long, value_enum)]
        kind: CliComponentKind,
        /// Component name (matched case-insensitively later).
        #[arg(long)]
        name: String,
        /// Optional version string.
        #[arg(long)]
        version: Option<String>,
        /// Optional free-form note.
        #[arg(long)]
        note: Option<String>,
        /// endoflife.date product id for support checks.
        #[arg(long = "eol-product")]
        eol_product: Option<String>,
    },
    /// Change one stack component.
    #[command(long_about = STACK_SET_ABOUT)]
    Set {
        /// Workspace id, craftward.json path, or directory that contains one.
        #[arg(value_name = "PROJECT")]
        project: String,
        /// Component name to change.
        #[arg(value_name = "NAME")]
        name: String,
        /// New version; pass an empty value to clear.
        #[arg(long)]
        version: Option<String>,
        /// New note; pass an empty value to clear.
        #[arg(long)]
        note: Option<String>,
        /// New endoflife.date product id; pass an empty value to clear.
        #[arg(long = "eol-product")]
        eol_product: Option<String>,
    },
    /// Remove one stack component.
    #[command(long_about = STACK_REMOVE_ABOUT)]
    Remove {
        /// Workspace id, craftward.json path, or directory that contains one.
        #[arg(value_name = "PROJECT")]
        project: String,
        /// Component name to remove.
        #[arg(value_name = "NAME")]
        name: String,
    },
    /// Add or remove a lockfile path.
    #[command(long_about = STACK_LOCKFILE_ABOUT)]
    Lockfile {
        #[command(subcommand)]
        command: StackLockfileCommand,
    },
    /// Create or change stack.android.
    #[command(long_about = STACK_ANDROID_ABOUT)]
    Android {
        /// Workspace id, craftward.json path, or directory that contains one.
        #[arg(value_name = "PROJECT")]
        project: String,
        /// Android targetSdk; pass an empty value to clear.
        #[arg(long = "target-sdk")]
        target_sdk: Option<String>,
        /// Android minSdk; pass an empty value to clear.
        #[arg(long = "min-sdk")]
        min_sdk: Option<String>,
        /// Device form factor (phone, automotive, wear, tv, or xr).
        #[arg(long = "form-factor", value_enum)]
        form_factor: Option<CliFormFactor>,
        /// Distribution store (`google_play` or `none`).
        #[arg(long, value_enum)]
        store: Option<CliAndroidStore>,
        /// Relative path used to detect targetSdk; may be repeated.
        #[arg(long = "detect-from", action = clap::ArgAction::Append)]
        detect_from: Vec<String>,
        /// Drop the current detect-from list before appending new paths.
        #[arg(long = "clear-detect-from")]
        clear_detect_from: bool,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum StackLockfileCommand {
    /// Append a lockfile path.
    Add {
        /// Workspace id, craftward.json path, or directory that contains one.
        #[arg(value_name = "PROJECT")]
        project: String,
        /// Relative lockfile path inside the project.
        #[arg(value_name = "PATH")]
        path: String,
    },
    /// Remove a lockfile path.
    Remove {
        /// Workspace id, craftward.json path, or directory that contains one.
        #[arg(value_name = "PROJECT")]
        project: String,
        /// Relative lockfile path to remove.
        #[arg(value_name = "PATH")]
        path: String,
    },
}

/// Finding state filter for `findings`.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum CliStateFilter {
    /// Open findings.
    Open,
    /// Ignored findings.
    Ignored,
    /// Resolved findings.
    Resolved,
    /// Every stored state.
    All,
}

/// Project lifecycle status.
#[derive(Debug, Clone, Copy, ValueEnum)]
#[value(rename_all = "snake_case")]
pub(crate) enum CliProjectStatus {
    Idea,
    InDevelopment,
    Released,
    Maintenance,
    Paused,
    Discontinued,
}

/// Task status word.
#[derive(Debug, Clone, Copy, ValueEnum)]
#[value(rename_all = "snake_case")]
pub(crate) enum CliTaskStatus {
    Open,
    InProgress,
    Done,
    Dropped,
}

/// Issue status word.
#[derive(Debug, Clone, Copy, ValueEnum)]
#[value(rename_all = "snake_case")]
pub(crate) enum CliIssueStatus {
    Open,
    InProgress,
    Resolved,
    WontFix,
}

/// Issue severity word.
#[derive(Debug, Clone, Copy, ValueEnum)]
#[value(rename_all = "snake_case")]
pub(crate) enum CliIssueSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Watch item kind.
#[derive(Debug, Clone, Copy, ValueEnum)]
#[value(rename_all = "snake_case")]
pub(crate) enum CliWatchKind {
    Deadline,
    Expiry,
    Review,
    Other,
}

/// Stack component kind.
#[derive(Debug, Clone, Copy, ValueEnum)]
#[value(rename_all = "snake_case")]
pub(crate) enum CliComponentKind {
    Platform,
    Language,
    Framework,
    Runtime,
    Sdk,
    Service,
    Tool,
    Other,
}

/// Android form factor.
#[derive(Debug, Clone, Copy, ValueEnum)]
#[value(rename_all = "snake_case")]
pub(crate) enum CliFormFactor {
    Phone,
    Automotive,
    Wear,
    Tv,
    Xr,
}

/// Android store.
#[derive(Debug, Clone, Copy, ValueEnum)]
#[value(rename_all = "snake_case")]
pub(crate) enum CliAndroidStore {
    GooglePlay,
    None,
}

/// Finding kind filter.
#[derive(Debug, Clone, Copy, ValueEnum)]
#[value(rename_all = "snake_case")]
pub(crate) enum CliFindingKind {
    Vulnerability,
    Malicious,
    Unmaintained,
    Unsound,
    Notice,
    Eol,
    EolSoon,
    PlatformDeadline,
    PlatformDeadlineSoon,
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

impl CliProjectStatus {
    /// Schema word stored in craftward.json.
    #[must_use]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Idea => "idea",
            Self::InDevelopment => "in_development",
            Self::Released => "released",
            Self::Maintenance => "maintenance",
            Self::Paused => "paused",
            Self::Discontinued => "discontinued",
        }
    }
}

impl From<CliProjectStatus> for ProjectStatus {
    fn from(value: CliProjectStatus) -> Self {
        match value {
            CliProjectStatus::Idea => Self::Idea,
            CliProjectStatus::InDevelopment => Self::InDevelopment,
            CliProjectStatus::Released => Self::Released,
            CliProjectStatus::Maintenance => Self::Maintenance,
            CliProjectStatus::Paused => Self::Paused,
            CliProjectStatus::Discontinued => Self::Discontinued,
        }
    }
}

impl CliTaskStatus {
    /// Schema word stored in craftward.json.
    #[must_use]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::InProgress => "in_progress",
            Self::Done => "done",
            Self::Dropped => "dropped",
        }
    }
}

impl CliIssueStatus {
    /// Schema word stored in craftward.json.
    #[must_use]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::InProgress => "in_progress",
            Self::Resolved => "resolved",
            Self::WontFix => "wont_fix",
        }
    }
}

impl CliIssueSeverity {
    /// Schema word stored in craftward.json.
    #[must_use]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}

impl CliWatchKind {
    /// Schema word stored in craftward.json.
    #[must_use]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Deadline => "deadline",
            Self::Expiry => "expiry",
            Self::Review => "review",
            Self::Other => "other",
        }
    }
}

impl CliComponentKind {
    /// Schema word stored in craftward.json.
    #[must_use]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Platform => "platform",
            Self::Language => "language",
            Self::Framework => "framework",
            Self::Runtime => "runtime",
            Self::Sdk => "sdk",
            Self::Service => "service",
            Self::Tool => "tool",
            Self::Other => "other",
        }
    }
}

impl CliFormFactor {
    /// Schema word stored in craftward.json.
    #[must_use]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Phone => "phone",
            Self::Automotive => "automotive",
            Self::Wear => "wear",
            Self::Tv => "tv",
            Self::Xr => "xr",
        }
    }
}

impl CliAndroidStore {
    /// Schema word stored in craftward.json.
    #[must_use]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::GooglePlay => "google_play",
            Self::None => "none",
        }
    }
}

impl CliFindingKind {
    /// Schema word used by the findings filter.
    #[must_use]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Vulnerability => "vulnerability",
            Self::Malicious => "malicious",
            Self::Unmaintained => "unmaintained",
            Self::Unsound => "unsound",
            Self::Notice => "notice",
            Self::Eol => "eol",
            Self::EolSoon => "eol_soon",
            Self::PlatformDeadline => "platform_deadline",
            Self::PlatformDeadlineSoon => "platform_deadline_soon",
        }
    }
}

fn parse_language(text: &str) -> Result<Language, String> {
    Language::parse(text).ok_or_else(|| format!("unknown language: {text}; expected en or de"))
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

const AFTER_HELP: &str = "\
Storage paths resolve in this order: command-line option, environment
variable, settings file, then the default directory for this system.

Exit status:
  0  success, or radar found nothing
  1  a file or request failed
  2  the command line is wrong
  3  radar has an open vulnerability, malicious package, past support end,
     or past Play deadline. Notices count only with --strict";

const INIT_ABOUT: &str = "\
STATUS is idea, in_development, released, maintenance, paused, or
discontinued. The default is idea. DIR defaults to the current directory.
An existing craftward.json is not overwritten. The workspace list is not
changed.";

const ADD_ABOUT: &str = "\
PATH is a craftward.json file or a directory that contains one. The file
must be valid. A path or project id that is already listed is rejected.";

const REMOVE_ABOUT: &str = "\
Without --delete-file the project file stays on disk. --delete-file removes
only that craftward.json after updating the workspace; the folder and every
other file are left alone.";

const LIST_ABOUT: &str = "\
Open tasks and open issues are those marked open or in_progress. The next
due date is the earliest task or watch-item date still outstanding. Done
and dropped tasks are left out. A missing or invalid project file is shown
with a note, and the command exits 1.";

const VALIDATE_ABOUT: &str = "\
With no paths, every project in the workspace is checked. Each file is
reported as \"ok\" or as a list of errors. An error names its location, for
example plan.phases[0].tasks[1].due. A missing source directory or
lockfile is reported as a warning and does not change the exit status.";

const WORKSPACE_ABOUT: &str = "\
The path subcommand prints which workspace file is in use.";

const WORKSPACE_PATH_ABOUT: &str = "\
Prints whether the file came from --workspace, CRAFTWARD_WORKSPACE, the
settings file, or the default location.";

const SETTINGS_ABOUT: &str = "\
Show or change the workspace file, data directory, and cache directory
stored in settings.json. Paths fixed by a command-line option or
environment variable cannot be changed here.";

const SETTINGS_SHOW_ABOUT: &str = "\
Prints each storage location and whether it came from a command-line
option, an environment variable, the settings file, or the default.";

const SETTINGS_SET_ABOUT: &str = "\
Use workspace, data-dir, or cache-dir with a positional path. The global
--workspace, --data-dir, and --cache-dir options keep their usual meaning
(override for this invocation) and are not the destination. A location
fixed by a command-line option or environment variable cannot be changed.";

const SETTINGS_SET_WORKSPACE_ABOUT: &str = "\
FILE is the new workspace.json path. An existing file is copied there,
checked, then removed from the old path. A target that already exists is
refused.";

const SETTINGS_SET_DATA_ABOUT: &str = "\
DIR is the new directory for craftward.db. The database is copied with
SQLite's backup API and must pass integrity_check. A target that already
exists is refused. The old database is removed only after settings are
saved.";

const SETTINGS_SET_CACHE_ABOUT: &str = "\
DIR is the new cache directory. It is created empty; the previous cache
is not copied.";

const ROADMAP_ABOUT: &str = "\
ID selects a project from the workspace. PATH is a craftward.json file or
a directory that contains one. DIR defaults to the current directory.
--lang is en or de, and defaults to en. An existing file is never
overwritten.";

const SHOW_ABOUT: &str = "\
PROJECT is a workspace project id, a craftward.json file, or a directory
that contains one.";

const PROJECT_ABOUT: &str = "\
Use the set subcommand to change name, status, version, description,
source_dir, tags, or links.";

const PROJECT_SET_ABOUT: &str = "\
At least one option is required. An empty --version, --description,
--source-dir, or --tags removes that field. --link LABEL=URL may be
repeated and replaces the whole links list.";

const PHASE_ABOUT: &str = "\
Use add, set, or remove.";

const PHASE_ADD_ABOUT: &str = "\
The plan is created when the project has none. Without --id, the id is
formed from the title.";

const PHASE_SET_ABOUT: &str = "\
At least one of --title or --goal is required. An empty --goal clears
the goal.";

const PHASE_REMOVE_ABOUT: &str = "\
The phase must have no tasks. --force removes the phase and its tasks.";

const TASK_ABOUT: &str = "\
Use add, set, status, done, or remove.";

const TASK_ADD_ABOUT: &str = "\
Without --id, the id is formed from the title. STATUS defaults to open.";

const TASK_SET_ABOUT: &str = "\
At least one of --title, --due, or --note is required. An empty --due or
--note clears that field.";

const TASK_STATUS_ABOUT: &str = "\
STATUS is open, in_progress, done, or dropped.";

const TASK_DONE_ABOUT: &str = "\
This is the same as task status with status done.";

const TASK_REMOVE_ABOUT: &str = "\
Removes the task from its phase.";

const ISSUE_ABOUT: &str = "\
Use add, set, status, or remove.";

const ISSUE_ADD_ABOUT: &str = "\
Its status is open. SEVERITY is low, medium, high, or critical.";

const ISSUE_SET_ABOUT: &str = "\
At least one of --title, --severity, --url, or --note is required. An
empty --url or --note clears that field.";

const ISSUE_STATUS_ABOUT: &str = "\
STATUS is open, in_progress, resolved, or wont_fix.";

const ISSUE_REMOVE_ABOUT: &str = "\
Removes the issue from the project.";

const WATCH_ABOUT: &str = "\
Use add, set, or remove.";

const WATCH_ADD_ABOUT: &str = "\
KIND is deadline, expiry, review, or other.";

const WATCH_SET_ABOUT: &str = "\
At least one of --title, --kind, --due, or --note is required. An empty
--note clears that field.";

const WATCH_REMOVE_ABOUT: &str = "\
Removes the watch item from the project.";

const RADAR_ABOUT: &str = "\
With no PROJECT, every project in the workspace is checked. --refresh
ignores cached answers. --offline never contacts OSV or endoflife.date.
--all includes ignored findings. --eol-warning-days defaults to 90.
--deadline-warning-days defaults to 180.";

const FINDINGS_ABOUT: &str = "\
With no PROJECT, every stored finding is listed. The default state is open.
Use show, ignore, or reopen for one finding.";

const FINDINGS_SHOW_ABOUT: &str = "\
Includes the finding's event history.";

const FINDINGS_IGNORE_ABOUT: &str = "\
--reason is required. --until is the last UTC day the ignore stays in
force.";

const FINDINGS_REOPEN_ABOUT: &str = "\
Clears an ignore so the finding is open again.";

const STACK_ABOUT: &str = "\
Components are matched by name, ignoring letter case. An empty --version,
--note, --eol-product, --target-sdk, or --min-sdk removes that field.";

const STACK_ADD_ABOUT: &str = "\
Requires --kind and --name.";

const STACK_SET_ABOUT: &str = "\
An empty optional flag clears that field.";

const STACK_REMOVE_ABOUT: &str = "\
Removes the component with that name.";

const STACK_LOCKFILE_ABOUT: &str = "\
Use add or remove with a project and a relative path.";

const STACK_ANDROID_ABOUT: &str = "\
--detect-from may be repeated. --clear-detect-from drops the current list
first.";

const DEADLINES_ABOUT: &str = "\
The built-in file is used unless the data directory contains
platform-deadlines.json with a newer data_version.";

#[cfg(test)]
mod tests {
    use clap::{Command, CommandFactory};

    use super::Cli;

    #[test]
    fn clap_definition_is_consistent() {
        Cli::command().debug_assert();
    }

    #[test]
    fn help_uses_craftward_once_per_usage_and_never_exe() {
        assert_help_clean(Cli::command());
    }

    #[test]
    fn every_argument_and_option_has_a_non_empty_description() {
        assert_args_documented(&Cli::command(), "craftward");
    }

    fn assert_help_clean(mut command: Command) {
        for help in [command.render_help(), command.render_long_help()] {
            let text = help.to_string();
            assert_eq!(
                text.matches("Usage:").count(),
                1,
                "expected one Usage: in help for `{}`:\n{text}",
                command.get_name()
            );
            assert!(
                !text.contains("craftward.exe"),
                "help for `{}` must not contain craftward.exe:\n{text}",
                command.get_name()
            );
        }
        let children: Vec<Command> = command.get_subcommands().cloned().collect();
        for child in children {
            assert_help_clean(child);
        }
    }

    fn assert_args_documented(command: &Command, path: &str) {
        for arg in command.get_arguments() {
            let id = arg.get_id().as_str();
            if id == "version" || id == "help" {
                continue;
            }
            let help = arg
                .get_help()
                .map_or_default(std::string::ToString::to_string);
            let long = arg
                .get_long_help()
                .map_or_default(std::string::ToString::to_string);
            assert!(
                !help.trim().is_empty() || !long.trim().is_empty(),
                "missing help text for argument `{id}` on `{path}`"
            );
        }
        for child in command.get_subcommands() {
            let child_path = format!("{path} {}", child.get_name());
            assert_args_documented(child, &child_path);
        }
    }
}
