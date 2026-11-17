//! Schema versions 1 and 2 of a Craftward project file.

use serde::{Deserialize, Serialize};

/// A project described by one `craftward.json` file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Project {
    /// Format generation. This build reads `1` and `2`. Version `2` is written
    /// only when [`Self::category`] is set.
    pub schema_version: u32,
    /// Stable short name used in paths and cross-references.
    ///
    /// One to 64 characters from `a-z`, `0-9`, and `-`. It does not start or
    /// end with `-`.
    pub id: String,
    /// Human-readable project name. It must contain a non-whitespace character.
    pub name: String,
    /// Where the project is in its lifecycle.
    pub status: ProjectStatus,
    /// Directory that holds the project's source code.
    ///
    /// A relative path is resolved from the project file. An absolute path is
    /// used as given. `..` is allowed, because the project file may live
    /// outside the repository. When this is absent, the directory that
    /// contains the project file is the source directory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_dir: Option<String>,
    /// Longer description of the project.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Version the maintainer currently ships or is working on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_version: Option<String>,
    /// Optional single portfolio category (schema version 2).
    ///
    /// After normalization: trimmed, internal whitespace collapsed, 1–40
    /// characters, no control characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// Labels used to group projects. Exact duplicates are rejected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// Day the file was last updated, as `YYYY-MM-DD`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated: Option<String>,
    /// Links a person would open for this project.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub links: Option<Vec<Link>>,
    /// Languages, tools, and lockfiles that make up the project.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stack: Option<Stack>,
    /// Planned phases and their tasks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<Plan>,
    /// Bugs and other tracked problems.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issues: Option<Vec<Issue>>,
    /// Dates the maintainer wants to be reminded about.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub watch_items: Option<Vec<WatchItem>>,
}

/// Lifecycle status of a project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ProjectStatus {
    /// The project has not been started.
    Idea,
    /// The project is being built.
    InDevelopment,
    /// A version has been published.
    Released,
    /// The project is kept working, without major new work.
    Maintenance,
    /// Work has stopped for now and may resume.
    Paused,
    /// The project will not be continued.
    Discontinued,
}

impl ProjectStatus {
    /// Parses a schema-version-1 status word.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "idea" => Some(Self::Idea),
            "in_development" => Some(Self::InDevelopment),
            "released" => Some(Self::Released),
            "maintenance" => Some(Self::Maintenance),
            "paused" => Some(Self::Paused),
            "discontinued" => Some(Self::Discontinued),
            _ => None,
        }
    }

    /// The schema-version-1 word for this status.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Idea => "idea",
            Self::InDevelopment => "in_development",
            Self::Released => "released",
            Self::Maintenance => "maintenance",
            Self::Paused => "paused",
            Self::Discontinued => "discontinued",
        }
    }

    /// Portfolio / `list` sort rank (lower first).
    ///
    /// Order: released, maintenance, `in_development`, paused, idea, discontinued.
    #[must_use]
    pub const fn sort_rank(self) -> u8 {
        match self {
            Self::Released => 0,
            Self::Maintenance => 1,
            Self::InDevelopment => 2,
            Self::Paused => 3,
            Self::Idea => 4,
            Self::Discontinued => 5,
        }
    }
}

/// A labeled web address.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Link {
    /// Text shown for the link.
    pub label: String,
    /// Address. It must start with `http://` or `https://`.
    pub url: String,
}

/// The technical stack recorded for a project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Stack {
    /// Named pieces of the stack, such as a language or a hosted service.
    pub components: Vec<Component>,
    /// Lockfiles or other dependency manifests, relative to `source_dir`.
    ///
    /// When `source_dir` is absent, they are relative to the project file's
    /// directory. Each path must be relative. Absolute paths and `..`
    /// segments are rejected.
    pub lockfiles: Vec<String>,
    /// Android `targetSdk` settings. Absent means Play deadlines are not checked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub android: Option<AndroidConfig>,
}

/// Android settings used by the Play target-API check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AndroidConfig {
    /// `targetSdk` to compare with Play deadlines. When set, files are not read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_sdk: Option<u32>,
    /// `minSdk` recorded for the project. The Play check does not use it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_sdk: Option<u32>,
    /// Which Play device rules apply. The default is [`FormFactor::Phone`].
    #[serde(default, skip_serializing_if = "is_phone")]
    pub form_factor: FormFactor,
    /// `google_play` runs the check. `none` skips it. The default is Google Play.
    #[serde(default, skip_serializing_if = "is_google_play")]
    pub store: AndroidStore,
    /// Gradle and version-catalog files, relative to `source_dir`.
    ///
    /// The same rules as `lockfiles`: relative, and no `..` segment.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub detect_from: Vec<String>,
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_phone(value: &FormFactor) -> bool {
    *value == FormFactor::Phone
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_google_play(value: &AndroidStore) -> bool {
    *value == AndroidStore::GooglePlay
}

/// Device family whose Play deadline applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum FormFactor {
    /// Phones and tablets. This is the default.
    #[default]
    Phone,
    /// Android Automotive OS.
    Automotive,
    /// Wear OS.
    Wear,
    /// Android TV.
    Tv,
    /// Android XR.
    Xr,
}

impl FormFactor {
    /// Stable stored word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Phone => "phone",
            Self::Automotive => "automotive",
            Self::Wear => "wear",
            Self::Tv => "tv",
            Self::Xr => "xr",
        }
    }

    /// Parses a stored word.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "phone" => Some(Self::Phone),
            "automotive" => Some(Self::Automotive),
            "wear" => Some(Self::Wear),
            "tv" => Some(Self::Tv),
            "xr" => Some(Self::Xr),
            _ => None,
        }
    }
}

/// Whether the Play deadline check runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum AndroidStore {
    /// Compare `targetSdk` with the Google Play deadlines.
    #[default]
    GooglePlay,
    /// Do not compare `targetSdk`.
    None,
}

impl AndroidStore {
    /// Stable stored word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GooglePlay => "google_play",
            Self::None => "none",
        }
    }

    /// Parses a stored word.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "google_play" => Some(Self::GooglePlay),
            "none" => Some(Self::None),
            _ => None,
        }
    }
}

/// One piece of a project's stack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Component {
    /// What role this component plays.
    pub kind: ComponentKind,
    /// Component name, such as `Rust` or `PostgreSQL`.
    pub name: String,
    /// Version the project uses, when that is known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Free-form note about how the component is used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Product id on endoflife.date, when the name is not a unique match.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eol_product: Option<String>,
}

/// Role of a stack component.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ComponentKind {
    /// An operating system or app store platform.
    Platform,
    /// A programming language.
    Language,
    /// A framework or application library that shapes the program.
    Framework,
    /// A runtime the program executes on.
    Runtime,
    /// A vendor SDK.
    Sdk,
    /// A hosted or local service the program depends on.
    Service,
    /// A development or build tool.
    Tool,
    /// A component that does not fit the other roles.
    Other,
}

/// Phased plan for the work that remains.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Plan {
    /// Ordered phases. Phase ids are unique within the file.
    pub phases: Vec<Phase>,
}

/// One phase of a plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Phase {
    /// Short name of the phase, unique among phases in this file.
    pub id: String,
    /// Human-readable phase title.
    pub title: String,
    /// What this phase is meant to achieve.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goal: Option<String>,
    /// Tasks in this phase. Task ids are unique within the phase.
    pub tasks: Vec<Task>,
}

/// One task inside a phase.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Task {
    /// Short name of the task, unique among tasks in its phase.
    pub id: String,
    /// Human-readable task title.
    pub title: String,
    /// Whether the task is still open.
    pub status: TaskStatus,
    /// Day the task is due, as `YYYY-MM-DD`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due: Option<String>,
    /// Free-form note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Progress of a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Nobody has started the task.
    Open,
    /// The task is underway.
    InProgress,
    /// The task is finished.
    Done,
    /// The task will not be done.
    Dropped,
}

/// A bug or other tracked problem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Issue {
    /// Short name of the issue, unique among issues in this file.
    pub id: String,
    /// Human-readable issue title.
    pub title: String,
    /// How severe the issue is.
    pub severity: IssueSeverity,
    /// Whether the issue is still open.
    pub status: IssueStatus,
    /// Tracker address. It must start with `http://` or `https://` when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Free-form note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Severity of an issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum IssueSeverity {
    /// Small impact.
    Low,
    /// Noticeable impact.
    Medium,
    /// Should be handled soon.
    High,
    /// Needs attention before other work.
    Critical,
}

/// Progress of an issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum IssueStatus {
    /// The issue is not being worked on.
    Open,
    /// Someone is working on the issue.
    InProgress,
    /// The issue is fixed.
    Resolved,
    /// The issue will not be fixed.
    WontFix,
}

/// A dated reminder that is not a task or an issue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct WatchItem {
    /// Short name of the reminder, unique among watch items in this file.
    pub id: String,
    /// What kind of date this is.
    pub kind: WatchKind,
    /// Human-readable reminder title.
    pub title: String,
    /// The day this reminder is about, as `YYYY-MM-DD`.
    pub due: String,
    /// Free-form note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Kind of dated reminder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum WatchKind {
    /// A date by which something must happen.
    Deadline,
    /// A date on which something stops being valid.
    Expiry,
    /// A date on which the project should be reviewed.
    Review,
    /// A dated reminder of another kind.
    Other,
}
