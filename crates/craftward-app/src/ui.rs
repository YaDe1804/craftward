//! egui surface that renders the view model.

use eframe::egui::{
    self, Align, Button, CentralPanel, CollapsingHeader, Color32, ComboBox, Context, CornerRadius,
    Frame, Key, Label, Layout, Margin, Panel, ProgressBar, RichText, ScrollArea, Sense, Stroke,
    TextEdit, Theme, ThemePreference, Ui, Vec2,
};

use crate::actions::{
    SaveResult, save_issue_status, save_new_issue, save_new_task, save_task_status,
};
use crate::load::{LoadedPortfolio, LoadedProject, load_portfolio};
use crate::theme::{
    self, ACCENT, BACKGROUND, CARD, CARD_GAP, CARD_INNER, CARD_PADDING, CARD_ROUNDING,
    DETAIL_TWO_COLUMNS_FROM, DONE, HEADING, LINE, MUTED, PAGE_MARGIN, PROGRESS_TRACK, RUNNING,
    SEVERITY_CRITICAL, SEVERITY_HIGH, SEVERITY_LOW, SEVERITY_MEDIUM, TEXT, TYPE,
};
use crate::view::{
    FindingRow, IssueForm, PortfolioView, ProgressView, ProjectCard, ProjectDetail, StatusBadge,
    TaskForm, badge_stretches_to_parent, build_detail, build_portfolio, finding_count_badges,
    grid_columns, grid_row_column_slots, issue_severity_choices, issue_status_choices,
    longest_status_choice_label, open_findings_label, task_status_choices, task_title_is_muted,
};

/// Which screen is visible.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    /// Portfolio overview.
    Overview,
    /// Detail for `LoadedPortfolio::projects[index]`.
    Detail {
        /// Project index.
        index: usize,
    },
}

/// Where a save error should appear.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ErrorSite {
    /// Task status menu.
    Task {
        /// Phase id.
        phase_id: String,
        /// Task id.
        task_id: String,
    },
    /// Issue status menu.
    Issue {
        /// Issue id.
        issue_id: String,
    },
    /// Open task form.
    TaskForm,
    /// Open issue form.
    IssueForm,
}

/// Detail-view editing widgets (no egui types).
#[derive(Debug, Default)]
struct DetailEdit {
    /// Open task form, when adding to a phase.
    task_form: Option<TaskForm>,
    /// Open issue form.
    issue_form: Option<IssueForm>,
    /// Last save error message.
    error: Option<String>,
    /// Whether the error is an external-change conflict.
    conflict: bool,
    /// Which control owns [`Self::error`].
    error_site: Option<ErrorSite>,
}

impl DetailEdit {
    fn clear_message(&mut self) {
        self.error = None;
        self.conflict = false;
        self.error_site = None;
    }

    fn clear_all(&mut self) {
        self.task_form = None;
        self.issue_form = None;
        self.clear_message();
    }

    fn set_error(&mut self, site: ErrorSite, message: String, conflict: bool) {
        self.error_site = Some(site);
        self.error = Some(message);
        self.conflict = conflict;
    }
}

/// Action produced by the detail view widgets.
enum DetailAction {
    /// Set a task status.
    SetTaskStatus {
        phase_id: String,
        task_id: String,
        status: String,
    },
    /// Set an issue status.
    SetIssueStatus { issue_id: String, status: String },
    /// Persist the open task form.
    SaveTask,
    /// Persist the open issue form.
    SaveIssue,
    /// Reload after a conflict.
    Reload,
}

/// The Craftward desktop application.
pub struct CraftwardApp {
    /// Last loaded data.
    loaded: LoadedPortfolio,
    /// Derived overview.
    portfolio: PortfolioView,
    /// Current screen.
    screen: Screen,
    /// Detail editing widgets.
    edit: DetailEdit,
}

impl CraftwardApp {
    /// Loads data and applies the theme.
    #[must_use]
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        apply_theme(&cc.egui_ctx);
        let loaded = load_portfolio();
        let portfolio = build_portfolio(&loaded);
        Self {
            loaded,
            portfolio,
            screen: Screen::Overview,
            edit: DetailEdit::default(),
        }
    }

    fn reload(&mut self) {
        self.loaded = load_portfolio();
        self.portfolio = build_portfolio(&self.loaded);
        self.edit.clear_all();
        if let Screen::Detail { index } = self.screen
            && index >= self.loaded.projects.len()
        {
            self.screen = Screen::Overview;
        }
    }

    fn apply_detail_action(&mut self, index: usize, action: DetailAction) {
        match action {
            DetailAction::Reload => self.reload(),
            DetailAction::SetTaskStatus {
                phase_id,
                task_id,
                status,
            } => {
                let site = ErrorSite::Task {
                    phase_id: phase_id.clone(),
                    task_id: task_id.clone(),
                };
                self.run_save(index, site, |path, fingerprint| {
                    save_task_status(path, fingerprint, &phase_id, &task_id, &status)
                });
            }
            DetailAction::SetIssueStatus { issue_id, status } => {
                let site = ErrorSite::Issue {
                    issue_id: issue_id.clone(),
                };
                self.run_save(index, site, |path, fingerprint| {
                    save_issue_status(path, fingerprint, &issue_id, &status)
                });
            }
            DetailAction::SaveTask => {
                let Some(form) = self.edit.task_form.clone() else {
                    return;
                };
                self.run_save(index, ErrorSite::TaskForm, |path, fingerprint| {
                    save_new_task(path, fingerprint, &form)
                });
            }
            DetailAction::SaveIssue => {
                let Some(form) = self.edit.issue_form.clone() else {
                    return;
                };
                self.run_save(index, ErrorSite::IssueForm, |path, fingerprint| {
                    save_new_issue(path, fingerprint, &form)
                });
            }
        }
    }

    fn run_save(
        &mut self,
        index: usize,
        site: ErrorSite,
        save: impl FnOnce(&std::path::Path, &craftward_core::ContentFingerprint) -> SaveResult,
    ) {
        let Some(project) = self.loaded.projects.get(index) else {
            return;
        };
        let Some(fingerprint) = project.fingerprint.clone() else {
            self.edit.set_error(
                site,
                "project file is not available for editing".to_owned(),
                false,
            );
            return;
        };
        let path = project.path.clone();
        match save(&path, &fingerprint) {
            SaveResult::Saved => {
                let stay = self.screen;
                self.reload();
                self.screen = stay;
            }
            SaveResult::Failed { message, conflict } => {
                self.edit.set_error(site, message, conflict);
            }
        }
    }
}

impl eframe::App for CraftwardApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.handle_escape(ui);
        self.draw_top_bar(ui);
        Panel::bottom("craftward_footer")
            .frame(
                Frame::NONE
                    .fill(BACKGROUND)
                    .inner_margin(Margin::symmetric(PAGE_MARGIN, 8))
                    .stroke(Stroke::new(1.0, LINE)),
            )
            .show(ui, |ui| {
                draw_footer(ui, &self.portfolio);
            });
        let action = self.draw_central(ui);
        if let (Screen::Detail { index }, Some(action)) = (self.screen, action) {
            self.apply_detail_action(index, action);
        }
    }
}

impl CraftwardApp {
    fn handle_escape(&mut self, ui: &Ui) {
        if !ui.ctx().input(|input| input.key_pressed(Key::Escape)) {
            return;
        }
        if self.edit.task_form.is_some() || self.edit.issue_form.is_some() {
            self.edit.task_form = None;
            self.edit.issue_form = None;
            self.edit.clear_message();
        } else if matches!(self.screen, Screen::Detail { .. }) {
            self.screen = Screen::Overview;
            self.edit.clear_all();
        }
    }

    fn draw_top_bar(&mut self, ui: &mut Ui) {
        Panel::top("craftward_top")
            .frame(
                Frame::NONE
                    .fill(BACKGROUND)
                    .inner_margin(Margin::symmetric(PAGE_MARGIN, 10))
                    .stroke(Stroke::new(1.0, LINE)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let title = match self.screen {
                        Screen::Overview => "Craftward".to_owned(),
                        Screen::Detail { index } => self.loaded.projects.get(index).map_or_else(
                            || "Portfolio".to_owned(),
                            |project| build_detail(index, project).breadcrumb,
                        ),
                    };
                    ui.label(
                        RichText::new(title)
                            .font(TYPE.title_id())
                            .color(TEXT)
                            .strong(),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add(Button::new(
                                RichText::new("Reload").font(TYPE.body_id()).color(ACCENT),
                            ))
                            .on_hover_text("Reload workspace, projects, and findings")
                            .clicked()
                        {
                            self.reload();
                        }
                        if matches!(self.screen, Screen::Detail { .. })
                            && ui
                                .add(Button::new(
                                    RichText::new("Back").font(TYPE.body_id()).color(ACCENT),
                                ))
                                .on_hover_text("Return to the portfolio (Esc)")
                                .clicked()
                        {
                            self.screen = Screen::Overview;
                            self.edit.clear_all();
                        }
                    });
                });
            });
    }

    fn draw_central(&mut self, ui: &mut Ui) -> Option<DetailAction> {
        let mut action = None;
        CentralPanel::default()
            .frame(
                Frame::NONE
                    .fill(BACKGROUND)
                    .inner_margin(Margin::same(PAGE_MARGIN)),
            )
            .show(ui, |ui| match self.screen {
                Screen::Overview => {
                    if let Some(index) = draw_overview(ui, &self.portfolio) {
                        self.edit.clear_all();
                        self.screen = Screen::Detail { index };
                    }
                }
                Screen::Detail { index } => {
                    if let Some(project) = self.loaded.projects.get(index) {
                        let detail = build_detail(index, project);
                        action = draw_detail(ui, &detail, project, &mut self.edit);
                    } else {
                        ui.label(
                            RichText::new("That project is no longer listed.")
                                .font(TYPE.body_id())
                                .color(MUTED),
                        );
                        if ui
                            .add(Button::new(
                                RichText::new("Back").font(TYPE.body_id()).color(ACCENT),
                            ))
                            .clicked()
                        {
                            self.screen = Screen::Overview;
                            self.edit.clear_all();
                        }
                    }
                }
            });
        action
    }
}

fn apply_theme(ctx: &Context) {
    ctx.set_theme(ThemePreference::Dark);
    ctx.set_style_of(Theme::Dark, theme::style());
}

fn draw_footer(ui: &mut Ui, portfolio: &PortfolioView) {
    if let Some(error) = &portfolio.workspace_error {
        ui.colored_label(SEVERITY_CRITICAL, error);
        return;
    }
    let mut parts = Vec::new();
    if let Some(path) = &portfolio.workspace_path {
        parts.push(format!("Workspace: {path}"));
    }
    if let Some(path) = &portfolio.database_path {
        parts.push(format!("Database: {path}"));
    }
    if parts.is_empty() {
        ui.label(
            RichText::new("No workspace or database path resolved.")
                .font(TYPE.secondary_id())
                .color(MUTED),
        );
    } else {
        ui.label(
            RichText::new(parts.join("  ·  "))
                .font(TYPE.secondary_id())
                .color(MUTED),
        )
        .on_hover_text(parts.join("\n"));
    }
    if let Some(message) = &portfolio.database_message {
        ui.label(
            RichText::new(message)
                .font(TYPE.secondary_id())
                .color(RUNNING),
        );
    }
}

fn draw_overview(ui: &mut Ui, portfolio: &PortfolioView) -> Option<usize> {
    let mut open = None;
    ScrollArea::vertical().show(ui, |ui| {
        draw_summary(ui, &portfolio.summary);
        ui.add_space(CARD_GAP);

        if portfolio.cards.is_empty() {
            ui.label(
                RichText::new("No projects in the workspace yet. Use craftward init or add.")
                    .font(TYPE.body_id())
                    .color(MUTED),
            );
            return;
        }

        let columns = grid_columns(ui.available_width());
        let mut row: Vec<&ProjectCard> = Vec::with_capacity(columns);
        for card in &portfolio.cards {
            row.push(card);
            if row.len() == columns {
                if let Some(index) = draw_card_row(ui, &row, columns) {
                    open = Some(index);
                }
                row.clear();
            }
        }
        if !row.is_empty()
            && let Some(index) = draw_card_row(ui, &row, columns)
        {
            open = Some(index);
        }
    });
    open
}

fn draw_card_row(ui: &mut Ui, cards: &[&ProjectCard], columns: usize) -> Option<usize> {
    let mut open = None;
    let slots = grid_row_column_slots(columns, cards.len());
    ui.columns(slots, |cols| {
        for (column, card) in cols.iter_mut().zip(cards.iter()) {
            if draw_card(column, card) {
                open = Some(card.index);
            }
        }
    });
    ui.add_space(CARD_GAP);
    open
}

fn draw_summary(ui: &mut Ui, summary: &crate::view::PortfolioSummary) {
    card_frame().show(ui, |ui| {
        ui.label(
            RichText::new("Portfolio")
                .font(TYPE.section_id())
                .color(HEADING)
                .strong(),
        );
        ui.label(
            RichText::new(&summary.summary_line)
                .font(TYPE.body_id())
                .color(TEXT),
        );
        if summary.open_findings > 0 {
            ui.horizontal_wrapped(|ui| {
                for (badge, count) in finding_count_badges(summary.findings_by_severity) {
                    draw_badge(ui, &badge, Some(count));
                }
            });
        }
    });
}

fn draw_card(ui: &mut Ui, card: &ProjectCard) -> bool {
    let response = card_frame()
        .stroke(Stroke::new(1.0, LINE))
        .show(ui, |ui| {
            ui.set_min_height(168.0);
            ui.label(
                RichText::new(&card.name)
                    .font(TYPE.title_id())
                    .color(TEXT)
                    .strong(),
            );
            if let Some(status) = &card.status {
                draw_badge(ui, status, None);
            }
            if let Some(version) = &card.version {
                ui.label(
                    RichText::new(format!("Version {version}"))
                        .font(TYPE.secondary_id())
                        .color(MUTED),
                );
            }
            if !card.id.is_empty() {
                ui.label(
                    RichText::new(&card.id)
                        .font(TYPE.secondary_id())
                        .color(MUTED),
                )
                .on_hover_text(format!("Project id: {}", card.id));
            }

            if let Some(error) = &card.load_error {
                ui.add_space(6.0);
                ui.colored_label(SEVERITY_CRITICAL, format!("Error: {error}"));
                return;
            }

            ui.add_space(6.0);
            draw_progress(ui, card.progress);
            ui.add_space(4.0);

            if let Some(phase) = &card.current_phase {
                ui.label(
                    RichText::new(format!("Current phase: {phase}"))
                        .font(TYPE.body_id())
                        .color(TEXT),
                );
            }
            if let Some(due) = &card.next_due {
                let remaining = card
                    .next_due_remaining
                    .as_deref()
                    .map_or(String::new(), |text| format!(" ({text})"));
                ui.label(
                    RichText::new(format!("Next due: {due}{remaining}"))
                        .font(TYPE.body_id())
                        .color(TEXT),
                );
            }
            if let Some(sdk) = card.target_sdk {
                ui.label(
                    RichText::new(format!("Android targetSdk: {sdk}"))
                        .font(TYPE.body_id())
                        .color(TEXT),
                );
            }

            ui.add_space(4.0);
            ui.label(
                RichText::new(&card.open_issues_label)
                    .font(TYPE.body_id())
                    .color(MUTED),
            );
            if card.finding_badges.is_empty() {
                ui.label(
                    RichText::new(open_findings_label(0))
                        .font(TYPE.body_id())
                        .color(MUTED),
                );
            } else {
                ui.horizontal_wrapped(|ui| {
                    for (badge, count) in &card.finding_badges {
                        draw_badge(ui, badge, Some(*count));
                    }
                });
            }
        })
        .response
        .interact(Sense::click());

    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response.clicked() || (response.has_focus() && ui.input(|input| input.key_pressed(Key::Enter)))
}

fn draw_detail(
    ui: &mut Ui,
    detail: &ProjectDetail,
    loaded: &LoadedProject,
    edit: &mut DetailEdit,
) -> Option<DetailAction> {
    let mut action = None;
    ScrollArea::vertical().show(ui, |ui| {
        ui.label(
            RichText::new(&detail.name)
                .font(TYPE.title_id())
                .color(TEXT)
                .strong(),
        );
        ui.label(
            RichText::new(&detail.path)
                .font(TYPE.secondary_id())
                .color(MUTED),
        )
        .on_hover_text(&detail.path);

        if let Some(error) = &detail.load_error {
            ui.add_space(8.0);
            ui.colored_label(SEVERITY_CRITICAL, format!("Error: {error}"));
            return;
        }

        if edit.conflict {
            ui.add_space(8.0);
            ui.colored_label(
                SEVERITY_CRITICAL,
                edit.error
                    .as_deref()
                    .unwrap_or("project file changed on disk"),
            );
            if ui
                .add(Button::new(
                    RichText::new("Reload").font(TYPE.body_id()).color(ACCENT),
                ))
                .clicked()
            {
                action = Some(DetailAction::Reload);
            }
        }

        ui.add_space(8.0);
        let editable = loaded.fingerprint.is_some() && detail.load_error.is_none();
        let wide = ui.available_width() >= DETAIL_TWO_COLUMNS_FROM;
        if wide {
            // Each column Ui is top_down_justified at full half-width (see egui::Ui::columns).
            ui.columns(2, |cols| {
                draw_detail_left(&mut cols[0], detail);
                if let Some(next) = draw_detail_right(&mut cols[1], detail, edit, editable) {
                    action = Some(next);
                }
            });
        } else {
            draw_detail_left(ui, detail);
            ui.add_space(CARD_GAP);
            if let Some(next) = draw_detail_right(ui, detail, edit, editable) {
                action = Some(next);
            }
        }
    });
    action
}

fn draw_detail_left(ui: &mut Ui, detail: &ProjectDetail) {
    section(ui, "Overview", false, |ui| {
        if let Some(status) = &detail.status {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Status:").font(TYPE.body_id()).color(MUTED));
                draw_badge(ui, status, None);
            });
        }
        for field in &detail.overview_fields {
            if field.label == "Status" {
                continue;
            }
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("{}:", field.label))
                        .font(TYPE.body_id())
                        .color(MUTED),
                );
                let value = ui.add(
                    Label::new(RichText::new(&field.value).font(TYPE.body_id()).color(TEXT)).wrap(),
                );
                if field.label == "Id" {
                    value.on_hover_text("Useful for CLI commands");
                }
            });
        }
        if let Some(description) = &detail.description {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Description:")
                        .font(TYPE.body_id())
                        .color(MUTED),
                );
                ui.add(
                    Label::new(RichText::new(description).font(TYPE.body_id()).color(MUTED)).wrap(),
                );
            });
        }
        ui.add_space(6.0);
        draw_progress(ui, detail.progress);
    });

    ui.add_space(CARD_GAP);
    section(ui, "Stack", detail.components.is_empty(), |ui| {
        for component in &detail.components {
            ui.horizontal(|ui| {
                draw_badge(ui, &component.kind, None);
                let version = component
                    .version
                    .as_ref()
                    .map_or(String::new(), |value| format!(" {value}"));
                ui.add(
                    Label::new(
                        RichText::new(format!("{}{version}", component.name))
                            .font(TYPE.body_id())
                            .color(TEXT),
                    )
                    .wrap(),
                );
            });
        }
    });

    ui.add_space(CARD_GAP);
    section(ui, "Deadlines", detail.deadlines.is_empty(), |ui| {
        for row in &detail.deadlines {
            ui.horizontal(|ui| {
                draw_badge(ui, &row.kind, None);
                ui.add(
                    Label::new(
                        RichText::new(format!("{} — {} ({})", row.title, row.due, row.remaining))
                            .font(TYPE.body_id())
                            .color(TEXT),
                    )
                    .wrap(),
                );
            });
        }
    });

    ui.add_space(CARD_GAP);
    section(ui, "Open findings", detail.findings.is_empty(), |ui| {
        for finding in &detail.findings {
            draw_finding(ui, finding);
        }
    });
}

fn draw_detail_right(
    ui: &mut Ui,
    detail: &ProjectDetail,
    edit: &mut DetailEdit,
    editable: bool,
) -> Option<DetailAction> {
    let mut action = draw_phases_panel(ui, detail, edit, editable);
    ui.add_space(CARD_GAP);
    if let Some(next) = draw_issues_panel(ui, detail, edit, editable) {
        action = Some(next);
    }
    action
}

fn draw_phases_panel(
    ui: &mut Ui,
    detail: &ProjectDetail,
    edit: &mut DetailEdit,
    editable: bool,
) -> Option<DetailAction> {
    let mut action = None;
    card_frame().show(ui, |ui| {
        ui.label(
            RichText::new("Phases")
                .font(TYPE.section_id())
                .color(HEADING)
                .strong(),
        );
        if detail.phases.is_empty() {
            ui.label(RichText::new("None").font(TYPE.body_id()).color(MUTED));
            return;
        }
        let status_width = status_chip_column_width(ui, task_status_choices());
        for phase in &detail.phases {
            let header = format!("{}  {}", phase.status.display(), phase.title);
            CollapsingHeader::new(
                RichText::new(header)
                    .font(TYPE.body_id())
                    .color(TEXT)
                    .strong(),
            )
            .default_open(phase.initially_expanded)
            .id_salt(&phase.id)
            .show(ui, |ui| {
                ui.add(
                    Label::new(
                        RichText::new(&phase.id)
                            .font(TYPE.secondary_id())
                            .color(MUTED),
                    )
                    .wrap(),
                )
                .on_hover_text(format!("Phase id: {}", phase.id));
                if let Some(goal) = &phase.goal {
                    ui.add(
                        Label::new(RichText::new(goal).font(TYPE.body_id()).color(MUTED)).wrap(),
                    );
                }
                draw_progress(ui, phase.progress);
                for task in &phase.tasks {
                    if let Some(next) =
                        draw_task_row(ui, &phase.id, task, edit, editable, status_width)
                    {
                        action = Some(next);
                    }
                }
                if editable {
                    let form_open = edit
                        .task_form
                        .as_ref()
                        .is_some_and(|form| form.phase_id == phase.id);
                    if form_open {
                        if let Some(next) = draw_task_form(ui, edit) {
                            action = Some(next);
                        }
                    } else if ui
                        .add(Button::new(
                            RichText::new("Add task")
                                .font(TYPE.secondary_id())
                                .color(ACCENT),
                        ))
                        .clicked()
                    {
                        edit.clear_message();
                        edit.issue_form = None;
                        edit.task_form = Some(TaskForm::new(phase.id.clone()));
                    }
                }
            });
            ui.add_space(4.0);
        }
    });
    action
}

fn draw_task_row(
    ui: &mut Ui,
    phase_id: &str,
    task: &crate::view::TaskRow,
    edit: &DetailEdit,
    editable: bool,
    status_width: f32,
) -> Option<DetailAction> {
    let mut action = None;
    let title_colour = if task_title_is_muted(task.status.key) {
        MUTED
    } else {
        TEXT
    };
    ui.horizontal(|ui| {
        if editable {
            if let Some(status) = status_menu(
                ui,
                &format!("task-status-{phase_id}-{}", task.id),
                task.status.label,
                status_colour(&task.status),
                task_status_choices(),
                status_width,
            ) {
                action = Some(DetailAction::SetTaskStatus {
                    phase_id: phase_id.to_owned(),
                    task_id: task.id.clone(),
                    status: status.to_owned(),
                });
            }
        } else {
            draw_status_chip(
                ui,
                task.status.label,
                status_colour(&task.status),
                status_width,
            );
        }
        ui.add(
            Label::new(
                RichText::new(&task.title)
                    .font(TYPE.body_id())
                    .color(title_colour),
            )
            .wrap(),
        )
        .on_hover_text(format!("{} ({})", task.status.label, task.id));
        if let Some(due) = &task.due {
            ui.label(
                RichText::new(format!("due {due}"))
                    .font(TYPE.secondary_id())
                    .color(MUTED),
            );
        }
    });
    if matches!(
        &edit.error_site,
        Some(ErrorSite::Task {
            phase_id: err_phase,
            task_id
        }) if err_phase == phase_id && task_id == &task.id
    ) {
        draw_inline_error(ui, edit, &mut action);
    }
    action
}

fn draw_issues_panel(
    ui: &mut Ui,
    detail: &ProjectDetail,
    edit: &mut DetailEdit,
    editable: bool,
) -> Option<DetailAction> {
    let mut action = None;
    card_frame().show(ui, |ui| {
        ui.label(
            RichText::new("Open issues")
                .font(TYPE.section_id())
                .color(HEADING)
                .strong(),
        );
        if detail.issues.is_empty() && edit.issue_form.is_none() {
            ui.label(RichText::new("None").font(TYPE.body_id()).color(MUTED));
        }
        let status_width = status_chip_column_width(ui, issue_status_choices());
        for issue in &detail.issues {
            ui.horizontal(|ui| {
                draw_badge(ui, &issue.severity, None);
                if editable {
                    if let Some(status) = status_menu(
                        ui,
                        &format!("issue-status-{}", issue.id),
                        issue.status.label,
                        status_colour(&issue.status),
                        issue_status_choices(),
                        status_width,
                    ) {
                        action = Some(DetailAction::SetIssueStatus {
                            issue_id: issue.id.clone(),
                            status: status.to_owned(),
                        });
                    }
                } else {
                    draw_status_chip(
                        ui,
                        issue.status.label,
                        status_colour(&issue.status),
                        status_width,
                    );
                }
                ui.add(
                    Label::new(RichText::new(&issue.title).font(TYPE.body_id()).color(TEXT)).wrap(),
                )
                .on_hover_text(format!("Issue id: {}", issue.id));
            });
            if matches!(
                &edit.error_site,
                Some(ErrorSite::Issue { issue_id }) if issue_id == &issue.id
            ) {
                draw_inline_error(ui, edit, &mut action);
            }
        }
        if editable {
            if edit.issue_form.is_some() {
                if let Some(next) = draw_issue_form(ui, edit) {
                    action = Some(next);
                }
            } else if ui
                .add(Button::new(
                    RichText::new("Add issue")
                        .font(TYPE.secondary_id())
                        .color(ACCENT),
                ))
                .clicked()
            {
                edit.clear_message();
                edit.task_form = None;
                edit.issue_form = Some(IssueForm::default());
            }
        }
    });
    action
}

fn draw_task_form(ui: &mut Ui, edit: &mut DetailEdit) -> Option<DetailAction> {
    let mut action = None;
    let mut cancel = false;
    let can_save;
    let validation;
    {
        let form = edit.task_form.as_mut()?;
        ui.add_space(6.0);
        ui.label(
            RichText::new("New task")
                .font(TYPE.secondary_id())
                .color(HEADING)
                .strong(),
        );
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Title")
                    .font(TYPE.secondary_id())
                    .color(MUTED),
            );
            ui.add(
                TextEdit::singleline(&mut form.title)
                    .font(TYPE.body_id())
                    .desired_width(220.0),
            );
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new("Due").font(TYPE.secondary_id()).color(MUTED));
            ui.add(
                TextEdit::singleline(&mut form.due)
                    .font(TYPE.body_id())
                    .hint_text("YYYY-MM-DD")
                    .desired_width(120.0),
            );
        });
        can_save = form.can_save();
        validation = form.validation_message();
    }
    if can_save && ui.input(|input| input.key_pressed(Key::Enter)) {
        action = Some(DetailAction::SaveTask);
    }
    if let Some(message) = validation {
        ui.label(
            RichText::new(message)
                .font(TYPE.secondary_id())
                .color(MUTED),
        );
    }
    ui.horizontal(|ui| {
        let save = ui.add_enabled(
            can_save,
            Button::new(RichText::new("Save").font(TYPE.body_id()).color(ACCENT)),
        );
        if save.clicked() {
            action = Some(DetailAction::SaveTask);
        }
        if ui
            .add(Button::new(
                RichText::new("Cancel").font(TYPE.body_id()).color(MUTED),
            ))
            .clicked()
        {
            cancel = true;
        }
    });
    if cancel {
        edit.task_form = None;
        edit.clear_message();
        return None;
    }
    if matches!(&edit.error_site, Some(ErrorSite::TaskForm)) {
        draw_inline_error(ui, edit, &mut action);
    }
    action
}

fn draw_issue_form(ui: &mut Ui, edit: &mut DetailEdit) -> Option<DetailAction> {
    let mut action = None;
    let mut cancel = false;
    let can_save;
    let validation;
    {
        let form = edit.issue_form.as_mut()?;
        ui.add_space(6.0);
        ui.label(
            RichText::new("New issue")
                .font(TYPE.secondary_id())
                .color(HEADING)
                .strong(),
        );
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Title")
                    .font(TYPE.secondary_id())
                    .color(MUTED),
            );
            ui.add(
                TextEdit::singleline(&mut form.title)
                    .font(TYPE.body_id())
                    .desired_width(220.0),
            );
        });
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Severity")
                    .font(TYPE.secondary_id())
                    .color(MUTED),
            );
            let selected = issue_severity_choices()
                .iter()
                .find(|(key, _)| *key == form.severity)
                .map_or(form.severity.as_str(), |(_, label)| *label)
                .to_owned();
            ComboBox::from_id_salt("new-issue-severity")
                .selected_text(RichText::new(selected).font(TYPE.body_id()).color(TEXT))
                .show_ui(ui, |ui| {
                    for (key, label) in issue_severity_choices() {
                        ui.selectable_value(&mut form.severity, (*key).to_owned(), *label);
                    }
                });
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new("Note").font(TYPE.secondary_id()).color(MUTED));
            ui.add(
                TextEdit::singleline(&mut form.note)
                    .font(TYPE.body_id())
                    .desired_width(220.0),
            );
        });
        can_save = form.can_save();
        validation = form.validation_message();
    }
    if can_save && ui.input(|input| input.key_pressed(Key::Enter)) {
        action = Some(DetailAction::SaveIssue);
    }
    if let Some(message) = validation {
        ui.label(
            RichText::new(message)
                .font(TYPE.secondary_id())
                .color(MUTED),
        );
    }
    ui.horizontal(|ui| {
        let save = ui.add_enabled(
            can_save,
            Button::new(RichText::new("Save").font(TYPE.body_id()).color(ACCENT)),
        );
        if save.clicked() {
            action = Some(DetailAction::SaveIssue);
        }
        if ui
            .add(Button::new(
                RichText::new("Cancel").font(TYPE.body_id()).color(MUTED),
            ))
            .clicked()
        {
            cancel = true;
        }
    });
    if cancel {
        edit.issue_form = None;
        edit.clear_message();
        return None;
    }
    if matches!(&edit.error_site, Some(ErrorSite::IssueForm)) {
        draw_inline_error(ui, edit, &mut action);
    }
    action
}

fn draw_inline_error(ui: &mut Ui, edit: &DetailEdit, action: &mut Option<DetailAction>) {
    if let Some(message) = &edit.error {
        ui.colored_label(SEVERITY_CRITICAL, message);
    }
    if edit.conflict
        && ui
            .add(Button::new(
                RichText::new("Reload")
                    .font(TYPE.secondary_id())
                    .color(ACCENT),
            ))
            .clicked()
    {
        *action = Some(DetailAction::Reload);
    }
}

/// Horizontal chip padding (frame margins) plus room for the menu caret.
const STATUS_CHIP_PAD: f32 = 28.0;

fn status_chip_column_width(ui: &Ui, choices: &[(&'static str, &'static str)]) -> f32 {
    let label = longest_status_choice_label(choices);
    let font = TYPE.secondary_id();
    let text_w = ui.ctx().fonts_mut(|fonts| {
        fonts
            .layout_no_wrap(label.to_owned(), font, Color32::WHITE)
            .size()
            .x
    });
    text_w + STATUS_CHIP_PAD
}

fn draw_status_chip(ui: &mut Ui, label: &str, colour: Color32, fixed_width: f32) {
    ui.allocate_ui_with_layout(
        Vec2::new(fixed_width, 0.0),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.set_min_width(fixed_width);
            ui.set_max_width(fixed_width);
            Frame::NONE
                .fill(CARD_INNER)
                .stroke(Stroke::new(1.0, LINE))
                .corner_radius(CornerRadius::same(6))
                .inner_margin(Margin::symmetric(8, 3))
                .show(ui, |ui| {
                    ui.set_min_width((fixed_width - 16.0).max(1.0));
                    ui.label(
                        RichText::new(label)
                            .font(TYPE.secondary_id())
                            .color(colour)
                            .strong(),
                    );
                });
        },
    );
}

fn status_menu<'a>(
    ui: &mut Ui,
    id_salt: &str,
    label: &str,
    colour: Color32,
    choices: &'a [(&'static str, &'static str)],
    fixed_width: f32,
) -> Option<&'a str> {
    let mut chosen = None;
    ui.push_id(id_salt, |ui| {
        ui.allocate_ui_with_layout(
            Vec2::new(fixed_width, 0.0),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.set_min_width(fixed_width);
                ui.set_max_width(fixed_width);
                Frame::NONE
                    .fill(CARD_INNER)
                    .stroke(Stroke::new(1.0, LINE))
                    .corner_radius(CornerRadius::same(6))
                    .inner_margin(Margin::symmetric(8, 3))
                    .show(ui, |ui| {
                        ui.set_min_width((fixed_width - 16.0).max(1.0));
                        ui.menu_button(
                            RichText::new(label)
                                .font(TYPE.secondary_id())
                                .color(colour)
                                .strong(),
                            |ui| {
                                for (key, choice_label) in choices {
                                    if ui
                                        .button(
                                            RichText::new(*choice_label)
                                                .font(TYPE.body_id())
                                                .color(TEXT),
                                        )
                                        .clicked()
                                    {
                                        chosen = Some(*key);
                                        ui.close();
                                    }
                                }
                            },
                        )
                        .response
                        .on_hover_text(format!("{label} — change status"));
                    });
            },
        );
    });
    chosen
}

fn draw_finding(ui: &mut Ui, finding: &FindingRow) {
    ui.horizontal(|ui| {
        draw_badge(ui, &finding.severity, None);
        draw_badge(ui, &finding.kind, None);
    });
    ui.add(
        Label::new(
            RichText::new(&finding.source)
                .font(TYPE.body_id())
                .color(TEXT),
        )
        .wrap(),
    );
    if !finding.summary.is_empty() {
        ui.add(
            Label::new(
                RichText::new(&finding.summary)
                    .font(TYPE.secondary_id())
                    .color(MUTED),
            )
            .wrap(),
        );
    }
    ui.add_space(6.0);
}

fn draw_progress(ui: &mut Ui, progress: ProgressView) {
    ui.label(
        RichText::new(progress.caption())
            .font(TYPE.secondary_id())
            .color(MUTED),
    );
    let fraction = if progress.total == 0 || progress.done == 0 {
        0.0
    } else {
        f32::from(u16::try_from(progress.done).unwrap_or(u16::MAX))
            / f32::from(u16::try_from(progress.total).unwrap_or(u16::MAX))
    };
    if fraction <= f32::EPSILON {
        let (_, rect) = ui.allocate_space(Vec2::new(ui.available_width(), 8.0));
        ui.painter()
            .rect_filled(rect, CornerRadius::same(4), PROGRESS_TRACK);
    } else {
        ui.add(
            ProgressBar::new(fraction)
                .fill(DONE)
                .desired_height(8.0)
                .animate(false),
        );
    }
}

fn draw_badge(ui: &mut Ui, badge: &StatusBadge, count: Option<usize>) {
    debug_assert!(!badge_stretches_to_parent());
    let text = match count {
        Some(value) if badge.symbol.is_empty() => format!("{} · {value}", badge.label),
        Some(value) => format!("{} {} · {value}", badge.symbol, badge.label),
        None => badge.display(),
    };
    let colour = status_colour(badge);
    // Horizontal layout shrinks the frame to the label instead of the card width.
    ui.horizontal(|ui| {
        Frame::NONE
            .fill(CARD_INNER)
            .stroke(Stroke::new(1.0, LINE))
            .corner_radius(CornerRadius::same(6))
            .inner_margin(Margin::symmetric(8, 3))
            .show(ui, |ui| {
                ui.set_max_width(ui.spacing().interact_size.x.max(1.0) * 20.0);
                ui.label(
                    RichText::new(text)
                        .font(TYPE.secondary_id())
                        .color(colour)
                        .strong(),
                );
            });
    });
}

fn status_colour(badge: &StatusBadge) -> Color32 {
    match badge.key {
        "done" | "resolved" | "released" => DONE,
        "in_progress" | "in_development" | "eol_soon" | "platform_deadline_soon" => RUNNING,
        "medium" => SEVERITY_MEDIUM,
        "critical" | "malicious" | "eol" | "platform_deadline" => SEVERITY_CRITICAL,
        "high" => SEVERITY_HIGH,
        "low" => SEVERITY_LOW,
        "open" | "idea" | "unknown" | "notice" => MUTED,
        _ => TEXT,
    }
}

fn section(ui: &mut Ui, title: &str, empty: bool, add_contents: impl FnOnce(&mut Ui)) {
    card_frame().show(ui, |ui| {
        ui.label(
            RichText::new(title)
                .font(TYPE.section_id())
                .color(HEADING)
                .strong(),
        );
        if empty {
            ui.label(RichText::new("None").font(TYPE.body_id()).color(MUTED));
        } else {
            add_contents(ui);
        }
    });
}

fn card_frame() -> Frame {
    Frame::NONE
        .fill(CARD)
        .stroke(Stroke::new(1.0, LINE))
        .corner_radius(CornerRadius::same(CARD_ROUNDING))
        .inner_margin(Margin::same(CARD_PADDING))
}

/// Minimum window size used at startup.
#[must_use]
pub fn min_window_size() -> Vec2 {
    Vec2::new(720.0, 480.0)
}

/// Default window size used at startup (matches the previous AP9 window).
#[must_use]
pub fn default_window_size() -> Vec2 {
    Vec2::new(980.0, 680.0)
}
