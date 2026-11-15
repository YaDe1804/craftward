//! egui surface that renders the view model.

use eframe::egui::{
    self, Align, Button, CentralPanel, CollapsingHeader, Color32, Context, CornerRadius, Frame,
    Key, Layout, Margin, Panel, ProgressBar, RichText, ScrollArea, Sense, Stroke, Theme,
    ThemePreference, Ui, Vec2,
};

use crate::load::{LoadedPortfolio, load_portfolio};
use crate::theme::{
    self, ACCENT, BACKGROUND, CARD, CARD_GAP, CARD_INNER, CARD_PADDING, CARD_ROUNDING,
    DETAIL_TWO_COLUMNS_FROM, DONE, HEADING, LINE, MUTED, PAGE_MARGIN, PROGRESS_TRACK, RUNNING,
    SEVERITY_CRITICAL, SEVERITY_HIGH, SEVERITY_LOW, SEVERITY_MEDIUM, TEXT, TYPE,
};
use crate::view::{
    FindingRow, PortfolioView, ProgressView, ProjectCard, ProjectDetail, StatusBadge, build_detail,
    build_portfolio, finding_count_badges, grid_columns, open_findings_label,
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

/// The Craftward desktop application.
pub struct CraftwardApp {
    /// Last loaded data.
    loaded: LoadedPortfolio,
    /// Derived overview.
    portfolio: PortfolioView,
    /// Current screen.
    screen: Screen,
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
        }
    }

    fn reload(&mut self) {
        self.loaded = load_portfolio();
        self.portfolio = build_portfolio(&self.loaded);
        if let Screen::Detail { index } = self.screen
            && index >= self.loaded.projects.len()
        {
            self.screen = Screen::Overview;
        }
    }
}

impl eframe::App for CraftwardApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if ctx.input(|input| input.key_pressed(Key::Escape))
            && matches!(self.screen, Screen::Detail { .. })
        {
            self.screen = Screen::Overview;
        }

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
                        }
                    });
                });
            });

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

        CentralPanel::default()
            .frame(
                Frame::NONE
                    .fill(BACKGROUND)
                    .inner_margin(Margin::same(PAGE_MARGIN)),
            )
            .show(ui, |ui| match self.screen {
                Screen::Overview => {
                    if let Some(index) = draw_overview(ui, &self.portfolio) {
                        self.screen = Screen::Detail { index };
                    }
                }
                Screen::Detail { index } => {
                    if let Some(project) = self.loaded.projects.get(index) {
                        let detail = build_detail(index, project);
                        draw_detail(ui, &detail);
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
                        }
                    }
                }
            });
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
                if let Some(index) = draw_card_row(ui, &row) {
                    open = Some(index);
                }
                row.clear();
            }
        }
        if !row.is_empty()
            && let Some(index) = draw_card_row(ui, &row)
        {
            open = Some(index);
        }
    });
    open
}

fn draw_card_row(ui: &mut Ui, cards: &[&ProjectCard]) -> Option<usize> {
    let mut open = None;
    let columns = cards.len().max(1);
    ui.columns(columns, |cols| {
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

fn draw_detail(ui: &mut Ui, detail: &ProjectDetail) {
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

        ui.add_space(8.0);
        let wide = ui.available_width() >= DETAIL_TWO_COLUMNS_FROM;
        if wide {
            ui.columns(2, |cols| {
                draw_detail_left(&mut cols[0], detail);
                draw_detail_right(&mut cols[1], detail);
            });
        } else {
            draw_detail_left(ui, detail);
            ui.add_space(CARD_GAP);
            draw_detail_right(ui, detail);
        }
    });
}

fn draw_detail_left(ui: &mut Ui, detail: &ProjectDetail) {
    section(ui, "Overview", false, |ui| {
        if let Some(status) = &detail.status {
            draw_badge(ui, status, None);
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
                let value = ui.label(RichText::new(&field.value).font(TYPE.body_id()).color(TEXT));
                if field.label == "Id" {
                    value.on_hover_text("Useful for CLI commands");
                }
            });
        }
        if let Some(description) = &detail.description {
            ui.add_space(4.0);
            ui.label(RichText::new(description).font(TYPE.body_id()).color(MUTED));
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
                ui.label(
                    RichText::new(format!("{}{version}", component.name))
                        .font(TYPE.body_id())
                        .color(TEXT),
                );
            });
        }
    });

    ui.add_space(CARD_GAP);
    section(ui, "Deadlines", detail.deadlines.is_empty(), |ui| {
        for row in &detail.deadlines {
            ui.horizontal(|ui| {
                draw_badge(ui, &row.kind, None);
                ui.label(
                    RichText::new(format!("{} — {} ({})", row.title, row.due, row.remaining))
                        .font(TYPE.body_id())
                        .color(TEXT),
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

fn draw_detail_right(ui: &mut Ui, detail: &ProjectDetail) {
    section(ui, "Phases", detail.phases.is_empty(), |ui| {
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
                ui.label(
                    RichText::new(&phase.id)
                        .font(TYPE.secondary_id())
                        .color(MUTED),
                )
                .on_hover_text(format!("Phase id: {}", phase.id));
                if let Some(goal) = &phase.goal {
                    ui.label(RichText::new(goal).font(TYPE.body_id()).color(MUTED));
                }
                draw_progress(ui, phase.progress);
                for task in &phase.tasks {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(task.status.symbol)
                                .font(TYPE.body_id())
                                .color(status_colour(&task.status)),
                        )
                        .on_hover_text(task.status.label);
                        ui.label(RichText::new(&task.title).font(TYPE.body_id()).color(TEXT))
                            .on_hover_text(format!("{} ({})", task.status.label, task.id));
                        if let Some(due) = &task.due {
                            ui.label(
                                RichText::new(format!("due {due}"))
                                    .font(TYPE.secondary_id())
                                    .color(MUTED),
                            );
                        }
                    });
                }
            });
            ui.add_space(4.0);
        }
    });

    ui.add_space(CARD_GAP);
    section(ui, "Open issues", detail.issues.is_empty(), |ui| {
        for issue in &detail.issues {
            ui.horizontal(|ui| {
                draw_badge(ui, &issue.severity, None);
                draw_badge(ui, &issue.status, None);
                ui.label(RichText::new(&issue.title).font(TYPE.body_id()).color(TEXT))
                    .on_hover_text(format!("Issue id: {}", issue.id));
            });
        }
    });
}

fn draw_finding(ui: &mut Ui, finding: &FindingRow) {
    ui.horizontal(|ui| {
        draw_badge(ui, &finding.severity, None);
        draw_badge(ui, &finding.kind, None);
    });
    ui.label(
        RichText::new(&finding.source)
            .font(TYPE.body_id())
            .color(TEXT),
    );
    if !finding.summary.is_empty() {
        ui.label(
            RichText::new(&finding.summary)
                .font(TYPE.secondary_id())
                .color(MUTED),
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
    let text = match count {
        Some(value) => format!("{} {} · {value}", badge.symbol, badge.label),
        None => badge.display(),
    };
    let colour = status_colour(badge);
    Frame::NONE
        .fill(CARD_INNER)
        .stroke(Stroke::new(1.0, LINE))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::symmetric(8, 3))
        .show(ui, |ui| {
            ui.label(
                RichText::new(text)
                    .font(TYPE.secondary_id())
                    .color(colour)
                    .strong(),
            );
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
