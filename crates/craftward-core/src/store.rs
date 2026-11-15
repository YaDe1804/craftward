//! Storage interface for radar findings and runs.
//!
//! The command-line tool and, later, a team server implement [`StateStore`].
//! This module does not open a database.

use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// File name of the SQLite database inside the data directory.
pub const DATABASE_FILE: &str = "craftward.db";

/// Schema version this build knows how to write.
pub const SCHEMA_VERSION: i64 = 6;

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
}

impl FromStr for FindingState {
    type Err = UnknownDomainValue;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text {
            "open" => Ok(Self::Open),
            "ignored" => Ok(Self::Ignored),
            "resolved" => Ok(Self::Resolved),
            _ => Err(UnknownDomainValue::new(text)),
        }
    }
}

impl fmt::Display for FindingState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// How severe a finding is. More severe values sort first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// The highest severity.
    Critical,
    /// Higher than medium.
    High,
    /// The middle severity.
    Medium,
    /// Lower than medium.
    Low,
    /// No recognized severity word.
    Unknown,
}

impl Severity {
    /// Stable stored word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Critical => "critical",
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
            Self::Unknown => "unknown",
        }
    }

    /// Every value, from most severe to least.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::Critical,
            Self::High,
            Self::Medium,
            Self::Low,
            Self::Unknown,
        ]
    }
}

impl FromStr for Severity {
    type Err = UnknownDomainValue;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text {
            "critical" => Ok(Self::Critical),
            "high" => Ok(Self::High),
            "medium" => Ok(Self::Medium),
            "low" => Ok(Self::Low),
            "unknown" => Ok(Self::Unknown),
            _ => Err(UnknownDomainValue::new(text)),
        }
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Kind of a stored advisory.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingKind {
    /// A vulnerability that is not one of the more specific kinds.
    #[default]
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

    /// Every stored kind.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::Vulnerability,
            Self::Malicious,
            Self::Unmaintained,
            Self::Unsound,
            Self::Notice,
            Self::Eol,
            Self::EolSoon,
            Self::PlatformDeadline,
            Self::PlatformDeadlineSoon,
        ]
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

impl FromStr for FindingKind {
    type Err = UnknownDomainValue;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text {
            "vulnerability" => Ok(Self::Vulnerability),
            "malicious" => Ok(Self::Malicious),
            "unmaintained" => Ok(Self::Unmaintained),
            "unsound" => Ok(Self::Unsound),
            "notice" => Ok(Self::Notice),
            "eol" => Ok(Self::Eol),
            "eol_soon" => Ok(Self::EolSoon),
            "platform_deadline" => Ok(Self::PlatformDeadline),
            "platform_deadline_soon" => Ok(Self::PlatformDeadlineSoon),
            _ => Err(UnknownDomainValue::new(text)),
        }
    }
}

impl fmt::Display for FindingKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// One row in a finding's history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    /// The finding was stored for the first time.
    Discovered,
    /// A person ignored the finding.
    Ignored,
    /// An ignore ended, or a person reopened the finding.
    Reopened,
    /// The finding was absent from a check that could have seen it.
    Resolved,
    /// A resolved finding was seen again.
    Reappeared,
    /// The advisory record was withdrawn.
    Withdrawn,
    /// A Play warning became a missed deadline.
    DeadlinePassed,
}

impl EventKind {
    /// Stable stored word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Discovered => "discovered",
            Self::Ignored => "ignored",
            Self::Reopened => "reopened",
            Self::Resolved => "resolved",
            Self::Reappeared => "reappeared",
            Self::Withdrawn => "withdrawn",
            Self::DeadlinePassed => "deadline_passed",
        }
    }

    /// Every stored event kind.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::Discovered,
            Self::Ignored,
            Self::Reopened,
            Self::Resolved,
            Self::Reappeared,
            Self::Withdrawn,
            Self::DeadlinePassed,
        ]
    }
}

impl FromStr for EventKind {
    type Err = UnknownDomainValue;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text {
            "discovered" => Ok(Self::Discovered),
            "ignored" => Ok(Self::Ignored),
            "reopened" => Ok(Self::Reopened),
            "resolved" => Ok(Self::Resolved),
            "reappeared" => Ok(Self::Reappeared),
            "withdrawn" => Ok(Self::Withdrawn),
            "deadline_passed" => Ok(Self::DeadlinePassed),
            _ => Err(UnknownDomainValue::new(text)),
        }
    }
}

impl fmt::Display for EventKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Text that is not one of the words a domain enum accepts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownDomainValue {
    value: String,
}

impl UnknownDomainValue {
    fn new(value: &str) -> Self {
        Self {
            value: value.to_owned(),
        }
    }

    /// The rejected text.
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }
}

impl fmt::Display for UnknownDomainValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "unknown value: {}", self.value)
    }
}

impl std::error::Error for UnknownDomainValue {}

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
    /// How severe the finding is.
    pub severity: Severity,
    /// Short description.
    pub summary: String,
    /// Other ids, such as CVE ids.
    pub aliases: Vec<String>,
    /// Versions in which this package is fixed.
    pub fixed_versions: Vec<String>,
    /// Current state.
    pub state: FindingState,
    /// Advisory kind.
    pub kind: FindingKind,
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
    /// How severe the finding is.
    pub severity: Severity,
    /// Short description.
    pub summary: String,
    /// Aliases.
    pub aliases: Vec<String>,
    /// Fixed versions.
    pub fixed_versions: Vec<String>,
    /// Advisory kind.
    pub kind: FindingKind,
    /// `first_seen` and `last_seen`.
    pub seen_at: String,
}

/// One row of a finding's history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StoredEvent {
    /// What changed.
    pub kind: EventKind,
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
        kind: EventKind,
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

#[cfg(test)]
mod tests {
    use super::{EventKind, FindingKind, FindingState, Severity};

    #[test]
    fn severity_words_round_trip_and_sort_most_severe_first() {
        assert_words::<Severity>(Severity::all());
        let mut ordered = Severity::all().to_vec();
        ordered.sort();
        assert_eq!(ordered.as_slice(), Severity::all());
    }

    #[test]
    fn finding_kind_words_round_trip() {
        assert_words::<FindingKind>(FindingKind::all());
    }

    #[test]
    fn event_kind_words_round_trip() {
        assert_words::<EventKind>(EventKind::all());
    }

    #[test]
    fn finding_state_words_round_trip() {
        for state in [
            FindingState::Open,
            FindingState::Ignored,
            FindingState::Resolved,
        ] {
            assert_eq!(state.as_str().parse::<FindingState>().unwrap(), state);
        }
        assert!("hidden".parse::<FindingState>().is_err());
    }

    fn assert_words<T>(values: &[T])
    where
        T: Copy
            + PartialEq
            + std::fmt::Debug
            + std::str::FromStr
            + serde::Serialize
            + serde::de::DeserializeOwned,
        T::Err: std::fmt::Debug,
    {
        for value in values {
            let word = serde_json::to_value(value).unwrap();
            let text = word.as_str().unwrap();
            assert_eq!(text.parse::<T>().unwrap(), *value);
            assert_eq!(serde_json::from_value::<T>(word).unwrap(), *value);
        }
        assert!("not-a-value".parse::<T>().is_err());
        assert!(serde_json::from_str::<T>("\"not-a-value\"").is_err());
    }
}
