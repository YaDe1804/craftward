//! Pure view-model builders. No egui types live here.

use std::path::{Path, PathBuf};

use craftward_core::{
    ComponentKind, FindingState, Issue, IssueSeverity, IssueStatus, LockKind, Phase, PhaseState,
    Project, ProjectStatus, Severity, StoredFinding, Task, TaskProgress, TaskStatus, WatchItem,
    WatchKind, days_between, lock_kind, normalize_category, normalize_category_against,
    phase_state, project_platforms, resolve_project_paths, snapshot, task_progress, unix_now,
    utc_day,
};

use crate::date::{format_date_for_display, invalid_date_message, parse_date_input};
use crate::load::{DatabaseStatus, LoadedPortfolio, LoadedProject};
use crate::symbols;
use crate::theme::{GRID_THREE_COLUMNS_FROM, GRID_TWO_COLUMNS_FROM};

/// Portfolio summary shown above the project cards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortfolioSummary {
    /// Number of projects in the workspace (including load failures).
    pub project_count: usize,
    /// Projects whose files loaded.
    pub loaded_count: usize,
    /// Projects that failed to load.
    pub error_count: usize,
    /// Open issues across loaded projects.
    pub open_issues: usize,
    /// Open findings across loaded projects.
    pub open_findings: usize,
    /// Actively ignored findings (not expired) across loaded projects.
    pub ignored_findings: usize,
    /// Open findings by severity: critical, high, medium, low, unknown.
    pub findings_by_severity: [usize; 5],
    /// Human summary line.
    pub summary_line: String,
}

/// Task progress for display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgressView {
    /// Done tasks (not dropped).
    pub done: usize,
    /// Counted tasks.
    pub total: usize,
    /// Rounded percent.
    pub percent: u32,
}

impl ProgressView {
    fn from_progress(progress: TaskProgress) -> Self {
        Self {
            done: progress.done,
            total: progress.total,
            percent: progress.percent,
        }
    }

    fn empty() -> Self {
        Self {
            done: 0,
            total: 0,
            percent: 0,
        }
    }

    /// Caption next to the bar, for example `Tasks 2/4 (50%)`.
    #[must_use]
    pub fn caption(self) -> String {
        format!("Tasks {}/{} ({}%)", self.done, self.total, self.percent)
    }
}

/// Symbol plus readable label for a status or severity.
///
/// Labels live here so a later translation pass can replace the strings in one
/// place without touching egui code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusBadge {
    /// Stable key (schema word), useful for tests and later i18n.
    pub key: &'static str,
    /// Short symbol shown with the label.
    pub symbol: &'static str,
    /// Human-readable English label.
    pub label: &'static str,
}

impl StatusBadge {
    /// Combined badge text (`√ Done`), or the label alone when there is no symbol.
    #[must_use]
    pub fn display(&self) -> String {
        if self.symbol.is_empty() {
            self.label.to_owned()
        } else {
            format!("{} {}", self.symbol, self.label)
        }
    }
}

/// One project card on the overview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectCard {
    /// Index into [`LoadedPortfolio::projects`].
    pub index: usize,
    /// Project id when known (tooltip / secondary only).
    pub id: String,
    /// Display name (or a fallback when loading failed).
    pub name: String,
    /// Status badge, when the file loaded.
    pub status: Option<StatusBadge>,
    /// Current version when set.
    pub version: Option<String>,
    /// Task progress.
    pub progress: ProgressView,
    /// Open issues (`open` / `in_progress`).
    pub open_issues: usize,
    /// Label for open issues.
    pub open_issues_label: String,
    /// Open findings by severity.
    pub findings_by_severity: [usize; 5],
    /// Finding severity badges with counts (non-zero only).
    pub finding_badges: Vec<(StatusBadge, usize)>,
    /// Actively ignored findings (not counted in severity badges).
    pub ignored_findings: usize,
    /// Next due date `YYYY-MM-DD`.
    pub next_due: Option<String>,
    /// Human remaining-time text for [`Self::next_due`].
    pub next_due_remaining: Option<String>,
    /// Title of the first phase that is not done.
    pub current_phase: Option<String>,
    /// Stored Android `target_sdk` when the project file sets one.
    pub target_sdk: Option<u32>,
    /// Portfolio category when set.
    pub category: Option<String>,
    /// Readable platform labels derived from the stack.
    pub platforms: Vec<String>,
    /// Project tags.
    pub tags: Vec<String>,
    /// Load error shown on the card.
    pub load_error: Option<String>,
}

/// Sidebar selection under Portfolio (session-sticky).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PortfolioSidebarFilter {
    /// Every project, including load failures.
    #[default]
    All,
    /// One category spelling as stored on projects.
    Category(String),
    /// Loaded projects with no category.
    Uncategorized,
    /// Projects whose files failed to load.
    WithErrors,
}

/// One indented sidebar row under Portfolio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortfolioSidebarEntry {
    /// Selection this row applies.
    pub filter: PortfolioSidebarFilter,
    /// Label including count, for example `All (3)`.
    pub label: String,
}

/// Platform and tag filters on the overview (combined with the sidebar with AND).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OverviewFilters {
    /// Readable platform label, or empty for all.
    pub platform: String,
    /// Exact tag, or empty for all.
    pub tag: String,
}

/// One open finding row in the detail view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindingRow {
    /// Severity badge.
    pub severity: StatusBadge,
    /// Advisory kind badge.
    pub kind: StatusBadge,
    /// Source label (package + vulnerability id).
    pub source: String,
    /// Short summary.
    pub summary: String,
}

/// One issue row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueRow {
    /// Issue id (tooltip / secondary).
    pub id: String,
    /// Title.
    pub title: String,
    /// Severity badge.
    pub severity: StatusBadge,
    /// Status badge.
    pub status: StatusBadge,
    /// Tracker URL when set.
    pub url: Option<String>,
    /// Note when set.
    pub note: Option<String>,
}

/// One task row inside a phase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRow {
    /// Task id (tooltip / secondary).
    pub id: String,
    /// Title.
    pub title: String,
    /// Status badge.
    pub status: StatusBadge,
    /// Due date when set.
    pub due: Option<String>,
    /// Note when set.
    pub note: Option<String>,
}

/// One phase block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseBlock {
    /// Phase id (tooltip / secondary).
    pub id: String,
    /// Title.
    pub title: String,
    /// Goal when set.
    pub goal: Option<String>,
    /// Derived phase status.
    pub status: StatusBadge,
    /// Whether the phase should start expanded.
    pub initially_expanded: bool,
    /// Progress of tasks in the phase.
    pub progress: ProgressView,
    /// Tasks.
    pub tasks: Vec<TaskRow>,
}

/// One stack component row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentRow {
    /// Kind label.
    pub kind: StatusBadge,
    /// Name.
    pub name: String,
    /// Version when set.
    pub version: Option<String>,
    /// Free-form note when set.
    pub note: Option<String>,
    /// endoflife.date product id when set.
    pub eol_product: Option<String>,
}

/// Android fields for the detail page and edit form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AndroidDetail {
    /// Stored `target_sdk`.
    pub target_sdk: Option<u32>,
    /// Stored `min_sdk`.
    pub min_sdk: Option<u32>,
    /// Form factor schema word.
    pub form_factor: String,
    /// Store schema word.
    pub store: String,
    /// Detection paths relative to `source_dir`.
    pub detect_from: Vec<String>,
}

/// One watch / deadline row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeadlineRow {
    /// Kind badge.
    pub kind: StatusBadge,
    /// Title.
    pub title: String,
    /// Due date.
    pub due: String,
    /// Remaining-time text for `due`.
    pub remaining: String,
    /// Watch id when this row is a watch item (task dues leave this empty).
    pub watch_id: Option<String>,
    /// Watch kind schema word when this row is a watch item.
    pub watch_kind: Option<String>,
    /// Watch note when set.
    pub note: Option<String>,
}

/// Label/value pair for the overview section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldRow {
    /// Field label.
    pub label: &'static str,
    /// Field value.
    pub value: String,
}

/// Full detail page for one project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectDetail {
    /// Index into the portfolio.
    pub index: usize,
    /// Absolute path (secondary / tooltip).
    pub path: String,
    /// Breadcrumb title, for example `Portfolio › Lockward`.
    pub breadcrumb: String,
    /// Load error when the file failed.
    pub load_error: Option<String>,
    /// Project id (secondary).
    pub id: String,
    /// Name.
    pub name: String,
    /// Status badge.
    pub status: Option<StatusBadge>,
    /// Overview fields (label/value).
    pub overview_fields: Vec<FieldRow>,
    /// Description.
    pub description: Option<String>,
    /// `current_version` when set.
    pub version: Option<String>,
    /// `source_dir` when set.
    pub source_dir: Option<String>,
    /// Portfolio category when set.
    pub category: Option<String>,
    /// Tags for the header form.
    pub tags: Vec<String>,
    /// Links as `(label, url)` for the header form.
    pub links: Vec<(String, String)>,
    /// Overall progress.
    pub progress: ProgressView,
    /// Stored Android `target_sdk` when set.
    pub target_sdk: Option<u32>,
    /// Stack components.
    pub components: Vec<ComponentRow>,
    /// Lockfile paths relative to `source_dir`.
    pub lockfiles: Vec<String>,
    /// Android block when `stack.android` is present.
    pub android: Option<AndroidDetail>,
    /// Phases.
    pub phases: Vec<PhaseBlock>,
    /// Open issues.
    pub issues: Vec<IssueRow>,
    /// Open findings.
    pub findings: Vec<FindingRow>,
    /// Watch items / due dates.
    pub deadlines: Vec<DeadlineRow>,
}

/// Overview built from a loaded portfolio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortfolioView {
    /// Workspace path for the footer / tooltip.
    pub workspace_path: Option<String>,
    /// Workspace-level error.
    pub workspace_error: Option<String>,
    /// Database path for the footer / tooltip.
    pub database_path: Option<String>,
    /// Human message about the database.
    pub database_message: Option<String>,
    /// Aggregate summary.
    pub summary: PortfolioSummary,
    /// Cards sorted by urgency.
    pub cards: Vec<ProjectCard>,
}

/// Builds the overview using today's UTC date from the system clock.
#[must_use]
pub fn build_portfolio(loaded: &LoadedPortfolio) -> PortfolioView {
    let today = utc_day(unix_now()).unwrap_or_else(|| "1970-01-01".to_owned());
    build_portfolio_at(loaded, &today)
}

/// Builds the overview with an injected calendar day (`YYYY-MM-DD`).
#[must_use]
pub fn build_portfolio_at(loaded: &LoadedPortfolio, today: &str) -> PortfolioView {
    let mut cards: Vec<ProjectCard> = loaded
        .projects
        .iter()
        .enumerate()
        .map(|(index, project)| build_card(index, project, today))
        .collect();
    cards.sort_by(compare_urgency);

    let mut findings_by_severity = [0_usize; 5];
    let mut open_issues = 0;
    let mut ignored_findings = 0;
    let mut loaded_count = 0;
    let mut error_count = 0;
    for card in &cards {
        if card.load_error.is_some() {
            error_count += 1;
        } else {
            loaded_count += 1;
        }
        open_issues += card.open_issues;
        ignored_findings += card.ignored_findings;
        for (slot, count) in findings_by_severity
            .iter_mut()
            .zip(card.findings_by_severity)
        {
            *slot += count;
        }
    }
    let open_findings = findings_by_severity.iter().sum();
    let summary_line = format!(
        "{} · {} · {}",
        project_count_label(cards.len(), loaded_count, error_count),
        open_issues_label(open_issues),
        open_findings_label(open_findings)
    );

    PortfolioView {
        workspace_path: loaded
            .workspace_path
            .as_ref()
            .map(|path| path.display().to_string()),
        workspace_error: loaded.workspace_error.clone(),
        database_path: loaded
            .database_path
            .as_ref()
            .map(|path| path.display().to_string()),
        database_message: database_message(&loaded.database),
        summary: PortfolioSummary {
            project_count: cards.len(),
            loaded_count,
            error_count,
            open_issues,
            open_findings,
            ignored_findings,
            findings_by_severity,
            summary_line,
        },
        cards,
    }
}

/// Builds the detail page using today's UTC date from the system clock.
#[must_use]
pub fn build_detail(index: usize, loaded: &LoadedProject) -> ProjectDetail {
    let today = utc_day(unix_now()).unwrap_or_else(|| "1970-01-01".to_owned());
    build_detail_at(index, loaded, &today)
}

/// Builds the detail page with an injected calendar day (`YYYY-MM-DD`).
#[must_use]
pub fn build_detail_at(index: usize, loaded: &LoadedProject, today: &str) -> ProjectDetail {
    let path = loaded.path.display().to_string();
    match &loaded.project {
        Ok(project) => {
            let status = Some(project_status_badge(project.status));
            let target_sdk = stored_target_sdk(project);
            let mut overview_fields = vec![
                FieldRow {
                    label: "Status",
                    value: status
                        .as_ref()
                        .map_or_else(String::new, StatusBadge::display),
                },
                FieldRow {
                    label: "Id",
                    value: project.id.clone(),
                },
            ];
            if let Some(category) = &project.category {
                overview_fields.push(FieldRow {
                    label: "Category",
                    value: category.clone(),
                });
            }
            if let Some(version) = &project.current_version {
                overview_fields.push(FieldRow {
                    label: "Version",
                    value: version.clone(),
                });
            }
            if let Some(sdk) = target_sdk {
                overview_fields.push(FieldRow {
                    label: "Android targetSdk",
                    value: sdk.to_string(),
                });
            }
            ProjectDetail {
                index,
                path,
                breadcrumb: format!("Portfolio › {}", project.name),
                load_error: None,
                id: project.id.clone(),
                name: project.name.clone(),
                status,
                overview_fields,
                description: project.description.clone(),
                version: project.current_version.clone(),
                source_dir: project.source_dir.clone(),
                category: project.category.clone(),
                tags: project.tags.clone().unwrap_or_default(),
                links: project.links.as_ref().map_or_default(|links| {
                    links
                        .iter()
                        .map(|link| (link.label.clone(), link.url.clone()))
                        .collect()
                }),
                progress: project_progress(project),
                target_sdk,
                components: component_rows(project),
                lockfiles: lockfile_rows(project),
                android: android_detail(project),
                phases: phase_blocks(project),
                issues: open_issue_rows(project),
                findings: finding_rows(&loaded.findings, today),
                deadlines: deadline_rows(project, today),
            }
        }
        Err(error) => ProjectDetail {
            index,
            path,
            breadcrumb: "Portfolio › Unavailable project".to_owned(),
            load_error: Some(error.clone()),
            id: String::new(),
            name: "Unavailable project".to_owned(),
            status: None,
            overview_fields: Vec::new(),
            description: None,
            version: None,
            source_dir: None,
            category: None,
            tags: Vec::new(),
            links: Vec::new(),
            progress: ProgressView::empty(),
            target_sdk: None,
            components: Vec::new(),
            lockfiles: Vec::new(),
            android: None,
            phases: Vec::new(),
            issues: Vec::new(),
            findings: finding_rows(&loaded.findings, today),
            deadlines: Vec::new(),
        },
    }
}

/// How many portfolio columns fit in `available_width` points.
#[must_use]
pub fn grid_columns(available_width: f32) -> usize {
    if available_width >= GRID_THREE_COLUMNS_FROM {
        3
    } else if available_width >= GRID_TWO_COLUMNS_FROM {
        2
    } else {
        1
    }
}

/// Longest choice label in a status menu (drives the shared chip column width).
#[must_use]
pub fn longest_status_choice_label(choices: &[(&'static str, &'static str)]) -> &'static str {
    choices
        .iter()
        .map(|(_, label)| *label)
        .max_by_key(|label| label.chars().count())
        .unwrap_or("")
}

/// Whether a task title should use the muted colour (done / dropped).
#[must_use]
pub fn task_title_is_muted(status_key: &str) -> bool {
    matches!(status_key, "done" | "dropped")
}

/// Column slots to allocate for a portfolio row.
///
/// Always returns `columns` so a short last row keeps each card at the same
/// width as a full row, instead of stretching a single card across the row.
#[must_use]
pub fn grid_row_column_slots(columns: usize, cards_in_row: usize) -> usize {
    debug_assert!(cards_in_row > 0);
    debug_assert!(cards_in_row <= columns);
    columns.max(1)
}

/// Whether a status badge should expand to the parent width.
///
/// Badges shrink to their text. The UI must not stretch them across a card.
#[must_use]
pub const fn badge_stretches_to_parent() -> bool {
    false
}

/// Draft for adding a task to a phase.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TaskForm {
    /// Phase that receives the task.
    pub phase_id: String,
    /// Required title.
    pub title: String,
    /// Optional due date (`YYYY-MM-DD`), empty when unset.
    pub due: String,
}

impl TaskForm {
    /// Starts an empty form for `phase_id`.
    #[must_use]
    pub fn new(phase_id: impl Into<String>) -> Self {
        Self {
            phase_id: phase_id.into(),
            title: String::new(),
            due: String::new(),
        }
    }

    /// Whether Save may run.
    #[must_use]
    pub fn can_save(&self) -> bool {
        self.validation_message().is_none()
    }

    /// Human validation error, when the draft is not ready to save.
    #[must_use]
    pub fn validation_message(&self) -> Option<&'static str> {
        if self.title.trim().is_empty() {
            return Some("Title is required");
        }
        let due = self.due.trim();
        if !due.is_empty() && parse_date_input(due).is_none() {
            return Some(invalid_date_message());
        }
        None
    }

    /// Due date for the core API as ISO (`None` when blank).
    #[must_use]
    pub fn due_iso(&self) -> Option<String> {
        parse_date_input(self.due.trim())
    }

    /// Trimmed title for the core API.
    #[must_use]
    pub fn title_for_edit(&self) -> &str {
        self.title.trim()
    }
}

/// Draft for adding an issue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueForm {
    /// Required title.
    pub title: String,
    /// Severity schema word.
    pub severity: String,
    /// Optional tracker URL.
    pub url: String,
    /// Optional note.
    pub note: String,
}

impl Default for IssueForm {
    fn default() -> Self {
        Self {
            title: String::new(),
            severity: "medium".to_owned(),
            url: String::new(),
            note: String::new(),
        }
    }
}

impl IssueForm {
    /// Whether Save may run.
    #[must_use]
    pub fn can_save(&self) -> bool {
        self.validation_message().is_none()
    }

    /// Human validation error, when the draft is not ready to save.
    #[must_use]
    pub fn validation_message(&self) -> Option<&'static str> {
        if self.title.trim().is_empty() {
            return Some("Title is required");
        }
        if !matches!(
            self.severity.as_str(),
            "low" | "medium" | "high" | "critical"
        ) {
            return Some("Severity must be low, medium, high, or critical");
        }
        let url = self.url.trim();
        if !(url.is_empty() || url.starts_with("http://") || url.starts_with("https://")) {
            return Some("URL must start with http:// or https://");
        }
        None
    }

    /// Trimmed title for the core API.
    #[must_use]
    pub fn title_for_edit(&self) -> &str {
        self.title.trim()
    }

    /// URL for the core API (`None` when blank).
    #[must_use]
    pub fn url_for_edit(&self) -> Option<&str> {
        let url = self.url.trim();
        if url.is_empty() { None } else { Some(url) }
    }

    /// Note for the core API (`None` when blank).
    #[must_use]
    pub fn note_for_edit(&self) -> Option<&str> {
        let note = self.note.trim();
        if note.is_empty() { None } else { Some(note) }
    }
}

/// Draft for editing overview header fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeaderForm {
    /// Display name.
    pub name: String,
    /// Lifecycle status schema word.
    pub status: String,
    /// Version string (empty clears).
    pub version: String,
    /// Description (empty clears).
    pub description: String,
    /// Source directory (empty clears).
    pub source_dir: String,
    /// Portfolio category (empty clears).
    pub category: String,
    /// Comma-separated tags.
    pub tags: String,
    /// Links, one `Label|URL` per line.
    pub links: String,
    /// Short hint when letter case was unified against an existing category.
    pub category_unify_hint: Option<String>,
}

impl HeaderForm {
    /// Fills the form from a loaded detail view.
    #[must_use]
    pub fn from_detail(detail: &ProjectDetail) -> Self {
        let status = detail
            .status
            .as_ref()
            .map_or("idea", |badge| badge.key)
            .to_owned();
        Self {
            name: detail.name.clone(),
            status,
            version: detail.version.clone().unwrap_or_default(),
            description: detail.description.clone().unwrap_or_default(),
            source_dir: detail.source_dir.clone().unwrap_or_default(),
            category: detail.category.clone().unwrap_or_default(),
            tags: detail.tags.join(", "),
            links: detail
                .links
                .iter()
                .map(|(label, url)| format!("{label}|{url}"))
                .collect::<Vec<_>>()
                .join("\n"),
            category_unify_hint: None,
        }
    }

    /// Whether Save may run.
    #[must_use]
    pub fn can_save(&self) -> bool {
        self.validation_message().is_none()
    }

    /// Name field error.
    #[must_use]
    pub fn name_error(&self) -> Option<&'static str> {
        if self.name.trim().is_empty() {
            Some("Name is required")
        } else {
            None
        }
    }

    /// Status field error.
    #[must_use]
    pub fn status_error(&self) -> Option<&'static str> {
        if project_status_choices()
            .iter()
            .any(|(key, _)| *key == self.status.as_str())
        {
            None
        } else {
            Some("Status is not a known lifecycle value")
        }
    }

    /// Category field error when non-empty text cannot be normalized.
    #[must_use]
    pub fn category_error(&self) -> Option<&'static str> {
        let trimmed = self.category.trim();
        if trimmed.is_empty() {
            return None;
        }
        if normalize_category(trimmed).is_none() {
            Some("Category must be 1–40 characters after trimming, with no control characters")
        } else {
            None
        }
    }

    /// Links field error.
    #[must_use]
    pub fn links_error(&self) -> Option<&'static str> {
        for line in self.links.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let Some((label, url)) = line.split_once('|') else {
                return Some("Each link must be Label|URL");
            };
            if label.trim().is_empty() || url.trim().is_empty() {
                return Some("Each link must be Label|URL");
            }
            let url = url.trim();
            if !(url.starts_with("http://") || url.starts_with("https://")) {
                return Some("Link URL must start with http:// or https://");
            }
        }
        None
    }

    /// Human validation error (first field error).
    #[must_use]
    pub fn validation_message(&self) -> Option<&'static str> {
        self.name_error()
            .or_else(|| self.status_error())
            .or_else(|| self.category_error())
            .or_else(|| self.links_error())
    }

    /// Trimmed name.
    #[must_use]
    pub fn name_for_edit(&self) -> &str {
        self.name.trim()
    }

    /// Version update for the core API.
    #[must_use]
    pub fn version_update(&self) -> craftward_core::ProjectUpdate<'_> {
        optional_clearable(&self.version)
    }

    /// Description update for the core API.
    #[must_use]
    pub fn description_update(&self) -> craftward_core::ProjectUpdate<'_> {
        optional_clearable(&self.description)
    }

    /// Source directory update for the core API.
    #[must_use]
    pub fn source_dir_update(&self) -> craftward_core::ProjectUpdate<'_> {
        optional_clearable(&self.source_dir)
    }

    /// Resolves the category for save: `None` clears, `Some` sets the stored spelling.
    ///
    /// Also updates [`Self::category`] and [`Self::category_unify_hint`].
    pub fn resolve_category(
        &mut self,
        existing: &[String],
    ) -> Result<Option<String>, &'static str> {
        self.category_unify_hint = None;
        let trimmed = self.category.trim();
        if trimmed.is_empty() {
            self.category.clear();
            return Ok(None);
        }
        match normalize_category_against(existing, trimmed) {
            Ok((value, unified)) => {
                if unified {
                    self.category_unify_hint = Some(format!("Using existing spelling “{value}”"));
                }
                self.category.clone_from(&value);
                Ok(Some(value))
            }
            Err(_) => {
                Err("Category must be 1–40 characters after trimming, with no control characters")
            }
        }
    }

    /// Parsed tags.
    #[must_use]
    pub fn tags_for_edit(&self) -> Vec<String> {
        self.tags
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(str::to_owned)
            .collect()
    }

    /// Parsed links as `(label, url)`.
    #[must_use]
    pub fn links_for_edit(&self) -> Vec<(String, String)> {
        self.links
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                if line.is_empty() {
                    return None;
                }
                let (label, url) = line.split_once('|')?;
                Some((label.trim().to_owned(), url.trim().to_owned()))
            })
            .collect()
    }
}

/// Draft for adding or editing a phase.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PhaseForm {
    /// Optional explicit id (add only); empty means form from title.
    pub id: String,
    /// Phase title.
    pub title: String,
    /// Optional goal.
    pub goal: String,
}

impl PhaseForm {
    /// Starts an edit form from an existing phase.
    #[must_use]
    pub fn from_phase(phase: &PhaseBlock) -> Self {
        Self {
            id: phase.id.clone(),
            title: phase.title.clone(),
            goal: phase.goal.clone().unwrap_or_default(),
        }
    }

    /// Whether Save may run.
    #[must_use]
    pub fn can_save(&self) -> bool {
        self.validation_message().is_none()
    }

    /// Title field error.
    #[must_use]
    pub fn title_error(&self) -> Option<&'static str> {
        if self.title.trim().is_empty() {
            Some("Title is required")
        } else {
            None
        }
    }

    /// Human validation error.
    #[must_use]
    pub fn validation_message(&self) -> Option<&'static str> {
        self.title_error()
    }

    /// Trimmed title.
    #[must_use]
    pub fn title_for_edit(&self) -> &str {
        self.title.trim()
    }

    /// Explicit id for add (`None` when blank).
    #[must_use]
    pub fn id_for_edit(&self) -> Option<&str> {
        let id = self.id.trim();
        if id.is_empty() { None } else { Some(id) }
    }

    /// Goal for add (`None` when blank).
    #[must_use]
    pub fn goal_for_edit(&self) -> Option<&str> {
        let goal = self.goal.trim();
        if goal.is_empty() { None } else { Some(goal) }
    }

    /// Goal update for `set_phase`.
    #[must_use]
    pub fn goal_update(&self) -> craftward_core::ProjectUpdate<'_> {
        optional_clearable(&self.goal)
    }
}

/// Draft for editing an existing task.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TaskEditForm {
    /// Task title.
    pub title: String,
    /// Due day.
    pub due: String,
    /// Note.
    pub note: String,
}

impl TaskEditForm {
    /// Starts an edit form from an existing task.
    #[must_use]
    pub fn from_task(task: &TaskRow) -> Self {
        Self {
            title: task.title.clone(),
            due: task
                .due
                .as_deref()
                .map_or_else(String::new, format_date_for_display),
            note: task.note.clone().unwrap_or_default(),
        }
    }

    /// Whether Save may run.
    #[must_use]
    pub fn can_save(&self) -> bool {
        self.validation_message().is_none()
    }

    /// Title field error.
    #[must_use]
    pub fn title_error(&self) -> Option<&'static str> {
        if self.title.trim().is_empty() {
            Some("Title is required")
        } else {
            None
        }
    }

    /// Due field error.
    #[must_use]
    pub fn due_error(&self) -> Option<&'static str> {
        let due = self.due.trim();
        if !due.is_empty() && parse_date_input(due).is_none() {
            Some(invalid_date_message())
        } else {
            None
        }
    }

    /// Human validation error.
    #[must_use]
    pub fn validation_message(&self) -> Option<&'static str> {
        self.title_error().or_else(|| self.due_error())
    }

    /// Trimmed title.
    #[must_use]
    pub fn title_for_edit(&self) -> &str {
        self.title.trim()
    }

    /// Parsed ISO due, or [`None`] when the field is blank.
    #[must_use]
    pub fn due_iso(&self) -> Option<String> {
        parse_date_input(self.due.trim())
    }

    /// Note update.
    #[must_use]
    pub fn note_update(&self) -> craftward_core::ProjectUpdate<'_> {
        optional_clearable(&self.note)
    }
}

/// Draft for editing an existing issue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueEditForm {
    /// Issue title.
    pub title: String,
    /// Severity schema word.
    pub severity: String,
    /// Tracker URL.
    pub url: String,
    /// Note.
    pub note: String,
}

impl Default for IssueEditForm {
    fn default() -> Self {
        Self {
            title: String::new(),
            severity: "medium".to_owned(),
            url: String::new(),
            note: String::new(),
        }
    }
}

impl IssueEditForm {
    /// Starts an edit form from an existing issue.
    #[must_use]
    pub fn from_issue(issue: &IssueRow) -> Self {
        Self {
            title: issue.title.clone(),
            severity: issue.severity.key.to_owned(),
            url: issue.url.clone().unwrap_or_default(),
            note: issue.note.clone().unwrap_or_default(),
        }
    }

    /// Whether Save may run.
    #[must_use]
    pub fn can_save(&self) -> bool {
        self.validation_message().is_none()
    }

    /// Title field error.
    #[must_use]
    pub fn title_error(&self) -> Option<&'static str> {
        if self.title.trim().is_empty() {
            Some("Title is required")
        } else {
            None
        }
    }

    /// Severity field error.
    #[must_use]
    pub fn severity_error(&self) -> Option<&'static str> {
        if matches!(
            self.severity.as_str(),
            "low" | "medium" | "high" | "critical"
        ) {
            None
        } else {
            Some("Severity must be low, medium, high, or critical")
        }
    }

    /// URL field error.
    #[must_use]
    pub fn url_error(&self) -> Option<&'static str> {
        let url = self.url.trim();
        if url.is_empty() || url.starts_with("http://") || url.starts_with("https://") {
            None
        } else {
            Some("URL must start with http:// or https://")
        }
    }

    /// Human validation error.
    #[must_use]
    pub fn validation_message(&self) -> Option<&'static str> {
        self.title_error()
            .or_else(|| self.severity_error())
            .or_else(|| self.url_error())
    }

    /// Trimmed title.
    #[must_use]
    pub fn title_for_edit(&self) -> &str {
        self.title.trim()
    }

    /// URL update.
    #[must_use]
    pub fn url_update(&self) -> craftward_core::ProjectUpdate<'_> {
        optional_clearable(&self.url)
    }

    /// Note update.
    #[must_use]
    pub fn note_update(&self) -> craftward_core::ProjectUpdate<'_> {
        optional_clearable(&self.note)
    }
}

/// Draft for adding a stack component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentForm {
    /// Component kind schema word.
    pub kind: String,
    /// Component name.
    pub name: String,
    /// Optional version.
    pub version: String,
    /// Optional endoflife.date product id.
    pub eol_product: String,
}

impl Default for ComponentForm {
    fn default() -> Self {
        Self {
            kind: "runtime".to_owned(),
            name: String::new(),
            version: String::new(),
            eol_product: String::new(),
        }
    }
}

impl ComponentForm {
    /// Whether Save may run.
    #[must_use]
    pub fn can_save(&self) -> bool {
        self.validation_message().is_none()
    }

    /// Human validation error.
    #[must_use]
    pub fn validation_message(&self) -> Option<&'static str> {
        if self.name.trim().is_empty() {
            return Some("Name is required");
        }
        if !component_kind_choices()
            .iter()
            .any(|(key, _)| *key == self.kind.as_str())
        {
            return Some("Choose a valid component kind");
        }
        None
    }

    /// Trimmed name.
    #[must_use]
    pub fn name_for_edit(&self) -> &str {
        self.name.trim()
    }

    /// Optional version.
    #[must_use]
    pub fn version_for_edit(&self) -> Option<&str> {
        optional_text(&self.version)
    }

    /// Optional eol product.
    #[must_use]
    pub fn eol_product_for_edit(&self) -> Option<&str> {
        optional_text(&self.eol_product)
    }
}

/// Draft for editing optional fields of an existing component.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ComponentEditForm {
    /// Version (empty clears).
    pub version: String,
    /// Note (empty clears).
    pub note: String,
    /// endoflife.date product id (empty clears).
    pub eol_product: String,
}

impl ComponentEditForm {
    /// Starts an edit form from an existing component.
    #[must_use]
    pub fn from_component(component: &ComponentRow) -> Self {
        Self {
            version: component.version.clone().unwrap_or_default(),
            note: component.note.clone().unwrap_or_default(),
            eol_product: component.eol_product.clone().unwrap_or_default(),
        }
    }

    /// Always valid; empty fields clear optional values.
    #[must_use]
    pub const fn can_save() -> bool {
        true
    }

    /// Version update.
    #[must_use]
    pub fn version_update(&self) -> craftward_core::ProjectUpdate<'_> {
        optional_clearable(&self.version)
    }

    /// Note update.
    #[must_use]
    pub fn note_update(&self) -> craftward_core::ProjectUpdate<'_> {
        optional_clearable(&self.note)
    }

    /// `eol_product` update.
    #[must_use]
    pub fn eol_product_update(&self) -> craftward_core::ProjectUpdate<'_> {
        optional_clearable(&self.eol_product)
    }
}

/// Draft for adding a lockfile path.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LockfileForm {
    /// Path relative to `source_dir`.
    pub path: String,
}

impl LockfileForm {
    /// Whether Save may run against `source_dir`.
    #[must_use]
    pub fn can_save(&self, source_dir: &Path) -> bool {
        self.validation_message(source_dir).is_none()
    }

    /// Human validation error.
    #[must_use]
    pub fn validation_message(&self, source_dir: &Path) -> Option<&'static str> {
        validate_lockfile_relative(self.path.trim(), source_dir)
    }

    /// Trimmed path for the core API.
    #[must_use]
    pub fn path_for_edit(&self) -> &str {
        self.path.trim()
    }
}

/// Draft for editing `stack.android`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AndroidForm {
    /// Target SDK (empty clears).
    pub target_sdk: String,
    /// Min SDK (empty clears).
    pub min_sdk: String,
    /// Form factor schema word.
    pub form_factor: String,
    /// Store schema word.
    pub store: String,
    /// One relative path per line.
    pub detect_from: String,
}

impl Default for AndroidForm {
    fn default() -> Self {
        Self {
            target_sdk: String::new(),
            min_sdk: String::new(),
            form_factor: "phone".to_owned(),
            store: "google_play".to_owned(),
            detect_from: String::new(),
        }
    }
}

impl AndroidForm {
    /// Starts from stored android fields, or defaults when absent.
    #[must_use]
    pub fn from_detail(android: Option<&AndroidDetail>) -> Self {
        match android {
            Some(android) => Self {
                target_sdk: android.target_sdk.map_or_default(|value| value.to_string()),
                min_sdk: android.min_sdk.map_or_default(|value| value.to_string()),
                form_factor: android.form_factor.clone(),
                store: android.store.clone(),
                detect_from: android.detect_from.join("\n"),
            },
            None => Self::default(),
        }
    }

    /// Whether Save may run.
    #[must_use]
    pub fn can_save(&self) -> bool {
        self.validation_message().is_none()
    }

    /// Human validation error.
    #[must_use]
    pub fn validation_message(&self) -> Option<&'static str> {
        if let Some(error) = sdk_field_error(&self.target_sdk) {
            return Some(error);
        }
        if let Some(error) = sdk_field_error(&self.min_sdk) {
            return Some(error);
        }
        if !form_factor_choices()
            .iter()
            .any(|(key, _)| *key == self.form_factor.as_str())
        {
            return Some("Choose a valid form factor");
        }
        if !android_store_choices()
            .iter()
            .any(|(key, _)| *key == self.store.as_str())
        {
            return Some("Choose a valid store");
        }
        for path in self.detect_from_paths() {
            if path.is_empty() {
                return Some("detect_from paths must not be empty");
            }
            if craftward_core::is_portable_absolute(&path)
                || path.split(['/', '\\']).any(|segment| segment == "..")
            {
                return Some("detect_from paths must be relative without ..");
            }
        }
        None
    }

    /// Target SDK update.
    #[must_use]
    pub fn target_sdk_update(&self) -> craftward_core::ProjectUpdate<'_> {
        optional_clearable(&self.target_sdk)
    }

    /// Min SDK update.
    #[must_use]
    pub fn min_sdk_update(&self) -> craftward_core::ProjectUpdate<'_> {
        optional_clearable(&self.min_sdk)
    }

    /// Detect-from paths for a full replace.
    #[must_use]
    pub fn detect_from_paths(&self) -> Vec<String> {
        self.detect_from
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_owned)
            .collect()
    }
}

/// Draft for adding a watch item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchForm {
    /// Kind schema word.
    pub kind: String,
    /// Title.
    pub title: String,
    /// Due day.
    pub due: String,
    /// Optional note.
    pub note: String,
}

impl Default for WatchForm {
    fn default() -> Self {
        Self {
            kind: "deadline".to_owned(),
            title: String::new(),
            due: String::new(),
            note: String::new(),
        }
    }
}

impl WatchForm {
    /// Whether Save may run.
    #[must_use]
    pub fn can_save(&self) -> bool {
        self.validation_message().is_none()
    }

    /// Human validation error.
    #[must_use]
    pub fn validation_message(&self) -> Option<&'static str> {
        if self.title.trim().is_empty() {
            return Some("Title is required");
        }
        if !watch_kind_choices()
            .iter()
            .any(|(key, _)| *key == self.kind.as_str())
        {
            return Some("Choose a valid watch kind");
        }
        let due = self.due.trim();
        if due.is_empty() {
            return Some("Due date is required");
        }
        if parse_date_input(due).is_none() {
            return Some(invalid_date_message());
        }
        None
    }

    /// Trimmed title.
    #[must_use]
    pub fn title_for_edit(&self) -> &str {
        self.title.trim()
    }

    /// Parsed ISO due date.
    #[must_use]
    pub fn due_iso(&self) -> String {
        parse_date_input(self.due.trim()).unwrap_or_default()
    }

    /// Optional note.
    #[must_use]
    pub fn note_for_edit(&self) -> Option<&str> {
        optional_text(&self.note)
    }
}

/// Draft for editing an existing watch item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchEditForm {
    /// Kind schema word.
    pub kind: String,
    /// Title.
    pub title: String,
    /// Due day.
    pub due: String,
    /// Note (empty clears).
    pub note: String,
}

impl WatchEditForm {
    /// Starts from a watch deadline row.
    #[must_use]
    pub fn from_deadline(row: &DeadlineRow) -> Self {
        Self {
            kind: row
                .watch_kind
                .clone()
                .unwrap_or_else(|| "deadline".to_owned()),
            title: row.title.clone(),
            due: format_date_for_display(&row.due),
            note: row.note.clone().unwrap_or_default(),
        }
    }

    /// Whether Save may run.
    #[must_use]
    pub fn can_save(&self) -> bool {
        self.validation_message().is_none()
    }

    /// Human validation error.
    #[must_use]
    pub fn validation_message(&self) -> Option<&'static str> {
        if self.title.trim().is_empty() {
            return Some("Title is required");
        }
        if !watch_kind_choices()
            .iter()
            .any(|(key, _)| *key == self.kind.as_str())
        {
            return Some("Choose a valid watch kind");
        }
        let due = self.due.trim();
        if due.is_empty() || parse_date_input(due).is_none() {
            return Some(invalid_date_message());
        }
        None
    }

    /// Trimmed title.
    #[must_use]
    pub fn title_for_edit(&self) -> &str {
        self.title.trim()
    }

    /// Parsed ISO due date.
    #[must_use]
    pub fn due_iso(&self) -> String {
        parse_date_input(self.due.trim()).unwrap_or_default()
    }

    /// Note update.
    #[must_use]
    pub fn note_update(&self) -> craftward_core::ProjectUpdate<'_> {
        optional_clearable(&self.note)
    }
}

/// Validates a lockfile path relative to `source_dir`.
#[must_use]
pub fn validate_lockfile_relative(path: &str, source_dir: &Path) -> Option<&'static str> {
    if path.is_empty() {
        return Some("Path is required");
    }
    if craftward_core::is_portable_absolute(path)
        || path.split(['/', '\\']).any(|segment| segment == "..")
    {
        return Some("Path must be relative to source_dir without ..");
    }
    let name = Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path);
    match lock_kind(Path::new(name)) {
        LockKind::Cargo | LockKind::Npm => {}
        LockKind::Unknown => {
            return Some("Lockfile must be Cargo.lock or package-lock.json");
        }
    }
    let resolved = source_dir.join(path);
    if !resolved.is_file() {
        return Some("Lockfile does not exist under source_dir");
    }
    None
}

/// Turns an absolute pick into a path relative to `base`, using `/` separators.
#[must_use]
pub fn relative_to_dir(absolute: &Path, base: &Path) -> Option<String> {
    let relative = absolute.strip_prefix(base).ok()?;
    let text = relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");
    if text.is_empty() { None } else { Some(text) }
}

/// Resolves the source directory used for lockfile paths.
#[must_use]
pub fn project_source_dir(project_file: &Path, project: &Project) -> PathBuf {
    resolve_project_paths(project_file, project).source_dir
}

fn optional_text(text: &str) -> Option<&str> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn sdk_field_error(text: &str) -> Option<&'static str> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    match trimmed.parse::<u32>() {
        Ok(value) if value >= 1 => None,
        _ => Some("SDK level must be a positive integer"),
    }
}

/// Draft for creating a new project and adding it to the workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewProjectForm {
    /// Absolute or relative folder path for `craftward.json`.
    pub directory: String,
    /// Optional source folder when code lives elsewhere.
    pub source_dir: String,
    /// Optional portfolio category.
    pub category: String,
    /// Project id.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Lifecycle status schema word.
    pub status: String,
    /// When true, id is no longer overwritten from the name.
    pub id_manual: bool,
    /// Short hint when letter case was unified against an existing category.
    pub category_unify_hint: Option<String>,
}

impl Default for NewProjectForm {
    fn default() -> Self {
        Self {
            directory: String::new(),
            source_dir: String::new(),
            category: String::new(),
            id: String::new(),
            name: String::new(),
            status: "idea".to_owned(),
            id_manual: false,
            category_unify_hint: None,
        }
    }
}

impl NewProjectForm {
    /// Whether Save may run for `taken_ids` already in the workspace.
    #[must_use]
    pub fn can_save(&self, taken_ids: &[String]) -> bool {
        self.validation_message(taken_ids).is_none()
    }

    /// Syncs the id from the name when the id has not been edited by hand.
    pub fn suggest_id_from_name(&mut self) {
        if self.id_manual {
            return;
        }
        self.id = craftward_core::id_from_title(self.name.trim()).unwrap_or_default();
    }

    /// Marks the id as hand-edited so further name changes leave it alone.
    pub fn mark_id_manual(&mut self) {
        self.id_manual = true;
    }

    /// Folder field error.
    #[must_use]
    pub fn directory_error(&self) -> Option<&'static str> {
        if self.directory.trim().is_empty() {
            Some("Folder is required")
        } else {
            None
        }
    }

    /// Optional source-folder field error.
    #[must_use]
    pub fn source_dir_error(&self) -> Option<&'static str> {
        let source = self.source_dir.trim();
        if source.is_empty() {
            return None;
        }
        if Path::new(source).is_dir() {
            None
        } else {
            Some("Source folder must exist")
        }
    }

    /// Category field error when non-empty text cannot be normalized.
    #[must_use]
    pub fn category_error(&self) -> Option<&'static str> {
        let trimmed = self.category.trim();
        if trimmed.is_empty() {
            return None;
        }
        if normalize_category(trimmed).is_none() {
            Some("Category must be 1–40 characters after trimming, with no control characters")
        } else {
            None
        }
    }

    /// Trimmed optional source directory.
    #[must_use]
    pub fn source_dir_for_edit(&self) -> Option<&str> {
        optional_text(&self.source_dir)
    }

    /// Resolves the category for create: `None` omits it.
    pub fn resolve_category(
        &mut self,
        existing: &[String],
    ) -> Result<Option<String>, &'static str> {
        self.category_unify_hint = None;
        let trimmed = self.category.trim();
        if trimmed.is_empty() {
            self.category.clear();
            return Ok(None);
        }
        match normalize_category_against(existing, trimmed) {
            Ok((value, unified)) => {
                if unified {
                    self.category_unify_hint = Some(format!("Using existing spelling “{value}”"));
                }
                self.category.clone_from(&value);
                Ok(Some(value))
            }
            Err(_) => {
                Err("Category must be 1–40 characters after trimming, with no control characters")
            }
        }
    }

    /// Name field error.
    #[must_use]
    pub fn name_error(&self) -> Option<&'static str> {
        if self.name.trim().is_empty() {
            Some("Name is required")
        } else {
            None
        }
    }

    /// Id field error, including duplicates in `taken_ids`.
    #[must_use]
    pub fn id_error(&self, taken_ids: &[String]) -> Option<&'static str> {
        let id = self.id.trim();
        if id.is_empty() {
            return Some("Id is required");
        }
        if !craftward_core::is_project_id(id) {
            return Some(
                "Id must be 1–64 characters from a-z, 0-9, and hyphen, and must not start or end with a hyphen",
            );
        }
        if taken_ids.iter().any(|taken| taken == id) {
            return Some("Id is already in the workspace");
        }
        None
    }

    /// Status field error.
    #[must_use]
    pub fn status_error(&self) -> Option<&'static str> {
        if project_status_choices()
            .iter()
            .any(|(key, _)| *key == self.status.as_str())
        {
            None
        } else {
            Some("Status is not a known lifecycle value")
        }
    }

    /// Human validation error (first field error).
    #[must_use]
    pub fn validation_message(&self, taken_ids: &[String]) -> Option<&'static str> {
        self.directory_error()
            .or_else(|| self.source_dir_error())
            .or_else(|| self.category_error())
            .or_else(|| self.name_error())
            .or_else(|| self.id_error(taken_ids))
            .or_else(|| self.status_error())
    }

    /// Trimmed id.
    #[must_use]
    pub fn id_for_edit(&self) -> &str {
        self.id.trim()
    }

    /// Trimmed name.
    #[must_use]
    pub fn name_for_edit(&self) -> &str {
        self.name.trim()
    }
}

/// Short explanation of each lifecycle status for form tooltips.
#[must_use]
pub fn project_status_hint(status: &str) -> &'static str {
    match status {
        "idea" => "Early idea; not actively built yet.",
        "in_development" => "Actively being built.",
        "released" => "Shipped to users.",
        "maintenance" => "Stable; only fixes and small updates.",
        "paused" => "On hold for now.",
        "discontinued" => "No longer maintained.",
        _ => "Project lifecycle status.",
    }
}

/// Draft for adding an existing project path to the workspace.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AddProjectForm {
    /// File or folder path.
    pub path: String,
}

impl AddProjectForm {
    /// Whether Save may run.
    #[must_use]
    pub fn can_save(&self) -> bool {
        !self.path.trim().is_empty()
    }
}

/// Project lifecycle choices for forms.
#[must_use]
pub fn project_status_choices() -> &'static [(&'static str, &'static str)] {
    &[
        ("idea", "Idea"),
        ("in_development", "In development"),
        ("released", "Released"),
        ("maintenance", "Maintenance"),
        ("paused", "Paused"),
        ("discontinued", "Discontinued"),
    ]
}

fn optional_clearable(text: &str) -> craftward_core::ProjectUpdate<'_> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        craftward_core::ProjectUpdate::Clear
    } else {
        craftward_core::ProjectUpdate::Set(trimmed)
    }
}

/// Task status choices for the status menu (schema word + label).
#[must_use]
pub fn task_status_choices() -> &'static [(&'static str, &'static str)] {
    &[
        ("open", "Open"),
        ("in_progress", "In progress"),
        ("done", "Done"),
        ("dropped", "Dropped"),
    ]
}

/// Issue status choices for the status menu (schema word + label).
#[must_use]
pub fn issue_status_choices() -> &'static [(&'static str, &'static str)] {
    &[
        ("open", "Open"),
        ("in_progress", "In progress"),
        ("resolved", "Resolved"),
        ("wont_fix", "Won't fix"),
    ]
}

/// Issue severity choices for the add form.
#[must_use]
pub fn issue_severity_choices() -> &'static [(&'static str, &'static str)] {
    &[
        ("low", "Low"),
        ("medium", "Medium"),
        ("high", "High"),
        ("critical", "Critical"),
    ]
}

/// Stack component kind choices.
#[must_use]
pub fn component_kind_choices() -> &'static [(&'static str, &'static str)] {
    &[
        ("platform", "Platform"),
        ("language", "Language"),
        ("framework", "Framework"),
        ("runtime", "Runtime"),
        ("sdk", "SDK"),
        ("service", "Service"),
        ("tool", "Tool"),
        ("other", "Other"),
    ]
}

/// Watch kind choices.
#[must_use]
pub fn watch_kind_choices() -> &'static [(&'static str, &'static str)] {
    &[
        ("deadline", "Deadline"),
        ("expiry", "Expiry"),
        ("review", "Review"),
        ("other", "Other"),
    ]
}

/// Android form factor choices.
#[must_use]
pub fn form_factor_choices() -> &'static [(&'static str, &'static str)] {
    &[
        ("phone", "Phone"),
        ("automotive", "Automotive"),
        ("wear", "Wear"),
        ("tv", "TV"),
        ("xr", "XR"),
    ]
}

/// Android store choices.
#[must_use]
pub fn android_store_choices() -> &'static [(&'static str, &'static str)] {
    &[("google_play", "Google Play"), ("none", "None")]
}

/// Readable label for a schema value from a `(key, label)` table.
#[must_use]
pub fn display_label(choices: &[(&'static str, &'static str)], key: &str) -> String {
    choices
        .iter()
        .find(|(candidate, _)| *candidate == key)
        .map_or_else(|| key.to_owned(), |(_, label)| (*label).to_owned())
}

/// Human label for open issues (singular / plural / none).
#[must_use]
pub fn open_issues_label(count: usize) -> String {
    match count {
        0 => "No open issues".to_owned(),
        1 => "1 open issue".to_owned(),
        _ => format!("{count} open issues"),
    }
}

/// Human label for open findings (singular / plural / none).
#[must_use]
pub fn open_findings_label(count: usize) -> String {
    match count {
        0 => "No open findings".to_owned(),
        1 => "1 open finding".to_owned(),
        _ => format!("{count} open findings"),
    }
}

/// Muted label for actively ignored findings.
#[must_use]
pub fn ignored_findings_label(count: usize) -> String {
    match count {
        0 => String::new(),
        1 => "1 ignored".to_owned(),
        _ => format!("{count} ignored"),
    }
}

/// How a stored finding contributes to portfolio counts on `today`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindingCountKind {
    /// Counts toward open severity badges.
    Open,
    /// Counts only as a muted ignored total.
    Ignored,
}

/// Classifies a finding for display counting.
///
/// Resolved findings are omitted. An ignore with `--until` in the past counts
/// as open again (same effective rule the CLI applies after radar reopens it).
#[must_use]
pub fn classify_finding_for_count(
    finding: &StoredFinding,
    today: &str,
) -> Option<FindingCountKind> {
    match finding.state {
        FindingState::Resolved => None,
        FindingState::Open => Some(FindingCountKind::Open),
        FindingState::Ignored => match finding.ignore_until.as_deref() {
            Some(until) if until < today => Some(FindingCountKind::Open),
            _ => Some(FindingCountKind::Ignored),
        },
    }
}

/// Counts open findings by severity (critical…unknown), excluding ignored and resolved.
#[must_use]
pub fn count_open_findings_by_severity(findings: &[StoredFinding], today: &str) -> [usize; 5] {
    let mut counts = [0_usize; 5];
    for finding in findings {
        if classify_finding_for_count(finding, today) != Some(FindingCountKind::Open) {
            continue;
        }
        let index = match finding.severity {
            Severity::Critical => 0,
            Severity::High => 1,
            Severity::Medium => 2,
            Severity::Low => 3,
            Severity::Unknown => 4,
        };
        counts[index] += 1;
    }
    counts
}

/// Counts actively ignored findings (expired ignores are open, not ignored).
#[must_use]
pub fn count_ignored_findings(findings: &[StoredFinding], today: &str) -> usize {
    findings
        .iter()
        .filter(|finding| {
            classify_finding_for_count(finding, today) == Some(FindingCountKind::Ignored)
        })
        .count()
}

/// Remaining-time text for a due date relative to `today` (`YYYY-MM-DD`).
#[must_use]
pub fn remaining_label(today: &str, due: &str) -> String {
    match days_between(today, due) {
        Some(0) => "due today".to_owned(),
        Some(days) if days > 0 => {
            if days == 1 {
                "in 1 day".to_owned()
            } else {
                format!("in {days} days")
            }
        }
        Some(days) => {
            let overdue = -days;
            if overdue == 1 {
                "1 day overdue".to_owned()
            } else {
                format!("{overdue} days overdue")
            }
        }
        None => String::new(),
    }
}

/// Title of the first phase that is not done.
#[must_use]
pub fn current_phase_title(project: &Project) -> Option<String> {
    let plan = project.plan.as_ref()?;
    plan.phases
        .iter()
        .find(|phase| phase_state(&phase.tasks) != PhaseState::Done)
        .map(|phase| phase.title.clone())
}

fn project_count_label(total: usize, loaded: usize, errors: usize) -> String {
    let projects = match total {
        0 => "No projects".to_owned(),
        1 => "1 project".to_owned(),
        _ => format!("{total} projects"),
    };
    format!("{projects} ({loaded} loaded, {errors} with errors)")
}

fn build_card(index: usize, loaded: &LoadedProject, today: &str) -> ProjectCard {
    let findings_by_severity = count_open_findings_by_severity(&loaded.findings, today);
    let finding_badges = finding_count_badges(findings_by_severity);
    let ignored_findings = count_ignored_findings(&loaded.findings, today);
    match &loaded.project {
        Ok(project) => {
            let snap = snapshot(project);
            let next_due = snap.next_due.clone();
            let next_due_remaining = next_due
                .as_ref()
                .map(|due| remaining_label(today, due))
                .filter(|text| !text.is_empty());
            ProjectCard {
                index,
                id: snap.id,
                name: snap.name,
                status: Some(project_status_badge(snap.status)),
                version: snap.current_version,
                progress: project_progress(project),
                open_issues: snap.open_issues,
                open_issues_label: open_issues_label(snap.open_issues),
                findings_by_severity,
                finding_badges,
                ignored_findings,
                next_due,
                next_due_remaining,
                current_phase: current_phase_title(project),
                target_sdk: stored_target_sdk(project),
                category: project.category.clone(),
                platforms: project_platforms(project),
                tags: project.tags.clone().unwrap_or_default(),
                load_error: None,
            }
        }
        Err(error) => ProjectCard {
            index,
            id: String::new(),
            name: loaded.path.file_name().map_or_else(
                || "Unavailable project".to_owned(),
                |name| name.to_string_lossy().into_owned(),
            ),
            status: None,
            version: None,
            progress: ProgressView::empty(),
            open_issues: 0,
            open_issues_label: open_issues_label(0),
            findings_by_severity,
            finding_badges,
            ignored_findings,
            next_due: None,
            next_due_remaining: None,
            current_phase: None,
            target_sdk: None,
            category: None,
            platforms: Vec::new(),
            tags: Vec::new(),
            load_error: Some(error.clone()),
        },
    }
}

/// Non-zero finding severity badges with counts.
#[must_use]
pub fn finding_count_badges(counts: [usize; 5]) -> Vec<(StatusBadge, usize)> {
    Severity::all()
        .iter()
        .zip(counts)
        .filter(|(_, count)| *count > 0)
        .map(|(severity, count)| (severity_badge(*severity), count))
        .collect()
}

fn compare_urgency(left: &ProjectCard, right: &ProjectCard) -> std::cmp::Ordering {
    let left_status = left
        .status
        .as_ref()
        .and_then(|badge| ProjectStatus::parse(badge.key))
        .unwrap_or(ProjectStatus::Idea);
    let right_status = right
        .status
        .as_ref()
        .and_then(|badge| ProjectStatus::parse(badge.key))
        .unwrap_or(ProjectStatus::Idea);
    craftward_core::compare_project_order(
        &craftward_core::ProjectOrderKey {
            status: left_status,
            findings: &left.findings_by_severity,
            due: left.next_due.as_deref(),
            name: &left.name,
            id: &left.id,
        },
        &craftward_core::ProjectOrderKey {
            status: right_status,
            findings: &right.findings_by_severity,
            due: right.next_due.as_deref(),
            name: &right.name,
            id: &right.id,
        },
    )
}

fn database_message(status: &DatabaseStatus) -> Option<String> {
    match status {
        DatabaseStatus::Ready => None,
        DatabaseStatus::Missing => {
            Some("No craftward.db yet. Open findings appear after a radar run.".to_owned())
        }
        DatabaseStatus::Error(message) => Some(format!("Database unavailable: {message}")),
    }
}

fn project_progress(project: &Project) -> ProgressView {
    let Some(plan) = &project.plan else {
        return ProgressView::empty();
    };
    let mut done = 0_usize;
    let mut total = 0_usize;
    for phase in &plan.phases {
        for task in &phase.tasks {
            if task.status == TaskStatus::Dropped {
                continue;
            }
            total += 1;
            if task.status == TaskStatus::Done {
                done += 1;
            }
        }
    }
    let percent = match total {
        0 => 0,
        _ => u32::try_from(done.saturating_mul(100).saturating_add(total / 2) / total).unwrap_or(0),
    };
    ProgressView {
        done,
        total,
        percent,
    }
}

fn stored_target_sdk(project: &Project) -> Option<u32> {
    project
        .stack
        .as_ref()
        .and_then(|stack| stack.android.as_ref())
        .and_then(|android| android.target_sdk)
}

fn component_rows(project: &Project) -> Vec<ComponentRow> {
    project.stack.as_ref().map_or_else(Vec::new, |stack| {
        stack
            .components
            .iter()
            .map(|component| ComponentRow {
                kind: component_kind_badge(component.kind),
                name: component.name.clone(),
                version: component.version.clone(),
                note: component.note.clone(),
                eol_product: component.eol_product.clone(),
            })
            .collect()
    })
}

fn lockfile_rows(project: &Project) -> Vec<String> {
    project
        .stack
        .as_ref()
        .map_or_else(Vec::new, |stack| stack.lockfiles.clone())
}

fn android_detail(project: &Project) -> Option<AndroidDetail> {
    project.stack.as_ref().and_then(|stack| {
        stack.android.as_ref().map(|android| AndroidDetail {
            target_sdk: android.target_sdk,
            min_sdk: android.min_sdk,
            form_factor: android.form_factor.as_str().to_owned(),
            store: android.store.as_str().to_owned(),
            detect_from: android.detect_from.clone(),
        })
    })
}

fn phase_blocks(project: &Project) -> Vec<PhaseBlock> {
    project.plan.as_ref().map_or_else(Vec::new, |plan| {
        let current = current_phase_title(project);
        plan.phases
            .iter()
            .map(|phase| phase_block(phase, current.as_deref()))
            .collect()
    })
}

fn phase_block(phase: &Phase, current_title: Option<&str>) -> PhaseBlock {
    let state = phase_state(&phase.tasks);
    let progress = task_progress(&phase.tasks);
    let initially_expanded =
        state != PhaseState::Done && current_title.is_some_and(|title| title == phase.title);
    PhaseBlock {
        id: phase.id.clone(),
        title: phase.title.clone(),
        goal: phase.goal.clone(),
        status: phase_status_badge(state),
        initially_expanded,
        progress: ProgressView::from_progress(progress),
        tasks: phase.tasks.iter().map(task_row).collect(),
    }
}

fn task_row(task: &Task) -> TaskRow {
    TaskRow {
        id: task.id.clone(),
        title: task.title.clone(),
        status: task_status_badge(task.status),
        due: task.due.clone(),
        note: task.note.clone(),
    }
}

fn open_issue_rows(project: &Project) -> Vec<IssueRow> {
    project.issues.as_ref().map_or_else(Vec::new, |issues| {
        issues
            .iter()
            .filter(|issue| is_open_issue(issue.status))
            .map(issue_row)
            .collect()
    })
}

fn issue_row(issue: &Issue) -> IssueRow {
    IssueRow {
        id: issue.id.clone(),
        title: issue.title.clone(),
        severity: issue_severity_badge(issue.severity),
        status: issue_status_badge(issue.status),
        url: issue.url.clone(),
        note: issue.note.clone(),
    }
}

fn finding_rows(findings: &[StoredFinding], today: &str) -> Vec<FindingRow> {
    findings
        .iter()
        .filter(|finding| {
            classify_finding_for_count(finding, today) == Some(FindingCountKind::Open)
        })
        .map(finding_row)
        .collect()
}

fn finding_row(finding: &StoredFinding) -> FindingRow {
    FindingRow {
        severity: severity_badge(finding.severity),
        kind: finding_kind_badge(finding.kind),
        source: format!(
            "{} {} ({})",
            finding.package, finding.version, finding.vulnerability_id
        ),
        summary: finding.summary.clone(),
    }
}

fn deadline_rows(project: &Project, today: &str) -> Vec<DeadlineRow> {
    let mut rows = Vec::new();
    if let Some(plan) = &project.plan {
        for phase in &plan.phases {
            for task in &phase.tasks {
                if matches!(task.status, TaskStatus::Open | TaskStatus::InProgress)
                    && let Some(due) = &task.due
                {
                    rows.push(DeadlineRow {
                        kind: StatusBadge {
                            key: "task",
                            symbol: symbols::TASK,
                            label: "Task",
                        },
                        title: task.title.clone(),
                        due: due.clone(),
                        remaining: remaining_label(today, due),
                        watch_id: None,
                        watch_kind: None,
                        note: None,
                    });
                }
            }
        }
    }
    if let Some(items) = &project.watch_items {
        for item in items {
            rows.push(watch_row(item, today));
        }
    }
    rows.sort_by(|left, right| left.due.cmp(&right.due).then(left.title.cmp(&right.title)));
    rows
}

fn watch_row(item: &WatchItem, today: &str) -> DeadlineRow {
    let kind = watch_kind_badge(item.kind);
    DeadlineRow {
        watch_kind: Some(kind.key.to_owned()),
        kind,
        title: item.title.clone(),
        due: item.due.clone(),
        remaining: remaining_label(today, &item.due),
        watch_id: Some(item.id.clone()),
        note: item.note.clone(),
    }
}

fn is_open_issue(status: IssueStatus) -> bool {
    matches!(status, IssueStatus::Open | IssueStatus::InProgress)
}

fn project_status_badge(status: ProjectStatus) -> StatusBadge {
    match status {
        ProjectStatus::Idea => StatusBadge {
            key: "idea",
            symbol: symbols::OPEN,
            label: "Idea",
        },
        ProjectStatus::InDevelopment => StatusBadge {
            key: "in_development",
            symbol: symbols::RUNNING,
            label: "In development",
        },
        ProjectStatus::Released => StatusBadge {
            key: "released",
            symbol: symbols::RELEASED,
            label: "Released",
        },
        ProjectStatus::Maintenance => StatusBadge {
            key: "maintenance",
            symbol: symbols::MAINTENANCE,
            label: "Maintenance",
        },
        ProjectStatus::Paused => StatusBadge {
            key: "paused",
            symbol: symbols::PAUSED,
            label: "Paused",
        },
        ProjectStatus::Discontinued => StatusBadge {
            key: "discontinued",
            symbol: symbols::CLOSED,
            label: "Discontinued",
        },
    }
}

fn phase_status_badge(state: PhaseState) -> StatusBadge {
    match state {
        PhaseState::Open => StatusBadge {
            key: "open",
            symbol: symbols::OPEN,
            label: "Open",
        },
        PhaseState::InProgress => StatusBadge {
            key: "in_progress",
            symbol: symbols::RUNNING,
            label: "In progress",
        },
        PhaseState::Done => StatusBadge {
            key: "done",
            symbol: symbols::DONE,
            label: "Done",
        },
    }
}

fn task_status_badge(status: TaskStatus) -> StatusBadge {
    match status {
        TaskStatus::Open => StatusBadge {
            key: "open",
            symbol: symbols::OPEN,
            label: "Open",
        },
        TaskStatus::InProgress => StatusBadge {
            key: "in_progress",
            symbol: symbols::RUNNING,
            label: "In progress",
        },
        TaskStatus::Done => StatusBadge {
            key: "done",
            symbol: symbols::DONE,
            label: "Done",
        },
        TaskStatus::Dropped => StatusBadge {
            key: "dropped",
            symbol: symbols::DROPPED,
            label: "Dropped",
        },
    }
}

fn issue_status_badge(status: IssueStatus) -> StatusBadge {
    match status {
        IssueStatus::Open => StatusBadge {
            key: "open",
            symbol: symbols::OPEN,
            label: "Open",
        },
        IssueStatus::InProgress => StatusBadge {
            key: "in_progress",
            symbol: symbols::RUNNING,
            label: "In progress",
        },
        IssueStatus::Resolved => StatusBadge {
            key: "resolved",
            symbol: symbols::DONE,
            label: "Resolved",
        },
        IssueStatus::WontFix => StatusBadge {
            key: "wont_fix",
            symbol: symbols::DROPPED,
            label: "Won't fix",
        },
    }
}

fn issue_severity_badge(severity: IssueSeverity) -> StatusBadge {
    match severity {
        IssueSeverity::Low => StatusBadge {
            key: "low",
            symbol: symbols::SEVERITY_LOW,
            label: "Low",
        },
        IssueSeverity::Medium => StatusBadge {
            key: "medium",
            symbol: symbols::SEVERITY_MEDIUM,
            label: "Medium",
        },
        IssueSeverity::High => StatusBadge {
            key: "high",
            symbol: symbols::SEVERITY_HIGH,
            label: "High",
        },
        IssueSeverity::Critical => StatusBadge {
            key: "critical",
            symbol: symbols::SEVERITY_CRITICAL,
            label: "Critical",
        },
    }
}

fn severity_badge(severity: Severity) -> StatusBadge {
    match severity {
        Severity::Critical => StatusBadge {
            key: "critical",
            symbol: symbols::SEVERITY_CRITICAL,
            label: "Critical",
        },
        Severity::High => StatusBadge {
            key: "high",
            symbol: symbols::SEVERITY_HIGH,
            label: "High",
        },
        Severity::Medium => StatusBadge {
            key: "medium",
            symbol: symbols::SEVERITY_MEDIUM,
            label: "Medium",
        },
        Severity::Low => StatusBadge {
            key: "low",
            symbol: symbols::SEVERITY_LOW,
            label: "Low",
        },
        Severity::Unknown => StatusBadge {
            key: "unknown",
            symbol: symbols::UNKNOWN,
            label: "Unknown",
        },
    }
}

fn finding_kind_badge(kind: craftward_core::FindingKind) -> StatusBadge {
    let (symbol, label) = match kind {
        craftward_core::FindingKind::Vulnerability => {
            (symbols::KIND_VULNERABILITY, "Vulnerability")
        }
        craftward_core::FindingKind::Malicious => (symbols::KIND_MALICIOUS, "Malicious"),
        craftward_core::FindingKind::Unmaintained => (symbols::KIND_UNMAINTAINED, "Unmaintained"),
        craftward_core::FindingKind::Unsound => (symbols::KIND_UNSOUND, "Unsound"),
        craftward_core::FindingKind::Notice => (symbols::KIND_NOTICE, "Notice"),
        craftward_core::FindingKind::Eol => (symbols::KIND_EOL, "End of life"),
        craftward_core::FindingKind::EolSoon => (symbols::KIND_EOL_SOON, "EOL soon"),
        craftward_core::FindingKind::PlatformDeadline => {
            (symbols::KIND_PLATFORM_DEADLINE, "Play deadline")
        }
        craftward_core::FindingKind::PlatformDeadlineSoon => {
            (symbols::KIND_PLATFORM_DEADLINE_SOON, "Play deadline soon")
        }
    };
    StatusBadge {
        key: kind.as_str(),
        symbol,
        label,
    }
}

fn component_kind_badge(kind: ComponentKind) -> StatusBadge {
    match kind {
        ComponentKind::Platform => StatusBadge {
            key: "platform",
            symbol: symbols::PLATFORM,
            label: "Platform",
        },
        ComponentKind::Language => StatusBadge {
            key: "language",
            symbol: symbols::LANGUAGE,
            label: "Language",
        },
        ComponentKind::Framework => StatusBadge {
            key: "framework",
            symbol: symbols::FRAMEWORK,
            label: "Framework",
        },
        ComponentKind::Runtime => StatusBadge {
            key: "runtime",
            symbol: symbols::RUNTIME,
            label: "Runtime",
        },
        ComponentKind::Sdk => StatusBadge {
            key: "sdk",
            symbol: symbols::SDK,
            label: "SDK",
        },
        ComponentKind::Service => StatusBadge {
            key: "service",
            symbol: symbols::SERVICE,
            label: "Service",
        },
        ComponentKind::Tool => StatusBadge {
            key: "tool",
            symbol: symbols::TOOL,
            label: "Tool",
        },
        ComponentKind::Other => StatusBadge {
            key: "other",
            symbol: symbols::OTHER,
            label: "Other",
        },
    }
}

fn watch_kind_badge(kind: WatchKind) -> StatusBadge {
    match kind {
        WatchKind::Deadline => StatusBadge {
            key: "deadline",
            symbol: symbols::WATCH_DEADLINE,
            label: "Deadline",
        },
        WatchKind::Expiry => StatusBadge {
            key: "expiry",
            symbol: symbols::WATCH_EXPIRY,
            label: "Expiry",
        },
        WatchKind::Review => StatusBadge {
            key: "review",
            symbol: symbols::WATCH_REVIEW,
            label: "Review",
        },
        WatchKind::Other => StatusBadge {
            key: "other",
            symbol: symbols::WATCH_OTHER,
            label: "Other",
        },
    }
}

/// One issue row in the cross-project Issues view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortfolioIssueRow {
    /// Index into [`LoadedPortfolio::projects`].
    pub project_index: usize,
    /// Project id.
    pub project_id: String,
    /// Project display name.
    pub project_name: String,
    /// Issue id.
    pub issue_id: String,
    /// Issue title.
    pub title: String,
    /// Severity badge.
    pub severity: StatusBadge,
    /// Status badge.
    pub status: StatusBadge,
}

/// Filters for the cross-project Issues view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssuesFilter {
    /// When true, only open and in-progress issues.
    pub open_only: bool,
    /// Severity schema word, or empty for all.
    pub severity: String,
    /// Project id, or empty for all.
    pub project_id: String,
    /// Category display string, or empty for all.
    pub category: String,
}

impl Default for IssuesFilter {
    fn default() -> Self {
        Self {
            open_only: true,
            severity: String::new(),
            project_id: String::new(),
            category: String::new(),
        }
    }
}

/// Collects every issue from loaded projects, filtered and sorted.
#[must_use]
pub fn collect_portfolio_issues(
    loaded: &LoadedPortfolio,
    filter: &IssuesFilter,
) -> Vec<PortfolioIssueRow> {
    let mut rows = Vec::new();
    for (project_index, loaded_project) in loaded.projects.iter().enumerate() {
        let Ok(project) = &loaded_project.project else {
            continue;
        };
        let Some(issues) = &project.issues else {
            continue;
        };
        for issue in issues {
            if filter.open_only && !is_open_issue(issue.status) {
                continue;
            }
            if !filter.severity.is_empty()
                && issue_severity_key(issue.severity) != filter.severity.as_str()
            {
                continue;
            }
            if !filter.project_id.is_empty() && project.id != filter.project_id {
                continue;
            }
            if !filter.category.is_empty() {
                let matches = project
                    .category
                    .as_deref()
                    .is_some_and(|value| value.eq_ignore_ascii_case(&filter.category));
                if !matches {
                    continue;
                }
            }
            rows.push(PortfolioIssueRow {
                project_index,
                project_id: project.id.clone(),
                project_name: project.name.clone(),
                issue_id: issue.id.clone(),
                title: issue.title.clone(),
                severity: issue_severity_badge(issue.severity),
                status: issue_status_badge(issue.status),
            });
        }
    }
    rows.sort_by(|left, right| {
        severity_rank(right.severity.key)
            .cmp(&severity_rank(left.severity.key))
            .then_with(|| left.project_name.cmp(&right.project_name))
            .then_with(|| left.title.cmp(&right.title))
            .then_with(|| left.issue_id.cmp(&right.issue_id))
    });
    rows
}

fn issue_severity_key(severity: IssueSeverity) -> &'static str {
    match severity {
        IssueSeverity::Low => "low",
        IssueSeverity::Medium => "medium",
        IssueSeverity::High => "high",
        IssueSeverity::Critical => "critical",
    }
}

/// Distinct category spellings from loaded projects, sorted A–Z (case-insensitive).
#[must_use]
pub fn portfolio_categories(loaded: &LoadedPortfolio) -> Vec<String> {
    let mut categories = Vec::new();
    for project in &loaded.projects {
        let Ok(parsed) = &project.project else {
            continue;
        };
        let Some(category) = &parsed.category else {
            continue;
        };
        if categories
            .iter()
            .any(|known: &String| known.eq_ignore_ascii_case(category))
        {
            continue;
        }
        categories.push(category.clone());
    }
    categories.sort_by_key(|left| left.to_ascii_lowercase());
    categories
}

/// Builds indented Portfolio sidebar rows with counts.
#[must_use]
pub fn portfolio_sidebar_entries(cards: &[ProjectCard]) -> Vec<PortfolioSidebarEntry> {
    let total = cards.len();
    let mut entries = vec![PortfolioSidebarEntry {
        filter: PortfolioSidebarFilter::All,
        label: format!("All ({total})"),
    }];
    let mut category_counts: Vec<(String, usize)> = Vec::new();
    let mut uncategorized = 0;
    let mut errors = 0;
    for card in cards {
        if card.load_error.is_some() {
            errors += 1;
            continue;
        }
        match &card.category {
            Some(category) => {
                if let Some((_, count)) = category_counts
                    .iter_mut()
                    .find(|(known, _)| known.eq_ignore_ascii_case(category))
                {
                    *count += 1;
                } else {
                    category_counts.push((category.clone(), 1));
                }
            }
            None => uncategorized += 1,
        }
    }
    category_counts.sort_by(|(left, _), (right, _)| {
        left.to_ascii_lowercase().cmp(&right.to_ascii_lowercase())
    });
    for (name, count) in category_counts {
        entries.push(PortfolioSidebarEntry {
            filter: PortfolioSidebarFilter::Category(name.clone()),
            label: format!("{name} ({count})"),
        });
    }
    if uncategorized > 0 {
        entries.push(PortfolioSidebarEntry {
            filter: PortfolioSidebarFilter::Uncategorized,
            label: format!("Uncategorized ({uncategorized})"),
        });
    }
    if errors > 0 {
        entries.push(PortfolioSidebarEntry {
            filter: PortfolioSidebarFilter::WithErrors,
            label: format!("With errors ({errors})"),
        });
    }
    entries
}

/// Title for the overview summary when a sidebar filter is active.
#[must_use]
pub fn portfolio_filter_title(filter: &PortfolioSidebarFilter) -> String {
    match filter {
        PortfolioSidebarFilter::All => "Portfolio".to_owned(),
        PortfolioSidebarFilter::Category(name) => format!("Portfolio › {name}"),
        PortfolioSidebarFilter::Uncategorized => "Portfolio › Uncategorized".to_owned(),
        PortfolioSidebarFilter::WithErrors => "Portfolio › With errors".to_owned(),
    }
}

/// Distinct platform labels across loaded cards, sorted.
#[must_use]
pub fn portfolio_platform_options(cards: &[ProjectCard]) -> Vec<String> {
    let mut platforms = Vec::new();
    for card in cards {
        for platform in &card.platforms {
            if platforms
                .iter()
                .any(|known: &String| known.eq_ignore_ascii_case(platform))
            {
                continue;
            }
            platforms.push(platform.clone());
        }
    }
    platforms.sort_by_key(|left| left.to_ascii_lowercase());
    platforms
}

/// Distinct tags across loaded cards, sorted.
#[must_use]
pub fn portfolio_tag_options(cards: &[ProjectCard]) -> Vec<String> {
    let mut tags = Vec::new();
    for card in cards {
        for tag in &card.tags {
            if tags.iter().any(|known: &String| known == tag) {
                continue;
            }
            tags.push(tag.clone());
        }
    }
    tags.sort();
    tags
}

/// Whether a card matches the sidebar category selection and overview filters.
#[must_use]
pub fn card_matches_filters(
    card: &ProjectCard,
    sidebar: &PortfolioSidebarFilter,
    overview: &OverviewFilters,
) -> bool {
    let sidebar_ok = match sidebar {
        PortfolioSidebarFilter::All => true,
        PortfolioSidebarFilter::Category(name) => card
            .category
            .as_deref()
            .is_some_and(|value| value.eq_ignore_ascii_case(name)),
        PortfolioSidebarFilter::Uncategorized => {
            card.load_error.is_none() && card.category.is_none()
        }
        PortfolioSidebarFilter::WithErrors => card.load_error.is_some(),
    };
    if !sidebar_ok {
        return false;
    }
    if !overview.platform.is_empty()
        && !card
            .platforms
            .iter()
            .any(|platform| platform.eq_ignore_ascii_case(&overview.platform))
    {
        return false;
    }
    if !overview.tag.is_empty() && !card.tags.iter().any(|tag| tag == &overview.tag) {
        return false;
    }
    true
}

fn severity_rank(key: &str) -> u8 {
    match key {
        "critical" => 4,
        "high" => 3,
        "medium" => 2,
        "low" => 1,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use craftward_core::{
        AndroidConfig, FindingKind, FindingState, IssueStatus, Plan, Project, ProjectStatus,
        Severity, Stack, StoredFinding, Task, TaskStatus,
    };

    use super::{
        FindingCountKind, HeaderForm, IssueEditForm, IssueForm, IssuesFilter, NewProjectForm,
        OverviewFilters, PhaseForm, PortfolioSidebarFilter, ProgressView, StatusBadge,
        TaskEditForm, TaskForm, android_store_choices, badge_stretches_to_parent, build_detail_at,
        build_portfolio_at, card_matches_filters, classify_finding_for_count,
        collect_portfolio_issues, compare_urgency, count_ignored_findings,
        count_open_findings_by_severity, current_phase_title, display_label, form_factor_choices,
        grid_columns, grid_row_column_slots, ignored_findings_label, issue_status_badge,
        issue_status_choices, longest_status_choice_label, open_findings_label, open_issues_label,
        portfolio_platform_options, portfolio_sidebar_entries, project_progress,
        project_status_badge, relative_to_dir, remaining_label, task_status_badge,
        task_status_choices, task_title_is_muted,
    };
    use crate::load::{DatabaseStatus, LoadedPortfolio, LoadedProject};
    use crate::symbols;
    use crate::view::ProjectCard;

    fn sample_project(id: &str, name: &str, due: Option<&str>) -> Project {
        Project {
            schema_version: 1,
            id: id.to_owned(),
            name: name.to_owned(),
            status: ProjectStatus::InDevelopment,
            source_dir: None,
            description: None,
            current_version: Some("1.0.0".to_owned()),
            category: None,
            tags: None,
            updated: None,
            links: None,
            stack: Some(Stack {
                components: Vec::new(),
                lockfiles: Vec::new(),
                android: Some(AndroidConfig {
                    target_sdk: Some(34),
                    ..AndroidConfig::default()
                }),
            }),
            plan: Some(Plan {
                phases: vec![
                    craftward_core::Phase {
                        id: "done-phase".to_owned(),
                        title: "Foundation".to_owned(),
                        goal: None,
                        tasks: vec![Task {
                            id: "done".to_owned(),
                            title: "Done".to_owned(),
                            status: TaskStatus::Done,
                            due: None,
                            note: None,
                        }],
                    },
                    craftward_core::Phase {
                        id: "current".to_owned(),
                        title: "Release".to_owned(),
                        goal: None,
                        tasks: vec![Task {
                            id: "open".to_owned(),
                            title: "Open".to_owned(),
                            status: TaskStatus::Open,
                            due: due.map(str::to_owned),
                            note: None,
                        }],
                    },
                ],
            }),
            issues: Some(vec![craftward_core::Issue {
                id: "bug".to_owned(),
                title: "Bug".to_owned(),
                severity: craftward_core::IssueSeverity::High,
                status: craftward_core::IssueStatus::Open,
                url: None,
                note: None,
            }]),
            watch_items: None,
        }
    }

    fn finding(project_id: &str, severity: Severity) -> StoredFinding {
        StoredFinding {
            id: 1,
            project_id: project_id.to_owned(),
            ecosystem: "crates.io".to_owned(),
            package: "demo".to_owned(),
            vulnerability_id: "OSV-1".to_owned(),
            version: "1.0.0".to_owned(),
            severity,
            summary: "demo".to_owned(),
            aliases: Vec::new(),
            related_ids: vec!["OSV-1".to_owned()],
            fixed_versions: Vec::new(),
            state: FindingState::Open,
            kind: FindingKind::Vulnerability,
            severity_source: craftward_core::SeveritySource::Unknown,
            first_seen: "2026-01-01T00:00:00Z".to_owned(),
            last_seen: "2026-01-01T00:00:00Z".to_owned(),
            resolved_at: None,
            ignore_reason: None,
            ignore_until: None,
        }
    }

    fn empty_card(id: &str, name: &str, findings: [usize; 5], due: Option<&str>) -> ProjectCard {
        ProjectCard {
            index: 0,
            id: id.to_owned(),
            name: name.to_owned(),
            status: Some(project_status_badge(ProjectStatus::Idea)),
            version: None,
            progress: ProgressView::empty(),
            open_issues: 0,
            open_issues_label: open_issues_label(0),
            findings_by_severity: findings,
            finding_badges: Vec::new(),
            ignored_findings: 0,
            next_due: due.map(str::to_owned),
            next_due_remaining: None,
            current_phase: None,
            target_sdk: None,
            category: None,
            platforms: Vec::new(),
            tags: Vec::new(),
            load_error: None,
        }
    }

    #[test]
    fn sorts_by_status_then_finding_severity_then_due_then_name() {
        let mut released = empty_card("r", "Released", [0, 0, 0, 0, 0], Some("2027-01-01"));
        released.status = Some(project_status_badge(ProjectStatus::Released));
        let critical = empty_card("c", "Zebra", [1, 0, 0, 0, 0], Some("2027-01-01"));
        let due_soon = empty_card("a", "Alpha", [0, 0, 0, 0, 0], Some("2026-01-01"));
        let due_later = empty_card("b", "Beta", [0, 0, 0, 0, 0], Some("2026-06-01"));
        let mut cards = [
            due_later.clone(),
            critical.clone(),
            released.clone(),
            due_soon.clone(),
        ];
        cards.sort_by(compare_urgency);
        assert_eq!(cards[0].id, "r");
        assert_eq!(cards[1].id, "c");
        assert_eq!(cards[2].id, "a");
        assert_eq!(cards[3].id, "b");
    }

    #[test]
    fn progress_counts_non_dropped_tasks() {
        let project = sample_project("p", "P", None);
        let progress = project_progress(&project);
        assert_eq!(progress.done, 1);
        assert_eq!(progress.total, 2);
        assert_eq!(progress.percent, 50);
    }

    #[test]
    fn empty_project_has_zero_progress() {
        let project = Project {
            schema_version: 1,
            id: "empty".to_owned(),
            name: "Empty".to_owned(),
            status: ProjectStatus::Idea,
            source_dir: None,
            description: None,
            current_version: None,
            category: None,
            tags: None,
            updated: None,
            links: None,
            stack: None,
            plan: None,
            issues: None,
            watch_items: None,
        };
        let progress = project_progress(&project);
        assert_eq!(progress, ProgressView::empty());
    }

    #[test]
    fn load_errors_and_missing_database_appear_in_the_view() {
        let loaded = LoadedPortfolio {
            workspace_path: Some(PathBuf::from("/tmp/workspace.json")),
            workspace_fingerprint: None,
            workspace_error: None,
            database_path: Some(PathBuf::from("/tmp/craftward.db")),
            database: DatabaseStatus::Missing,
            projects: vec![
                LoadedProject {
                    path: PathBuf::from("/tmp/good.json"),
                    project: Ok(sample_project("good", "Good", Some("2026-12-01"))),
                    fingerprint: Some(craftward_core::content_fingerprint(b"good")),
                    findings: vec![finding("good", Severity::High)],
                },
                LoadedProject {
                    path: PathBuf::from("/tmp/bad.json"),
                    project: Err("invalid file".to_owned()),
                    fingerprint: None,
                    findings: Vec::new(),
                },
            ],
        };
        let view = build_portfolio_at(&loaded, "2026-11-15");
        assert_eq!(view.summary.project_count, 2);
        assert_eq!(view.summary.loaded_count, 1);
        assert_eq!(view.summary.error_count, 1);
        assert_eq!(view.summary.open_findings, 1);
        assert_eq!(view.summary.open_issues, 1);
        assert!(
            view.database_message
                .as_ref()
                .is_some_and(|text| text.contains("craftward.db"))
        );
        assert!(view.cards.iter().any(|card| card.load_error.is_some()));
        let good = view
            .cards
            .iter()
            .find(|card| card.id == "good")
            .expect("good");
        assert_eq!(good.current_phase.as_deref(), Some("Release"));
        assert_eq!(good.target_sdk, Some(34));
        assert_eq!(
            good.status.as_ref().map(|b| b.label),
            Some("In development")
        );
        assert_eq!(good.next_due_remaining.as_deref(), Some("in 16 days"));
        let detail = build_detail_at(1, &loaded.projects[1], "2026-11-15");
        assert!(detail.load_error.is_some());
        assert!(detail.breadcrumb.starts_with("Portfolio ›"));
    }

    #[test]
    fn database_error_is_surfaced() {
        let loaded = LoadedPortfolio {
            workspace_path: None,
            workspace_fingerprint: None,
            workspace_error: Some("no workspace".to_owned()),
            database_path: None,
            database: DatabaseStatus::Error("locked".to_owned()),
            projects: Vec::new(),
        };
        let view = build_portfolio_at(&loaded, "2026-11-15");
        assert_eq!(view.workspace_error.as_deref(), Some("no workspace"));
        assert!(
            view.database_message
                .as_ref()
                .is_some_and(|text| text.contains("locked"))
        );
        assert_eq!(view.cards, []);
    }

    #[test]
    fn status_badges_use_readable_labels() {
        let badge = project_status_badge(ProjectStatus::InDevelopment);
        assert_eq!(badge.key, "in_development");
        assert_eq!(badge.label, "In development");
        assert_eq!(
            badge.display(),
            format!("{} In development", symbols::RUNNING)
        );
        let done = task_status_badge(TaskStatus::Done);
        assert_eq!(done.display(), format!("{} Done", symbols::DONE));
        assert_eq!(done.symbol, symbols::DONE);
        let released = project_status_badge(ProjectStatus::Released);
        assert_eq!(released.display(), "Released");
        assert_eq!(released.symbol, "");
    }

    #[test]
    fn every_status_has_non_empty_display_text() {
        let task_badges = [
            task_status_badge(TaskStatus::Open),
            task_status_badge(TaskStatus::InProgress),
            task_status_badge(TaskStatus::Done),
            task_status_badge(TaskStatus::Dropped),
        ];
        for badge in task_badges {
            assert!(!badge.label.is_empty(), "empty label for {}", badge.key);
            assert!(
                !badge.display().is_empty(),
                "empty display for {}",
                badge.key
            );
        }
        let issue_badges = [
            issue_status_badge(IssueStatus::Open),
            issue_status_badge(IssueStatus::InProgress),
            issue_status_badge(IssueStatus::Resolved),
            issue_status_badge(IssueStatus::WontFix),
        ];
        for badge in issue_badges {
            assert!(!badge.label.is_empty(), "empty label for {}", badge.key);
            assert!(
                !badge.display().is_empty(),
                "empty display for {}",
                badge.key
            );
        }
        assert_eq!(
            longest_status_choice_label(task_status_choices()),
            "In progress"
        );
        assert_eq!(
            longest_status_choice_label(issue_status_choices()),
            "In progress"
        );
        assert!(task_title_is_muted("done"));
        assert!(task_title_is_muted("dropped"));
        assert!(!task_title_is_muted("open"));
    }

    #[test]
    fn badges_do_not_stretch_to_parent_width() {
        assert!(!badge_stretches_to_parent());
    }

    #[test]
    fn last_grid_row_keeps_full_column_count() {
        assert_eq!(grid_row_column_slots(3, 1), 3);
        assert_eq!(grid_row_column_slots(3, 2), 3);
        assert_eq!(grid_row_column_slots(2, 1), 2);
        assert_eq!(grid_row_column_slots(1, 1), 1);
    }

    #[test]
    fn task_form_requires_title_and_valid_due() {
        let mut form = TaskForm::new("phase-a");
        assert!(!form.can_save());
        assert_eq!(form.validation_message(), Some("Title is required"));
        form.title = "  Ship it  ".to_owned();
        assert!(form.can_save());
        form.due = "15-11-2026".to_owned();
        assert!(!form.can_save());
        form.due = "2026-11-15".to_owned();
        assert!(form.can_save());
        assert_eq!(form.title_for_edit(), "Ship it");
        assert_eq!(form.due_iso().as_deref(), Some("2026-11-15"));
        form.due = "15.11.2026".to_owned();
        assert_eq!(form.due_iso().as_deref(), Some("2026-11-15"));
    }

    #[test]
    fn issue_form_requires_title_and_known_severity() {
        let mut form = IssueForm::default();
        assert!(!form.can_save());
        form.title = "Broken glyph".to_owned();
        assert!(form.can_save());
        form.severity = "urgent".to_owned();
        assert!(!form.can_save());
        form.severity = "high".to_owned();
        form.note = "  note  ".to_owned();
        assert!(form.can_save());
        assert_eq!(form.note_for_edit(), Some("note"));
    }

    #[test]
    fn stack_and_watch_forms_validate_fields() {
        use super::{
            ComponentForm, LockfileForm, WatchForm, relative_to_dir, validate_lockfile_relative,
        };
        use std::fs;

        let mut component = ComponentForm::default();
        assert!(!component.can_save());
        component.name = "Rust".to_owned();
        assert!(component.can_save());
        component.kind = "weird".to_owned();
        assert!(!component.can_save());

        let mut watch = WatchForm::default();
        assert!(!watch.can_save());
        watch.title = "Renew".to_owned();
        watch.due = "2027-06-30".to_owned();
        assert!(watch.can_save());
        watch.due = "bad".to_owned();
        assert!(!watch.can_save());

        let root = std::env::temp_dir().join(format!(
            "craftward-lockform-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("Cargo.lock"), "").unwrap();
        assert_eq!(
            validate_lockfile_relative("yarn.lock", &root),
            Some("Lockfile must be Cargo.lock or package-lock.json")
        );
        assert_eq!(
            validate_lockfile_relative("missing/Cargo.lock", &root),
            Some("Lockfile does not exist under source_dir")
        );
        assert!(validate_lockfile_relative("Cargo.lock", &root).is_none());
        assert_eq!(
            relative_to_dir(&root.join("Cargo.lock"), &root).as_deref(),
            Some("Cargo.lock")
        );
        let form = LockfileForm {
            path: "Cargo.lock".to_owned(),
        };
        assert!(form.can_save(&root));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn new_project_form_suggests_id_until_manual_edit() {
        let mut form = NewProjectForm {
            name: "Harbor Light".to_owned(),
            ..NewProjectForm::default()
        };
        form.suggest_id_from_name();
        assert_eq!(form.id, "harbor-light");
        form.name = "Prüfung".to_owned();
        form.suggest_id_from_name();
        assert_eq!(form.id, "pruefung");
        form.id = "custom".to_owned();
        form.mark_id_manual();
        form.name = "Other".to_owned();
        form.suggest_id_from_name();
        assert_eq!(form.id, "custom");
        let taken = vec!["custom".to_owned()];
        assert_eq!(
            form.id_error(&taken),
            Some("Id is already in the workspace")
        );
        form.id = "ok-id".to_owned();
        form.directory = "/tmp/p".to_owned();
        assert!(form.can_save(&[]));
    }

    #[test]
    fn header_form_keeps_input_and_reports_field_errors() {
        let mut form = HeaderForm {
            name: String::new(),
            status: "idea".to_owned(),
            version: "1.0".to_owned(),
            description: "desc".to_owned(),
            source_dir: "src".to_owned(),
            category: String::new(),
            tags: "a, b".to_owned(),
            links: "Docs|https://example.com".to_owned(),
            category_unify_hint: None,
        };
        assert_eq!(form.name_error(), Some("Name is required"));
        assert!(!form.can_save());
        form.name = " Harbor ".to_owned();
        assert!(form.can_save());
        form.status = "nope".to_owned();
        assert_eq!(
            form.status_error(),
            Some("Status is not a known lifecycle value")
        );
        form.status = "in_development".to_owned();
        form.links = "bad".to_owned();
        assert_eq!(form.links_error(), Some("Each link must be Label|URL"));
        form.links = "Docs|ftp://example.com".to_owned();
        assert_eq!(
            form.links_error(),
            Some("Link URL must start with http:// or https://")
        );
        form.links = "Docs|https://example.com\nRepo|https://example.com/r".to_owned();
        assert!(form.can_save());
        assert_eq!(form.name_for_edit(), "Harbor");
        assert_eq!(form.tags_for_edit(), vec!["a".to_owned(), "b".to_owned()]);
        assert_eq!(
            form.links_for_edit(),
            vec![
                ("Docs".to_owned(), "https://example.com".to_owned()),
                ("Repo".to_owned(), "https://example.com/r".to_owned()),
            ]
        );
    }

    #[test]
    fn phase_and_edit_forms_validate_fields() {
        let mut phase = PhaseForm::default();
        assert_eq!(phase.title_error(), Some("Title is required"));
        phase.title = " Build ".to_owned();
        phase.id = " build ".to_owned();
        phase.goal = " Ship ".to_owned();
        assert!(phase.can_save());
        assert_eq!(phase.id_for_edit(), Some("build"));
        assert_eq!(phase.goal_for_edit(), Some("Ship"));

        let mut task = TaskEditForm {
            title: String::new(),
            due: "nope".to_owned(),
            note: "keep".to_owned(),
        };
        assert_eq!(task.title_error(), Some("Title is required"));
        task.title = "Ship".to_owned();
        assert!(task.due_error().is_some());
        task.due = "2026-12-01".to_owned();
        assert!(task.can_save());
        task.due = "01.12.2026".to_owned();
        assert!(task.can_save());
        assert_eq!(task.due_iso().as_deref(), Some("2026-12-01"));
        assert_eq!(task.note, "keep");

        let mut issue = IssueEditForm::default();
        assert_eq!(issue.title_error(), Some("Title is required"));
        issue.title = "Contrast".to_owned();
        issue.severity = "urgent".to_owned();
        assert_eq!(
            issue.severity_error(),
            Some("Severity must be low, medium, high, or critical")
        );
        issue.severity = "high".to_owned();
        issue.url = "ftp://x".to_owned();
        assert_eq!(
            issue.url_error(),
            Some("URL must start with http:// or https://")
        );
        issue.url = "https://example.com".to_owned();
        issue.note = "keep".to_owned();
        assert!(issue.can_save());
        assert_eq!(issue.note, "keep");
    }

    #[test]
    fn detail_exposes_header_fields_for_edit_form() {
        let mut project = sample_project("p", "P", None);
        project.description = Some("Overview text".to_owned());
        project.source_dir = Some("crates/p".to_owned());
        project.tags = Some(vec!["desktop".to_owned()]);
        project.links = Some(vec![craftward_core::Link {
            label: "Docs".to_owned(),
            url: "https://example.com".to_owned(),
        }]);
        let loaded = LoadedProject {
            path: PathBuf::from("/tmp/p.json"),
            project: Ok(project),
            fingerprint: Some(craftward_core::content_fingerprint(b"p")),
            findings: Vec::new(),
        };
        let detail = build_detail_at(0, &loaded, "2026-11-15");
        assert_eq!(detail.version.as_deref(), Some("1.0.0"));
        assert_eq!(detail.source_dir.as_deref(), Some("crates/p"));
        assert_eq!(detail.tags, vec!["desktop".to_owned()]);
        assert_eq!(
            detail.links,
            vec![("Docs".to_owned(), "https://example.com".to_owned())]
        );
        let form = HeaderForm::from_detail(&detail);
        assert_eq!(form.name, "P");
        assert_eq!(form.version, "1.0.0");
        assert!(form.can_save());
    }

    #[test]
    fn pluralization_covers_zero_one_and_many() {
        assert_eq!(open_issues_label(0), "No open issues");
        assert_eq!(open_issues_label(1), "1 open issue");
        assert_eq!(open_issues_label(2), "2 open issues");
        assert_eq!(open_findings_label(0), "No open findings");
        assert_eq!(open_findings_label(1), "1 open finding");
        assert_eq!(open_findings_label(3), "3 open findings");
    }

    #[test]
    fn current_phase_skips_completed_phases() {
        let project = sample_project("p", "P", None);
        assert_eq!(current_phase_title(&project).as_deref(), Some("Release"));
    }

    #[test]
    fn remaining_time_uses_injected_today() {
        assert_eq!(remaining_label("2026-11-15", "2026-11-15"), "due today");
        assert_eq!(remaining_label("2026-11-15", "2026-11-16"), "in 1 day");
        assert_eq!(remaining_label("2026-11-15", "2026-11-20"), "in 5 days");
        assert_eq!(remaining_label("2026-11-15", "2026-11-14"), "1 day overdue");
        assert_eq!(
            remaining_label("2026-11-15", "2026-11-10"),
            "5 days overdue"
        );
    }

    #[test]
    fn grid_columns_follow_width_breakpoints() {
        assert_eq!(grid_columns(500.0), 1);
        assert_eq!(grid_columns(899.0), 1);
        assert_eq!(grid_columns(900.0), 2);
        assert_eq!(grid_columns(1399.0), 2);
        assert_eq!(grid_columns(1400.0), 3);
    }

    #[test]
    fn detail_collapses_done_phases_by_default() {
        let project = sample_project("p", "P", Some("2026-12-01"));
        let loaded = LoadedProject {
            path: PathBuf::from("/tmp/p.json"),
            project: Ok(project),
            fingerprint: Some(craftward_core::content_fingerprint(b"p")),
            findings: Vec::new(),
        };
        let detail = build_detail_at(0, &loaded, "2026-11-15");
        assert_eq!(detail.phases[0].status.key, "done");
        assert!(!detail.phases[0].initially_expanded);
        assert_eq!(detail.phases[1].status.key, "open");
        assert!(detail.phases[1].initially_expanded);
        assert!(
            detail
                .overview_fields
                .iter()
                .any(|field| field.label == "Android targetSdk" && field.value == "34")
        );
        let _badge: StatusBadge = detail.phases[1].status.clone();
    }

    #[test]
    fn portfolio_issues_filter_and_sort() {
        use craftward_core::{Issue, IssueSeverity, IssueStatus};

        let mut alpha = sample_project("alpha", "Alpha", None);
        alpha.issues = Some(vec![
            Issue {
                id: "a-low".to_owned(),
                title: "Zed".to_owned(),
                severity: IssueSeverity::Low,
                status: IssueStatus::Open,
                url: None,
                note: None,
            },
            Issue {
                id: "a-crit".to_owned(),
                title: "First".to_owned(),
                severity: IssueSeverity::Critical,
                status: IssueStatus::Resolved,
                url: None,
                note: None,
            },
        ]);
        let mut beta = sample_project("beta", "Beta", None);
        beta.issues = Some(vec![Issue {
            id: "b-high".to_owned(),
            title: "Middle".to_owned(),
            severity: IssueSeverity::High,
            status: IssueStatus::InProgress,
            url: None,
            note: None,
        }]);
        let loaded = LoadedPortfolio {
            workspace_path: None,
            workspace_error: None,
            workspace_fingerprint: None,
            database_path: None,
            database: DatabaseStatus::Missing,
            projects: vec![
                LoadedProject {
                    path: PathBuf::from("/tmp/alpha.json"),
                    project: Ok(alpha),
                    fingerprint: None,
                    findings: Vec::new(),
                },
                LoadedProject {
                    path: PathBuf::from("/tmp/beta.json"),
                    project: Ok(beta),
                    fingerprint: None,
                    findings: Vec::new(),
                },
            ],
        };
        let open = collect_portfolio_issues(&loaded, &IssuesFilter::default());
        assert_eq!(
            open.iter()
                .map(|row| row.issue_id.as_str())
                .collect::<Vec<_>>(),
            ["b-high", "a-low"]
        );
        let all = collect_portfolio_issues(
            &loaded,
            &IssuesFilter {
                open_only: false,
                severity: String::new(),
                project_id: String::new(),
                category: String::new(),
            },
        );
        assert_eq!(
            all.iter()
                .map(|row| row.issue_id.as_str())
                .collect::<Vec<_>>(),
            ["a-crit", "b-high", "a-low"]
        );
        let high = collect_portfolio_issues(
            &loaded,
            &IssuesFilter {
                open_only: false,
                severity: "high".to_owned(),
                project_id: String::new(),
                category: String::new(),
            },
        );
        assert_eq!(high.len(), 1);
        assert_eq!(high[0].issue_id, "b-high");
    }

    #[test]
    fn display_label_maps_android_schema_values() {
        assert_eq!(
            display_label(android_store_choices(), "google_play"),
            "Google Play"
        );
        assert_eq!(display_label(form_factor_choices(), "phone"), "Phone");
    }

    #[test]
    fn relative_path_rejects_outside_base() {
        let base = PathBuf::from("/tmp/project/src");
        let outside = PathBuf::from("/tmp/elsewhere/Cargo.lock");
        assert_eq!(relative_to_dir(&outside, &base), None);
    }

    #[test]
    fn sidebar_counts_categories_and_errors() {
        let mut ok = sample_project("a", "A", None);
        ok.category = Some("Security Tools".to_owned());
        let mut other = sample_project("b", "B", None);
        other.category = Some("security tools".to_owned());
        let loaded = LoadedPortfolio {
            workspace_path: None,
            workspace_fingerprint: None,
            workspace_error: None,
            database_path: None,
            database: DatabaseStatus::Missing,
            projects: vec![
                LoadedProject {
                    path: PathBuf::from("/tmp/a.json"),
                    project: Ok(ok),
                    fingerprint: None,
                    findings: Vec::new(),
                },
                LoadedProject {
                    path: PathBuf::from("/tmp/b.json"),
                    project: Ok(other),
                    fingerprint: None,
                    findings: Vec::new(),
                },
                LoadedProject {
                    path: PathBuf::from("/tmp/bad.json"),
                    project: Err("broken".to_owned()),
                    fingerprint: None,
                    findings: Vec::new(),
                },
                LoadedProject {
                    path: PathBuf::from("/tmp/plain.json"),
                    project: Ok(sample_project("c", "C", None)),
                    fingerprint: None,
                    findings: Vec::new(),
                },
            ],
        };
        let view = build_portfolio_at(&loaded, "2026-11-17");
        let entries = portfolio_sidebar_entries(&view.cards);
        assert_eq!(entries[0].label, "All (4)");
        assert!(
            entries
                .iter()
                .any(|entry| entry.label.starts_with("Security Tools ("))
        );
        assert!(
            entries
                .iter()
                .any(|entry| entry.label == "Uncategorized (1)")
        );
        assert!(entries.iter().any(|entry| entry.label == "With errors (1)"));
        let security = PortfolioSidebarFilter::Category("Security Tools".to_owned());
        let matched: Vec<_> = view
            .cards
            .iter()
            .filter(|card| card_matches_filters(card, &security, &OverviewFilters::default()))
            .collect();
        assert_eq!(matched.len(), 2);
        let errors_only: Vec<_> = view
            .cards
            .iter()
            .filter(|card| {
                card_matches_filters(
                    card,
                    &PortfolioSidebarFilter::WithErrors,
                    &OverviewFilters::default(),
                )
            })
            .collect();
        assert_eq!(errors_only.len(), 1);
        assert_eq!(
            portfolio_platform_options(&view.cards),
            vec!["Android".to_owned()]
        );
    }

    #[test]
    fn header_category_unifies_letter_case() {
        let mut form = HeaderForm {
            name: "Harbor".to_owned(),
            status: "idea".to_owned(),
            version: String::new(),
            description: String::new(),
            source_dir: String::new(),
            category: "security  tools".to_owned(),
            tags: String::new(),
            links: String::new(),
            category_unify_hint: None,
        };
        let existing = vec!["Security Tools".to_owned()];
        let resolved = form.resolve_category(&existing).unwrap();
        assert_eq!(resolved.as_deref(), Some("Security Tools"));
        assert!(form.category_unify_hint.is_some());
    }

    #[test]
    fn findings_count_only_open_and_expired_ignores() {
        let today = "2026-11-17";
        let mut open = finding("demo", Severity::High);
        open.state = FindingState::Open;
        let mut ignored = finding("demo", Severity::Critical);
        ignored.state = FindingState::Ignored;
        ignored.ignore_reason = Some("later".to_owned());
        ignored.ignore_until = Some("2026-12-01".to_owned());
        let mut expired = finding("demo", Severity::Medium);
        expired.state = FindingState::Ignored;
        expired.ignore_until = Some("2026-11-01".to_owned());
        let mut resolved = finding("demo", Severity::Low);
        resolved.state = FindingState::Resolved;
        resolved.resolved_at = Some("2026-11-01T00:00:00Z".to_owned());
        let rows = [open, ignored, expired, resolved];
        assert_eq!(
            classify_finding_for_count(&rows[0], today),
            Some(FindingCountKind::Open)
        );
        assert_eq!(
            classify_finding_for_count(&rows[1], today),
            Some(FindingCountKind::Ignored)
        );
        assert_eq!(
            classify_finding_for_count(&rows[2], today),
            Some(FindingCountKind::Open)
        );
        assert_eq!(classify_finding_for_count(&rows[3], today), None);
        assert_eq!(
            count_open_findings_by_severity(&rows, today),
            [0, 1, 1, 0, 0]
        );
        assert_eq!(count_ignored_findings(&rows, today), 1);
        assert_eq!(ignored_findings_label(1), "1 ignored");
        assert_eq!(ignored_findings_label(3), "3 ignored");
        assert_eq!(open_findings_label(0), "No open findings");

        let loaded = LoadedPortfolio {
            workspace_path: None,
            workspace_fingerprint: None,
            workspace_error: None,
            database_path: None,
            database: DatabaseStatus::Ready,
            projects: vec![LoadedProject {
                path: PathBuf::from("/tmp/demo.json"),
                project: Ok(sample_project("demo", "Demo", None)),
                fingerprint: None,
                findings: rows.to_vec(),
            }],
        };
        let view = build_portfolio_at(&loaded, today);
        assert_eq!(view.summary.open_findings, 2);
        assert_eq!(view.summary.ignored_findings, 1);
        assert_eq!(view.cards[0].finding_badges.len(), 2);
        assert_eq!(view.cards[0].ignored_findings, 1);
    }
}
