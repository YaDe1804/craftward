//! Fixed interface strings.
//!
//! Project content is never passed through this table. English is the default.
//! German is the other language available at launch.

/// A language for fixed labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    /// English, the default.
    En,
    /// German.
    De,
}

impl Language {
    /// Parses `en` or `de`.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "en" => Some(Self::En),
            "de" => Some(Self::De),
            _ => None,
        }
    }

    /// `en` or `de`, used as the HTML language tag.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::De => "de",
        }
    }
}

/// One fixed string. Every language defines every label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Label {
    /// `{name} – {roadmap} v{number}`
    DocumentTitle,
    /// `{roadmap} v{number} · {as_of} {date}`
    MetaLine,
    /// `{label}: {value}`
    FieldLine,
    /// The word for a roadmap.
    Roadmap,
    /// The word that introduces the generation date.
    AsOf,
    /// The word for lifecycle status.
    Status,
    /// The word for the current version.
    Version,
    /// Heading for the whole plan.
    OverallProgress,
    /// The word for tasks, used in the overall count.
    Tasks,
    /// `{done} / {total} {tasks} · {percent} %`
    OverallCount,
    /// `{done} of {total}` or `{done} von {total}`.
    PhaseCount,
    /// `{percent} %`
    Percent,
    /// A finished item.
    Done,
    /// An item that has been started.
    InProgress,
    /// An item that has not been started.
    Open,
    /// Heading for issues that are still open.
    OpenIssues,
    /// Heading for watch items.
    WatchItems,
    /// Heading for stack components.
    Stack,
    /// Heading for links.
    Links,
    /// Column heading.
    ColumnSeverity,
    /// Column heading.
    ColumnTitle,
    /// Column heading.
    ColumnStatus,
    /// Column heading.
    ColumnKind,
    /// Column heading.
    ColumnDue,
    /// Column heading.
    ColumnName,
    /// Column heading.
    ColumnVersion,
    /// Column heading.
    ColumnNote,
    /// Column heading.
    ColumnLabel,
    /// Column heading.
    ColumnUrl,
    /// Project status `idea`.
    ProjectIdea,
    /// Project status `in_development`.
    ProjectInDevelopment,
    /// Project status `released`.
    ProjectReleased,
    /// Project status `maintenance`.
    ProjectMaintenance,
    /// Project status `paused`.
    ProjectPaused,
    /// Project status `discontinued`.
    ProjectDiscontinued,
    /// Issue severity `low`.
    SeverityLow,
    /// Issue severity `medium`.
    SeverityMedium,
    /// Issue severity `high`.
    SeverityHigh,
    /// Issue severity `critical`.
    SeverityCritical,
    /// Issue status `open`.
    IssueOpen,
    /// Issue status `in_progress`.
    IssueInProgress,
    /// Watch kind `deadline`.
    WatchDeadline,
    /// Watch kind `expiry`.
    WatchExpiry,
    /// Watch kind `review`.
    WatchReview,
    /// Watch kind `other`.
    WatchOther,
    /// Component kind `platform`.
    KindPlatform,
    /// Component kind `language`.
    KindLanguage,
    /// Component kind `framework`.
    KindFramework,
    /// Component kind `runtime`.
    KindRuntime,
    /// Component kind `sdk`.
    KindSdk,
    /// Component kind `service`.
    KindService,
    /// Component kind `tool`.
    KindTool,
    /// Component kind `other`.
    KindOther,
}

/// Every label, in declaration order.
pub const LABELS: &[Label] = &[
    Label::DocumentTitle,
    Label::MetaLine,
    Label::FieldLine,
    Label::Roadmap,
    Label::AsOf,
    Label::Status,
    Label::Version,
    Label::OverallProgress,
    Label::Tasks,
    Label::OverallCount,
    Label::PhaseCount,
    Label::Percent,
    Label::Done,
    Label::InProgress,
    Label::Open,
    Label::OpenIssues,
    Label::WatchItems,
    Label::Stack,
    Label::Links,
    Label::ColumnSeverity,
    Label::ColumnTitle,
    Label::ColumnStatus,
    Label::ColumnKind,
    Label::ColumnDue,
    Label::ColumnName,
    Label::ColumnVersion,
    Label::ColumnNote,
    Label::ColumnLabel,
    Label::ColumnUrl,
    Label::ProjectIdea,
    Label::ProjectInDevelopment,
    Label::ProjectReleased,
    Label::ProjectMaintenance,
    Label::ProjectPaused,
    Label::ProjectDiscontinued,
    Label::SeverityLow,
    Label::SeverityMedium,
    Label::SeverityHigh,
    Label::SeverityCritical,
    Label::IssueOpen,
    Label::IssueInProgress,
    Label::WatchDeadline,
    Label::WatchExpiry,
    Label::WatchReview,
    Label::WatchOther,
    Label::KindPlatform,
    Label::KindLanguage,
    Label::KindFramework,
    Label::KindRuntime,
    Label::KindSdk,
    Label::KindService,
    Label::KindTool,
    Label::KindOther,
];

/// Returns the fixed string for `label` in `language`.
#[must_use]
pub fn text(language: Language, label: Label) -> &'static str {
    match (language, label) {
        (Language::En | Language::De, Label::DocumentTitle) => "{name} – {roadmap} v{number}",
        (Language::En | Language::De, Label::MetaLine) => "{roadmap} v{number} · {as_of} {date}",
        (Language::En | Language::De, Label::FieldLine) => "{label}: {value}",
        (Language::En | Language::De, Label::Roadmap) => "Roadmap",
        (Language::En, Label::AsOf) => "as of",
        (Language::De, Label::AsOf) => "Stand",
        (Language::En | Language::De, Label::Status | Label::ColumnStatus) => "Status",
        (Language::En | Language::De, Label::Version | Label::ColumnVersion) => "Version",
        (Language::En, Label::OverallProgress) => "Overall progress",
        (Language::De, Label::OverallProgress) => "Gesamtfortschritt",
        (Language::En, Label::Tasks) => "tasks",
        (Language::De, Label::Tasks) => "Aufgaben",
        (Language::En | Language::De, Label::OverallCount) => {
            "{done} / {total} {tasks} · {percent} %"
        }
        (Language::En, Label::PhaseCount) => "{done} of {total}",
        (Language::De, Label::PhaseCount) => "{done} von {total}",
        (Language::En | Language::De, Label::Percent) => "{percent} %",
        (Language::En, Label::Done) => "done",
        (Language::De, Label::Done) => "erledigt",
        (Language::En, Label::InProgress | Label::IssueInProgress) => "in progress",
        (Language::De, Label::InProgress | Label::IssueInProgress) => "in Arbeit",
        (Language::En, Label::Open | Label::IssueOpen) => "open",
        (Language::De, Label::Open | Label::IssueOpen) => "offen",
        (Language::En, Label::OpenIssues) => "Open issues",
        (Language::De, Label::OpenIssues) => "Offene Punkte",
        (Language::En, Label::WatchItems) => "Watch items",
        (Language::De, Label::WatchItems) => "Wächter",
        (Language::En | Language::De, Label::Stack) => "Stack",
        (Language::En | Language::De, Label::Links) => "Links",
        (Language::En, Label::ColumnSeverity) => "Severity",
        (Language::De, Label::ColumnSeverity) => "Schwere",
        (Language::En, Label::ColumnTitle) => "Title",
        (Language::De, Label::ColumnTitle) => "Titel",
        (Language::En, Label::ColumnKind) => "Kind",
        (Language::De, Label::ColumnKind) => "Art",
        (Language::En, Label::ColumnDue) => "Due",
        (Language::De, Label::ColumnDue) => "Fällig",
        (Language::En | Language::De, Label::ColumnName) => "Name",
        (Language::En, Label::ColumnNote) => "Note",
        (Language::De, Label::ColumnNote) => "Notiz",
        (Language::En, Label::ColumnLabel) => "Label",
        (Language::De, Label::ColumnLabel) => "Bezeichnung",
        (Language::En, Label::ColumnUrl) => "URL",
        (Language::De, Label::ColumnUrl) => "Adresse",
        (Language::En, Label::ProjectIdea) => "idea",
        (Language::De, Label::ProjectIdea) => "Idee",
        (Language::En, Label::ProjectInDevelopment) => "in development",
        (Language::De, Label::ProjectInDevelopment) => "in Entwicklung",
        (Language::En, Label::ProjectReleased) => "released",
        (Language::De, Label::ProjectReleased) => "veröffentlicht",
        (Language::En, Label::ProjectMaintenance) => "maintenance",
        (Language::De, Label::ProjectMaintenance) => "Wartung",
        (Language::En, Label::ProjectPaused) => "paused",
        (Language::De, Label::ProjectPaused) => "pausiert",
        (Language::En, Label::ProjectDiscontinued) => "discontinued",
        (Language::De, Label::ProjectDiscontinued) => "eingestellt",
        (Language::En, Label::SeverityLow) => "low",
        (Language::De, Label::SeverityLow) => "niedrig",
        (Language::En, Label::SeverityMedium) => "medium",
        (Language::De, Label::SeverityMedium) => "mittel",
        (Language::En, Label::SeverityHigh) => "high",
        (Language::De, Label::SeverityHigh) => "hoch",
        (Language::En, Label::SeverityCritical) => "critical",
        (Language::De, Label::SeverityCritical) => "kritisch",
        (Language::En, Label::WatchDeadline) => "deadline",
        (Language::De, Label::WatchDeadline) => "Frist",
        (Language::En, Label::WatchExpiry) => "expiry",
        (Language::De, Label::WatchExpiry) => "Ablauf",
        (Language::En, Label::WatchReview) => "review",
        (Language::De, Label::WatchReview) => "Prüfung",
        (Language::En, Label::WatchOther | Label::KindOther) => "other",
        (Language::De, Label::WatchOther | Label::KindOther) => "sonstiges",
        (Language::En, Label::KindPlatform) => "platform",
        (Language::De, Label::KindPlatform) => "Plattform",
        (Language::En, Label::KindLanguage) => "language",
        (Language::De, Label::KindLanguage) => "Sprache",
        (Language::En, Label::KindFramework) => "framework",
        (Language::De, Label::KindFramework) => "Framework",
        (Language::En, Label::KindRuntime) => "runtime",
        (Language::De, Label::KindRuntime) => "Laufzeit",
        (Language::En, Label::KindSdk) => "sdk",
        (Language::De, Label::KindSdk) => "SDK",
        (Language::En, Label::KindService) => "service",
        (Language::De, Label::KindService) => "Dienst",
        (Language::En, Label::KindTool) => "tool",
        (Language::De, Label::KindTool) => "Werkzeug",
    }
}

/// Fills `{name}` placeholders in a label. Values are inserted once and are
/// not scanned for further placeholders.
#[must_use]
pub fn fill(language: Language, label: Label, pairs: &[(&str, &str)]) -> String {
    let template = text(language, label);
    let mut filled = String::new();
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        filled.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let Some(end) = after.find('}') else {
            filled.push_str(&rest[start..]);
            return filled;
        };
        let key = &after[..end];
        let value = pairs
            .iter()
            .find_map(|(name, value)| (*name == key).then_some(*value))
            .unwrap_or("");
        filled.push_str(value);
        rest = &after[end + 1..];
    }
    filled.push_str(rest);
    filled
}

#[cfg(test)]
mod tests {
    use super::{LABELS, Label, Language, text};

    fn variant_index(label: Label) -> usize {
        match label {
            Label::DocumentTitle => 0,
            Label::MetaLine => 1,
            Label::FieldLine => 2,
            Label::Roadmap => 3,
            Label::AsOf => 4,
            Label::Status => 5,
            Label::Version => 6,
            Label::OverallProgress => 7,
            Label::Tasks => 8,
            Label::OverallCount => 9,
            Label::PhaseCount => 10,
            Label::Percent => 11,
            Label::Done => 12,
            Label::InProgress => 13,
            Label::Open => 14,
            Label::OpenIssues => 15,
            Label::WatchItems => 16,
            Label::Stack => 17,
            Label::Links => 18,
            Label::ColumnSeverity => 19,
            Label::ColumnTitle => 20,
            Label::ColumnStatus => 21,
            Label::ColumnKind => 22,
            Label::ColumnDue => 23,
            Label::ColumnName => 24,
            Label::ColumnVersion => 25,
            Label::ColumnNote => 26,
            Label::ColumnLabel => 27,
            Label::ColumnUrl => 28,
            Label::ProjectIdea => 29,
            Label::ProjectInDevelopment => 30,
            Label::ProjectReleased => 31,
            Label::ProjectMaintenance => 32,
            Label::ProjectPaused => 33,
            Label::ProjectDiscontinued => 34,
            Label::SeverityLow => 35,
            Label::SeverityMedium => 36,
            Label::SeverityHigh => 37,
            Label::SeverityCritical => 38,
            Label::IssueOpen => 39,
            Label::IssueInProgress => 40,
            Label::WatchDeadline => 41,
            Label::WatchExpiry => 42,
            Label::WatchReview => 43,
            Label::WatchOther => 44,
            Label::KindPlatform => 45,
            Label::KindLanguage => 46,
            Label::KindFramework => 47,
            Label::KindRuntime => 48,
            Label::KindSdk => 49,
            Label::KindService => 50,
            Label::KindTool => 51,
            Label::KindOther => 52,
        }
    }

    #[test]
    fn every_language_has_every_label() {
        let mut seen = [false; 53];
        assert_eq!(LABELS.len(), seen.len());
        for label in LABELS {
            let index = variant_index(*label);
            assert!(!seen[index], "duplicate label");
            seen[index] = true;
            for language in [Language::En, Language::De] {
                assert!(
                    !text(language, *label).is_empty(),
                    "{language:?} is missing {label:?}"
                );
            }
        }
        assert!(seen.iter().all(|present| *present));
    }
}
