//! Storage interface for radar findings and runs.
//!
//! The command-line tool and, later, a team server implement [`StateStore`].
//! This module does not open a database.

use std::path::PathBuf;

use serde::Serialize;

/// File name of the SQLite database inside the data directory.
pub const DATABASE_FILE: &str = "craftward.db";

/// Schema version this build knows how to write.
pub const SCHEMA_VERSION: i64 = 5;

/// Lifecycle state of one finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindingState {
    /// Still present and not ignored.
    Open,
    /// Hidden until it is reopened or the ignore date passes.
    Ignored,
    /// Absent from the latest successful scan.
    Resolved,
}

impl FindingState {
    /// Stable stored word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Ignored => "ignored",
            Self::Resolved => "resolved",
        }
    }

    /// Parses a stored word.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "open" => Some(Self::Open),
            "ignored" => Some(Self::Ignored),
            "resolved" => Some(Self::Resolved),
            _ => None,
        }
    }
}

/// Kind of a stored advisory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindingKind {
    /// A vulnerability that is not one of the more specific kinds.
    Vulnerability,
    /// A malicious package, including OSV ids that start with `MAL-`.
    Malicious,
    /// The package is no longer maintained.
    Unmaintained,
    /// The package can cause undefined behavior from safe code.
    Unsound,
    /// An informational notice, including an unrecognized notice kind.
    Notice,
    /// The release cycle's support end is in the past.
    Eol,
    /// The release cycle's support end is inside the warning window.
    EolSoon,
    /// A Play target-API deadline is today or in the past.
    PlatformDeadline,
    /// A Play target-API deadline is inside the warning window.
    PlatformDeadlineSoon,
}

impl FindingKind {
    /// Stable stored word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
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

    /// Parses a stored word.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "vulnerability" => Some(Self::Vulnerability),
            "malicious" => Some(Self::Malicious),
            "unmaintained" => Some(Self::Unmaintained),
            "unsound" => Some(Self::Unsound),
            "notice" => Some(Self::Notice),
            "eol" => Some(Self::Eol),
            "eol_soon" => Some(Self::EolSoon),
            "platform_deadline" => Some(Self::PlatformDeadline),
            "platform_deadline_soon" => Some(Self::PlatformDeadlineSoon),
            _ => None,
        }
    }

    /// Whether an open finding of this kind selects exit status 3.
    #[must_use]
    pub const fn affects_exit(self, strict: bool) -> bool {
        match self {
            Self::Vulnerability | Self::Malicious | Self::Eol | Self::PlatformDeadline => true,
            Self::Unmaintained
            | Self::Unsound
            | Self::Notice
            | Self::EolSoon
            | Self::PlatformDeadlineSoon => strict,
        }
    }

    /// Whether the text report lists this kind under notices.
    #[must_use]
    pub const fn is_notice(self) -> bool {
        matches!(
            self,
            Self::Unmaintained
                | Self::Unsound
                | Self::Notice
                | Self::EolSoon
                | Self::PlatformDeadlineSoon
        )
    }
}

/// Which findings a list command asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateFilter {
    /// Only open findings.
    Open,
    /// Only ignored findings.
    Ignored,
    /// Only resolved findings.
    Resolved,
    /// Every state.
    All,
}

/// One stored vulnerability for a project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredFinding {
    /// Database id used by later commands.
    pub id: i64,
    /// Project id from `craftward.json`.
    pub project_id: String,
    /// OSV ecosystem.
    pub ecosystem: String,
    /// Package name.
    pub package: String,
    /// OSV vulnerability id.
    pub vulnerability_id: String,
    /// Last version that was still affected.
    pub version: String,
    /// `low`, `medium`, `high`, `critical`, or `unknown`.
    pub severity: String,
    /// Short description.
    pub summary: String,
    /// Other ids, such as CVE ids.
    pub aliases: Vec<String>,
    /// Versions in which this package is fixed.
    pub fixed_versions: Vec<String>,
    /// Current state.
    pub state: FindingState,
    /// `vulnerability`, `malicious`, `unmaintained`, `unsound`, `notice`,
    /// `eol`, or `eol_soon`.
    pub kind: String,
    /// When the finding was first stored, `YYYY-MM-DDTHH:MM:SSZ`.
    pub first_seen: String,
    /// When a scan last observed it.
    pub last_seen: String,
    /// When it was marked resolved.
    pub resolved_at: Option<String>,
    /// Reason given to `findings ignore`.
    pub ignore_reason: Option<String>,
    /// Last UTC day the ignore is still in force, `YYYY-MM-DD`.
    pub ignore_until: Option<String>,
}

/// A finding that is not in the database yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewFinding {
    /// Project id.
    pub project_id: String,
    /// OSV ecosystem.
    pub ecosystem: String,
    /// Package name.
    pub package: String,
    /// OSV vulnerability id.
    pub vulnerability_id: String,
    /// Affected version.
    pub version: String,
    /// Severity word.
    pub severity: String,
    /// Short description.
    pub summary: String,
    /// Aliases.
    pub aliases: Vec<String>,
    /// Fixed versions.
    pub fixed_versions: Vec<String>,
    /// Advisory kind. See [`StoredFinding::kind`].
    pub kind: String,
    /// `first_seen` and `last_seen`.
    pub seen_at: String,
}

/// One row of a finding's history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StoredEvent {
    /// `discovered`, `ignored`, `reopened`, `resolved`, `reappeared`,
    /// `withdrawn`, or `deadline_passed`.
    pub kind: String,
    /// When the event was recorded.
    pub at: String,
    /// User text, or an empty string when the event has none.
    pub reason: String,
}

/// A radar run to append.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewRun {
    /// Project id.
    pub project_id: String,
    /// When the scan started.
    pub started_at: String,
    /// When the scan finished.
    pub finished_at: String,
    /// Whether the scan was not allowed to use the network.
    pub offline: bool,
    /// Packages sent to OSV. Zero when the project could not be checked.
    pub packages_checked: i64,
    /// Open findings after the run. Zero when the run failed.
    pub open_count: i64,
    /// Findings inserted by this run.
    pub new_count: i64,
    /// Findings marked resolved by this run.
    pub resolved_count: i64,
    /// Whether this run compared lockfiles or end-of-life data with findings.
    ///
    /// `false` with [`Self::error`] unset means nothing was evaluated.
    pub evaluated: bool,
    /// Why the project could not be checked. Unset when the check did not fail.
    pub error: Option<String>,
}

/// Why a storage call failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreError {
    /// No finding has this id.
    NotFound(i64),
    /// The file was written by a newer Craftward.
    NewerDatabase {
        /// Version stored in the file.
        found: i64,
        /// Newest version this build can open.
        supported: i64,
    },
    /// The backup file for this migration is already present.
    BackupExists(PathBuf),
    /// The request itself is not valid.
    Invalid(String),
    /// The database could not be read or written.
    Failed(String),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(id) => write!(formatter, "unknown finding: {id}"),
            Self::NewerDatabase { .. } => {
                formatter.write_str("database was written by a newer Craftward version")
            }
            Self::BackupExists(path) => {
                write!(formatter, "backup already exists: {}", path.display())
            }
            Self::Invalid(message) | Self::Failed(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for StoreError {}

/// Operations available inside one transaction.
pub trait StoreSession {
    /// Findings stored for one project.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the rows cannot be read.
    fn findings_for_project(&mut self, project_id: &str) -> Result<Vec<StoredFinding>, StoreError>;

    /// Inserts a finding and returns its id.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the row cannot be written.
    fn insert_finding(&mut self, finding: &NewFinding) -> Result<i64, StoreError>;

    /// Replaces the mutable columns of a finding.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the row cannot be written.
    fn update_finding(&mut self, finding: &StoredFinding) -> Result<(), StoreError>;

    /// Appends one history row.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the row cannot be written.
    fn insert_event(
        &mut self,
        finding_id: i64,
        kind: &str,
        at: &str,
        reason: &str,
    ) -> Result<(), StoreError>;

    /// Appends one radar run.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the row cannot be written.
    fn insert_run(&mut self, run: &NewRun) -> Result<(), StoreError>;

    /// Lists findings. An empty `projects` list means every project.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the rows cannot be read.
    fn list_findings(
        &mut self,
        projects: &[String],
        filter: StateFilter,
    ) -> Result<Vec<StoredFinding>, StoreError>;

    /// One finding, when the id exists.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the row cannot be read.
    fn finding(&mut self, id: i64) -> Result<Option<StoredFinding>, StoreError>;

    /// History of one finding, oldest first.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the rows cannot be read.
    fn events(&mut self, finding_id: i64) -> Result<Vec<StoredEvent>, StoreError>;
}

/// A database that can run a transaction.
pub trait StateStore {
    /// Runs `body` in one transaction and commits when it returns `Ok`.
    ///
    /// # Errors
    ///
    /// Returns the error from `body`, or an error opening or committing the
    /// transaction. A failure rolls the transaction back.
    fn in_transaction<T>(
        &mut self,
        body: impl FnOnce(&mut dyn StoreSession) -> Result<T, StoreError>,
    ) -> Result<T, StoreError>;
}
