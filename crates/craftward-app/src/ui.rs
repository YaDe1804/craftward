//! egui surface that renders the view model.

use eframe::egui::{
    self, Align, Button, CentralPanel, CollapsingHeader, Color32, ComboBox, Context, CornerRadius,
    Frame, Key, Label, Layout, Margin, Panel, ProgressBar, RichText, ScrollArea, Sense, Stroke,
    TextEdit, Theme, ThemePreference, Ui, UiBuilder, Vec2,
};

use craftward_core::UndoState;

use crate::actions::{
    SaveResult, now_secs, save_add_lockfile, save_add_project, save_android, save_component_edit,
    save_header, save_issue_edit, save_issue_status, save_new_component, save_new_issue,
    save_new_phase, save_new_project, save_new_task, save_new_watch, save_phase_edit,
    save_remove_component, save_remove_from_workspace, save_remove_issue, save_remove_lockfile,
    save_remove_phase, save_remove_task, save_remove_watch, save_task_edit, save_task_status,
    save_watch_edit,
};
use crate::date::{date_hint, format_date_for_display};
use crate::load::{LoadedPortfolio, LoadedProject, load_portfolio};
use crate::onboarding::{self, OnboardingAction, OnboardingState};
use crate::picker::{PathPicker, PickKind, PickerInbox, RfdPicker};
use crate::settings_panel::{self, SettingsEdit};
use crate::storage::{self, StorageSnapshot};
use crate::theme::{
    self, ACCENT, BACKGROUND, CARD, CARD_GAP, CARD_INNER, CARD_PADDING, CARD_ROUNDING,
    DETAIL_TWO_COLUMNS_FROM, DONE, HEADING, LINE, MUTED, OPEN_TEXT, PAGE_MARGIN, PROGRESS_TRACK,
    RUNNING, SEVERITY_CRITICAL, SEVERITY_HIGH, SEVERITY_LOW, SEVERITY_MEDIUM, SIDEBAR_WIDTH, TEXT,
    TYPE,
};
use crate::view::{
    AddProjectForm, AndroidForm, ComponentEditForm, ComponentForm, FindingRow, HeaderForm,
    IssueEditForm, IssueForm, IssuesFilter, LockfileForm, NewProjectForm, PhaseForm, PortfolioView,
    ProgressView, ProjectCard, ProjectDetail, StatusBadge, TaskEditForm, TaskForm, WatchEditForm,
    WatchForm, android_store_choices, badge_stretches_to_parent, build_detail, build_portfolio,
    collect_portfolio_issues, component_kind_choices, display_label, finding_count_badges,
    form_factor_choices, grid_columns, grid_row_column_slots, issue_severity_choices,
    issue_status_choices, longest_status_choice_label, open_findings_label, project_source_dir,
    project_status_choices, project_status_hint, relative_to_dir, task_status_choices,
    task_title_is_muted, watch_kind_choices,
};

/// Which screen is visible.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    /// Portfolio overview.
    Overview,
    /// Detail for `LoadedPortfolio::projects[index]` (still under Portfolio).
    Detail {
        /// Project index.
        index: usize,
    },
    /// Cross-project issues list.
    Issues,
    /// Storage settings.
    Settings,
}

/// Primary sidebar destinations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NavItem {
    /// Portfolio overview and project detail.
    Portfolio,
    /// Cross-project issues.
    Issues,
    /// Storage settings.
    Settings,
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
    /// Overview header form.
    HeaderForm,
    /// New or edit phase form.
    PhaseForm,
    /// Task field edit form.
    TaskEdit {
        /// Phase id.
        phase_id: String,
        /// Task id.
        task_id: String,
    },
    /// Issue field edit form.
    IssueEdit {
        /// Issue id.
        issue_id: String,
    },
    /// New or edit component form.
    ComponentForm,
    /// Lockfile add form.
    LockfileForm,
    /// Android edit form.
    AndroidForm,
    /// New watch form.
    WatchForm,
    /// Watch edit form.
    WatchEdit {
        /// Watch id.
        watch_id: String,
    },
}

/// Detail-view editing widgets (no egui types).
#[derive(Debug, Default)]
struct DetailEdit {
    /// Open task form, when adding to a phase.
    task_form: Option<TaskForm>,
    /// Open issue form.
    issue_form: Option<IssueForm>,
    /// Overview header form.
    header_form: Option<HeaderForm>,
    /// New phase form.
    phase_form: Option<PhaseForm>,
    /// Edit phase form (`phase_id`, draft).
    phase_edit: Option<(String, PhaseForm)>,
    /// Edit task form (`phase_id`, `task_id`, draft).
    task_edit: Option<(String, String, TaskEditForm)>,
    /// Edit issue form (`issue_id`, draft).
    issue_edit: Option<(String, IssueEditForm)>,
    /// New component form.
    component_form: Option<ComponentForm>,
    /// Edit component form (`name`, draft).
    component_edit: Option<(String, ComponentEditForm)>,
    /// Add lockfile form.
    lockfile_form: Option<LockfileForm>,
    /// Android edit form.
    android_form: Option<AndroidForm>,
    /// New watch form.
    watch_form: Option<WatchForm>,
    /// Edit watch form (`watch_id`, draft).
    watch_edit: Option<(String, WatchEditForm)>,
    /// Sticky conflict / error banner (survives scrolling).
    banner: Option<String>,
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

    fn close_forms(&mut self) {
        self.task_form = None;
        self.issue_form = None;
        self.header_form = None;
        self.phase_form = None;
        self.phase_edit = None;
        self.task_edit = None;
        self.issue_edit = None;
        self.component_form = None;
        self.component_edit = None;
        self.lockfile_form = None;
        self.android_form = None;
        self.watch_form = None;
        self.watch_edit = None;
        self.clear_message();
    }

    fn clear_all(&mut self) {
        self.close_forms();
        self.banner = None;
    }

    fn set_error(&mut self, site: ErrorSite, message: String, conflict: bool) {
        self.error_site = Some(site);
        self.error = Some(message.clone());
        self.conflict = conflict;
        if conflict {
            self.banner = Some(message);
        }
    }
}

/// Portfolio-level forms and sticky errors.
#[derive(Debug, Default)]
struct OverviewEdit {
    /// Create-project form.
    new_project: Option<NewProjectForm>,
    /// Add-existing-project form.
    add_project: Option<AddProjectForm>,
    /// Sticky banner on the overview.
    banner: Option<String>,
    /// Confirm deleting craftward.json after removing from the list.
    delete_file_prompt: Option<DeleteFilePrompt>,
}

/// Confirm dialog for deleting craftward.json (irreversible).
#[derive(Debug, Clone, PartialEq, Eq)]
struct DeleteFilePrompt {
    /// Project id.
    id: String,
    /// Display name.
    name: String,
    /// Absolute path of craftward.json.
    path: String,
    /// Return to overview after success (true from the detail page).
    return_overview: bool,
}

/// Action produced by the detail view widgets.
enum DetailAction {
    /// Set a task status.
    SetTaskStatus {
        phase_id: String,
        task_id: String,
        status: String,
        title: String,
    },
    /// Set an issue status.
    SetIssueStatus {
        /// Project index when set from the Issues list; detail screen uses [`None`].
        project_index: Option<usize>,
        issue_id: String,
        status: String,
        title: String,
    },
    /// Open the header source-folder picker.
    BrowseHeaderSource,
    /// Persist the open task form.
    SaveTask,
    /// Persist the open issue form.
    SaveIssue,
    /// Persist the header form.
    SaveHeader,
    /// Persist a new phase.
    SavePhase,
    /// Persist a phase edit.
    SavePhaseEdit,
    /// Remove a phase (and its tasks; undo restores them).
    RemovePhase {
        phase_id: String,
        title: String,
        task_count: usize,
    },
    /// Persist a task edit.
    SaveTaskEdit,
    /// Remove a task.
    RemoveTask {
        phase_id: String,
        task_id: String,
        title: String,
    },
    /// Persist an issue edit.
    SaveIssueEdit,
    /// Remove an issue.
    RemoveIssue { issue_id: String, title: String },
    /// Persist a new component.
    SaveComponent,
    /// Persist a component edit.
    SaveComponentEdit,
    /// Remove a component.
    RemoveComponent { name: String },
    /// Persist a new lockfile path.
    SaveLockfile,
    /// Open the native lockfile picker.
    BrowseLockfile,
    /// Remove a lockfile path.
    RemoveLockfile { path: String },
    /// Persist Android settings.
    SaveAndroid,
    /// Persist a new watch item.
    SaveWatch,
    /// Persist a watch edit.
    SaveWatchEdit,
    /// Remove a watch item.
    RemoveWatch { watch_id: String, title: String },
    /// Reload after a conflict.
    Reload,
    /// Dismiss the sticky banner.
    DismissBanner,
    /// Remove from the workspace list (undoable; file kept).
    RemoveFromList { id: String, name: String },
    /// Confirm deleting craftward.json (irreversible).
    PromptDeleteFile {
        id: String,
        name: String,
        path: String,
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
    /// Detail editing widgets.
    edit: DetailEdit,
    /// Overview editing widgets.
    overview: OverviewEdit,
    /// Filters for the Issues view.
    issues_filter: IssuesFilter,
    /// Issue id to highlight after navigating from Issues.
    focus_issue_id: Option<String>,
    /// Last undoable change.
    undo: UndoState,
    /// Native path picker.
    picker: Box<dyn PathPicker>,
    /// Inbox for background picker results.
    picker_inbox: PickerInbox,
    /// First-run wizard when present.
    onboarding: Option<OnboardingState>,
    /// Tip after finishing first-run setup.
    post_setup_tip: bool,
    /// Resolved storage snapshot for Settings.
    storage: StorageSnapshot,
    /// Settings screen widgets.
    settings_edit: SettingsEdit,
}

impl CraftwardApp {
    /// Loads data and applies the theme.
    #[must_use]
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        apply_theme(&cc.egui_ctx);
        let onboarding = if storage::needs_onboarding() {
            match OnboardingState::start() {
                Ok(state) => Some(state),
                Err(error) => Some(OnboardingState {
                    step: onboarding::OnboardingStep::Finish,
                    draft: crate::storage::OnboardingDraft {
                        workspace: String::new(),
                        data_dir: String::new(),
                        cache_dir: String::new(),
                    },
                    workspace_env: false,
                    data_env: false,
                    cache_env: false,
                    error: Some(error),
                }),
            }
        } else {
            None
        };
        let loaded = load_portfolio();
        let portfolio = build_portfolio(&loaded);
        Self {
            loaded,
            portfolio,
            screen: Screen::Overview,
            edit: DetailEdit::default(),
            overview: OverviewEdit::default(),
            issues_filter: IssuesFilter::default(),
            focus_issue_id: None,
            undo: UndoState::new(),
            picker: Box::new(RfdPicker),
            picker_inbox: PickerInbox::default(),
            onboarding,
            post_setup_tip: false,
            storage: StorageSnapshot::load(),
            settings_edit: SettingsEdit::default(),
        }
    }

    fn reload(&mut self) {
        self.loaded = load_portfolio();
        self.portfolio = build_portfolio(&self.loaded);
        self.storage = StorageSnapshot::load();
        self.edit.clear_all();
        self.overview.banner = None;
        if let Screen::Detail { index } = self.screen
            && index >= self.loaded.projects.len()
        {
            self.screen = Screen::Overview;
            self.focus_issue_id = None;
        }
    }

    fn nav_item(&self) -> NavItem {
        match self.screen {
            Screen::Overview | Screen::Detail { .. } => NavItem::Portfolio,
            Screen::Issues => NavItem::Issues,
            Screen::Settings => NavItem::Settings,
        }
    }

    fn go_portfolio_overview(&mut self) {
        self.screen = Screen::Overview;
        self.edit.clear_all();
        self.focus_issue_id = None;
    }

    fn open_project_issue(&mut self, project_index: usize, issue_id: &str) {
        self.edit.clear_all();
        self.focus_issue_id = Some(issue_id.to_owned());
        self.screen = Screen::Detail {
            index: project_index,
        };
    }

    #[allow(clippy::too_many_lines)]
    fn apply_detail_action(&mut self, index: usize, action: DetailAction) {
        match action {
            DetailAction::Reload => self.reload(),
            DetailAction::DismissBanner => {
                self.edit.banner = None;
                self.edit.clear_message();
            }
            DetailAction::BrowseHeaderSource | DetailAction::BrowseLockfile => {}
            DetailAction::SetTaskStatus {
                phase_id,
                task_id,
                status,
                title,
            } => {
                let site = ErrorSite::Task {
                    phase_id: phase_id.clone(),
                    task_id: task_id.clone(),
                };
                self.run_save(index, site, |path, fingerprint| {
                    save_task_status(path, fingerprint, &phase_id, &task_id, &status, &title)
                });
            }
            DetailAction::SetIssueStatus {
                project_index,
                issue_id,
                status,
                title,
            } => {
                let target = project_index.unwrap_or(index);
                let site = ErrorSite::Issue {
                    issue_id: issue_id.clone(),
                };
                self.run_save(target, site, |path, fingerprint| {
                    save_issue_status(path, fingerprint, &issue_id, &status, &title)
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
            DetailAction::SaveHeader => {
                let Some(form) = self.edit.header_form.clone() else {
                    return;
                };
                self.run_save(index, ErrorSite::HeaderForm, |path, fingerprint| {
                    save_header(path, fingerprint, &form)
                });
            }
            DetailAction::SavePhase => {
                let Some(form) = self.edit.phase_form.clone() else {
                    return;
                };
                self.run_save(index, ErrorSite::PhaseForm, |path, fingerprint| {
                    save_new_phase(path, fingerprint, &form)
                });
            }
            DetailAction::SavePhaseEdit => {
                let Some((phase_id, form)) = self.edit.phase_edit.clone() else {
                    return;
                };
                self.run_save(index, ErrorSite::PhaseForm, |path, fingerprint| {
                    save_phase_edit(path, fingerprint, &phase_id, &form)
                });
            }
            DetailAction::RemovePhase {
                phase_id,
                title,
                task_count,
            } => {
                self.run_save(index, ErrorSite::PhaseForm, |path, fingerprint| {
                    save_remove_phase(path, fingerprint, &phase_id, &title, task_count)
                });
            }
            DetailAction::SaveTaskEdit => {
                let Some((phase_id, task_id, form)) = self.edit.task_edit.clone() else {
                    return;
                };
                let site = ErrorSite::TaskEdit {
                    phase_id: phase_id.clone(),
                    task_id: task_id.clone(),
                };
                self.run_save(index, site, |path, fingerprint| {
                    save_task_edit(path, fingerprint, &phase_id, &task_id, &form)
                });
            }
            DetailAction::RemoveTask {
                phase_id,
                task_id,
                title,
            } => {
                let site = ErrorSite::Task {
                    phase_id: phase_id.clone(),
                    task_id: task_id.clone(),
                };
                self.run_save(index, site, |path, fingerprint| {
                    save_remove_task(path, fingerprint, &phase_id, &task_id, &title)
                });
            }
            DetailAction::SaveIssueEdit => {
                let Some((issue_id, form)) = self.edit.issue_edit.clone() else {
                    return;
                };
                let site = ErrorSite::IssueEdit {
                    issue_id: issue_id.clone(),
                };
                self.run_save(index, site, |path, fingerprint| {
                    save_issue_edit(path, fingerprint, &issue_id, &form)
                });
            }
            DetailAction::RemoveIssue { issue_id, title } => {
                let site = ErrorSite::Issue {
                    issue_id: issue_id.clone(),
                };
                self.run_save(index, site, |path, fingerprint| {
                    save_remove_issue(path, fingerprint, &issue_id, &title)
                });
            }
            DetailAction::SaveComponent => {
                let Some(form) = self.edit.component_form.clone() else {
                    return;
                };
                self.run_save(index, ErrorSite::ComponentForm, |path, fingerprint| {
                    save_new_component(path, fingerprint, &form)
                });
            }
            DetailAction::SaveComponentEdit => {
                let Some((name, form)) = self.edit.component_edit.clone() else {
                    return;
                };
                self.run_save(index, ErrorSite::ComponentForm, |path, fingerprint| {
                    save_component_edit(path, fingerprint, &name, &form)
                });
            }
            DetailAction::RemoveComponent { name } => {
                self.run_save(index, ErrorSite::ComponentForm, |path, fingerprint| {
                    save_remove_component(path, fingerprint, &name)
                });
            }
            DetailAction::SaveLockfile => {
                let Some(form) = self.edit.lockfile_form.clone() else {
                    return;
                };
                self.run_save(index, ErrorSite::LockfileForm, |path, fingerprint| {
                    save_add_lockfile(path, fingerprint, &form)
                });
            }
            DetailAction::RemoveLockfile { path } => {
                self.run_save(index, ErrorSite::LockfileForm, |file, fingerprint| {
                    save_remove_lockfile(file, fingerprint, &path)
                });
            }
            DetailAction::SaveAndroid => {
                let Some(form) = self.edit.android_form.clone() else {
                    return;
                };
                self.run_save(index, ErrorSite::AndroidForm, |path, fingerprint| {
                    save_android(path, fingerprint, &form)
                });
            }
            DetailAction::SaveWatch => {
                let Some(form) = self.edit.watch_form.clone() else {
                    return;
                };
                self.run_save(index, ErrorSite::WatchForm, |path, fingerprint| {
                    save_new_watch(path, fingerprint, &form)
                });
            }
            DetailAction::SaveWatchEdit => {
                let Some((watch_id, form)) = self.edit.watch_edit.clone() else {
                    return;
                };
                let site = ErrorSite::WatchEdit {
                    watch_id: watch_id.clone(),
                };
                self.run_save(index, site, |path, fingerprint| {
                    save_watch_edit(path, fingerprint, &watch_id, &form)
                });
            }
            DetailAction::RemoveWatch { watch_id, title } => {
                let site = ErrorSite::WatchEdit {
                    watch_id: watch_id.clone(),
                };
                self.run_save(index, site, |path, fingerprint| {
                    save_remove_watch(path, fingerprint, &watch_id, &title)
                });
            }
            DetailAction::RemoveFromList { id, name } => {
                self.remove_from_list(&id, &name, false, true);
            }
            DetailAction::PromptDeleteFile { id, name, path } => {
                self.overview.delete_file_prompt = Some(DeleteFilePrompt {
                    id,
                    name,
                    path,
                    return_overview: true,
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
            result @ SaveResult::Saved { .. } => {
                if let Some(offer) = result.undo_offer(now_secs()) {
                    self.undo.push(offer);
                }
                let stay = self.screen;
                self.reload();
                self.screen = stay;
            }
            SaveResult::Failed { message, conflict } => {
                self.edit.set_error(site, message, conflict);
            }
        }
    }

    fn run_workspace_save(
        &mut self,
        save: impl FnOnce(&std::path::Path, &craftward_core::ContentFingerprint) -> SaveResult,
    ) {
        let Some(path) = self.loaded.workspace_path.clone() else {
            self.overview.banner = Some("workspace path is not available".to_owned());
            return;
        };
        let Some(fingerprint) = self.loaded.workspace_fingerprint.clone() else {
            self.overview.banner = Some("workspace file is not available for editing".to_owned());
            return;
        };
        match save(&path, &fingerprint) {
            result @ SaveResult::Saved { .. } => {
                if let Some(offer) = result.undo_offer(now_secs()) {
                    self.undo.push(offer);
                }
                self.reload();
            }
            SaveResult::Failed { message, conflict } => {
                self.overview.banner = Some(message);
                if conflict {
                    // keep banner until reload
                }
            }
        }
    }
}

impl eframe::App for CraftwardApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.undo.tick(now_secs());
        self.drain_picker();
        if self.onboarding.is_some() {
            self.draw_onboarding_frame(ui);
            return;
        }
        self.handle_escape(ui);
        self.draw_sidebar(ui);
        self.draw_top_bar(ui);
        Panel::bottom("craftward_footer")
            .frame(
                Frame::NONE
                    .fill(BACKGROUND)
                    .inner_margin(Margin::symmetric(PAGE_MARGIN, 8))
                    .stroke(Stroke::new(1.0, LINE)),
            )
            .show(ui, |ui| {
                self.draw_undo_toast(ui);
            });
        let action = self.draw_central(ui);
        match (self.screen, action) {
            (_, Some(DetailAction::BrowseLockfile)) => {
                self.picker
                    .pick_lockfile(ui.ctx().clone(), self.picker_inbox.sender());
            }
            (_, Some(DetailAction::BrowseHeaderSource)) => {
                self.picker
                    .pick_header_source_folder(ui.ctx().clone(), self.picker_inbox.sender());
            }
            (Screen::Detail { index }, Some(action)) => {
                self.apply_detail_action(index, action);
            }
            (Screen::Issues, Some(action)) => {
                if let DetailAction::SetIssueStatus {
                    project_index: Some(index),
                    ..
                } = &action
                {
                    self.apply_detail_action(*index, action);
                }
            }
            _ => {}
        }
    }
}

impl CraftwardApp {
    fn draw_undo_toast(&mut self, ui: &mut Ui) {
        let now = now_secs();
        let Some(offer) = self.undo.current(now).cloned() else {
            return;
        };
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(&offer.message)
                    .font(TYPE.secondary_id())
                    .color(TEXT),
            );
            if !offer.snapshots.is_empty()
                && ui
                    .add(Button::new(
                        RichText::new("Undo")
                            .font(TYPE.secondary_id())
                            .color(ACCENT),
                    ))
                    .clicked()
            {
                match self.undo.undo(now) {
                    Ok(()) => self.reload(),
                    Err(error) => {
                        let message = error.to_string();
                        if matches!(self.screen, Screen::Detail { .. }) {
                            self.edit.banner = Some(message);
                            self.edit.conflict = error.is_conflict();
                        } else {
                            self.overview.banner = Some(message);
                        }
                    }
                }
            }
            if ui
                .add(Button::new(
                    RichText::new("Dismiss")
                        .font(TYPE.secondary_id())
                        .color(MUTED),
                ))
                .clicked()
            {
                self.undo.dismiss();
            }
        });
        ui.add_space(4.0);
    }
}

impl CraftwardApp {
    fn draw_onboarding_frame(&mut self, ui: &mut Ui) {
        CentralPanel::default()
            .frame(
                Frame::NONE
                    .fill(BACKGROUND)
                    .inner_margin(Margin::same(PAGE_MARGIN)),
            )
            .show(ui, |ui| {
                let Some(state) = self.onboarding.as_mut() else {
                    return;
                };
                let action = onboarding::draw_onboarding(
                    ui,
                    state,
                    self.picker.as_ref(),
                    &self.picker_inbox,
                );
                if action == OnboardingAction::Finished {
                    self.onboarding = None;
                    self.post_setup_tip = true;
                    self.reload();
                    self.screen = Screen::Overview;
                }
            });
    }

    fn drain_picker(&mut self) {
        while let Some(result) = self.picker_inbox.try_recv() {
            let Some(path) = result.path else {
                continue;
            };
            let text = path.display().to_string();
            match result.kind {
                PickKind::Folder => {
                    if let Some(form) = &mut self.overview.new_project {
                        form.directory = text;
                    }
                }
                PickKind::SourceFolder => {
                    if let Some(form) = &mut self.overview.new_project {
                        form.source_dir = text;
                    }
                }
                PickKind::HeaderSourceFolder => {
                    if let Some(form) = &mut self.edit.header_form {
                        form.source_dir = text;
                    }
                }
                PickKind::ProjectFile | PickKind::ProjectFolder => {
                    if let Some(form) = &mut self.overview.add_project {
                        form.path = text;
                    }
                }
                PickKind::Lockfile => {
                    if let Some(relative) = self.relative_lockfile_pick(&path) {
                        if let Some(form) = &mut self.edit.lockfile_form {
                            form.path = relative;
                        }
                        self.edit.clear_message();
                    } else if let Some(form) = &mut self.edit.lockfile_form {
                        form.path.clear();
                        self.edit.set_error(
                            ErrorSite::LockfileForm,
                            "Lockfile must be inside the source folder".to_owned(),
                            false,
                        );
                    }
                }
                PickKind::WorkspaceFolder | PickKind::DataFolder | PickKind::CacheFolder => {
                    if let Some(state) = self.onboarding.as_mut() {
                        onboarding::apply_pick(state, result.kind, &path);
                    } else if settings_panel::apply_settings_pick(
                        &self.storage,
                        &mut self.settings_edit,
                        result.kind,
                        &path,
                    ) {
                        self.reload();
                    }
                }
            }
        }
    }

    fn relative_lockfile_pick(&self, absolute: &std::path::Path) -> Option<String> {
        let Screen::Detail { index } = self.screen else {
            return None;
        };
        let loaded = self.loaded.projects.get(index)?;
        let project = loaded.project.as_ref().ok()?;
        let source = project_source_dir(&loaded.path, project);
        relative_to_dir(absolute, &source)
    }

    fn taken_project_ids(&self) -> Vec<String> {
        self.portfolio
            .cards
            .iter()
            .filter(|card| !card.id.is_empty())
            .map(|card| card.id.clone())
            .collect()
    }

    fn remove_from_list(&mut self, id: &str, name: &str, delete_file: bool, return_overview: bool) {
        self.run_workspace_save(|path, fingerprint| {
            save_remove_from_workspace(path, fingerprint, id, name, delete_file)
        });
        if return_overview {
            self.go_portfolio_overview();
        }
    }

    fn confirm_delete_file(&mut self) {
        let Some(prompt) = self.overview.delete_file_prompt.take() else {
            return;
        };
        self.remove_from_list(&prompt.id, &prompt.name, true, prompt.return_overview);
    }

    fn handle_escape(&mut self, ui: &Ui) {
        if !ui.ctx().input(|input| input.key_pressed(Key::Escape)) {
            return;
        }
        if self.overview.delete_file_prompt.is_some() {
            self.overview.delete_file_prompt = None;
        } else if self.edit.header_form.is_some()
            || self.edit.phase_form.is_some()
            || self.edit.phase_edit.is_some()
            || self.edit.task_form.is_some()
            || self.edit.task_edit.is_some()
            || self.edit.issue_form.is_some()
            || self.edit.issue_edit.is_some()
            || self.edit.component_form.is_some()
            || self.edit.component_edit.is_some()
            || self.edit.lockfile_form.is_some()
            || self.edit.android_form.is_some()
            || self.edit.watch_form.is_some()
            || self.edit.watch_edit.is_some()
        {
            self.edit.close_forms();
        } else if matches!(self.screen, Screen::Detail { .. }) {
            self.go_portfolio_overview();
        }
    }

    fn draw_sidebar(&mut self, ui: &mut Ui) {
        Panel::left("craftward_sidebar")
            .exact_size(SIDEBAR_WIDTH)
            .resizable(false)
            .frame(
                Frame::NONE
                    .fill(CARD)
                    .inner_margin(Margin::symmetric(10, 12))
                    .stroke(Stroke::new(1.0, LINE)),
            )
            .show(ui, |ui| {
                ui.label(
                    RichText::new("Craftward")
                        .font(TYPE.section_id())
                        .color(HEADING)
                        .strong(),
                );
                ui.add_space(10.0);
                let active = self.nav_item();
                if Self::draw_nav_entry(ui, "Portfolio", active == NavItem::Portfolio) {
                    self.go_portfolio_overview();
                }
                if Self::draw_nav_entry(ui, "Issues", active == NavItem::Issues) {
                    self.screen = Screen::Issues;
                    self.edit.clear_all();
                    self.focus_issue_id = None;
                }
                if Self::draw_nav_entry(ui, "Settings", active == NavItem::Settings) {
                    self.screen = Screen::Settings;
                    self.edit.clear_all();
                    self.focus_issue_id = None;
                    self.settings_edit = SettingsEdit::default();
                    self.storage = StorageSnapshot::load();
                }
                ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
                    draw_sidebar_paths(ui, &self.portfolio);
                });
            });
    }

    fn draw_nav_entry(ui: &mut Ui, label: &str, active: bool) -> bool {
        let colour = if active { ACCENT } else { TEXT };
        let fill = if active { CARD_INNER } else { CARD };
        ui.add(
            Button::new(
                RichText::new(label)
                    .font(TYPE.body_id())
                    .color(colour)
                    .strong(),
            )
            .fill(fill)
            .stroke(Stroke::new(1.0, if active { ACCENT } else { LINE }))
            .min_size(Vec2::new(SIDEBAR_WIDTH - 24.0, 28.0)),
        )
        .clicked()
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
                    match self.screen {
                        Screen::Overview => {
                            ui.label(
                                RichText::new("Portfolio")
                                    .font(TYPE.title_id())
                                    .color(TEXT)
                                    .strong(),
                            );
                        }
                        Screen::Issues => {
                            ui.label(
                                RichText::new("Issues")
                                    .font(TYPE.title_id())
                                    .color(TEXT)
                                    .strong(),
                            );
                        }
                        Screen::Settings => {
                            ui.label(
                                RichText::new("Settings")
                                    .font(TYPE.title_id())
                                    .color(TEXT)
                                    .strong(),
                            );
                        }
                        Screen::Detail { index } => {
                            let name = self.loaded.projects.get(index).map_or_else(
                                || "Unavailable project".to_owned(),
                                |project| build_detail(index, project).name,
                            );
                            ui.spacing_mut().item_spacing.x = 0.0;
                            if ui
                                .add(
                                    Button::new(
                                        RichText::new("Portfolio")
                                            .font(TYPE.title_id())
                                            .color(ACCENT)
                                            .strong(),
                                    )
                                    .fill(BACKGROUND)
                                    .stroke(Stroke::NONE)
                                    .min_size(Vec2::ZERO),
                                )
                                .on_hover_text("Return to the portfolio (Esc)")
                                .clicked()
                            {
                                self.go_portfolio_overview();
                            }
                            ui.label(
                                RichText::new(format!(" › {name}"))
                                    .font(TYPE.title_id())
                                    .color(TEXT)
                                    .strong(),
                            );
                        }
                    }
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
            .show(ui, |ui| {
                if let Some(prompt) = self.overview.delete_file_prompt.clone() {
                    self.draw_delete_file_prompt(ui, prompt);
                }
                match self.screen {
                    Screen::Overview => {
                        if let Some(index) = self.draw_overview_screen(ui) {
                            self.edit.clear_all();
                            self.focus_issue_id = None;
                            self.screen = Screen::Detail { index };
                        }
                    }
                    Screen::Issues => {
                        action = self.draw_issues_screen(ui);
                    }
                    Screen::Settings => {
                        settings_panel::draw_settings(
                            ui,
                            &self.storage,
                            &mut self.settings_edit,
                            self.picker.as_ref(),
                            &self.picker_inbox,
                        );
                    }
                    Screen::Detail { index } => {
                        if let Some(banner) = self.edit.banner.clone() {
                            ui.horizontal(|ui| {
                                ui.colored_label(SEVERITY_CRITICAL, &banner);
                                if ui
                                    .add(Button::new(
                                        RichText::new("Reload")
                                            .font(TYPE.secondary_id())
                                            .color(ACCENT),
                                    ))
                                    .clicked()
                                {
                                    action = Some(DetailAction::Reload);
                                }
                                if ui
                                    .add(Button::new(
                                        RichText::new("Dismiss")
                                            .font(TYPE.secondary_id())
                                            .color(MUTED),
                                    ))
                                    .clicked()
                                {
                                    action = Some(DetailAction::DismissBanner);
                                }
                            });
                            ui.add_space(8.0);
                        }
                        if let Some(project) = self.loaded.projects.get(index) {
                            let detail = build_detail(index, project);
                            if action.is_none() {
                                action = draw_detail(
                                    ui,
                                    &detail,
                                    project,
                                    &mut self.edit,
                                    self.focus_issue_id.as_deref(),
                                );
                            }
                        } else {
                            ui.label(
                                RichText::new("That project is no longer listed.")
                                    .font(TYPE.body_id())
                                    .color(MUTED),
                            );
                            if ui
                                .add(Button::new(
                                    RichText::new("Portfolio")
                                        .font(TYPE.body_id())
                                        .color(ACCENT),
                                ))
                                .clicked()
                            {
                                self.go_portfolio_overview();
                            }
                        }
                    }
                }
            });
        action
    }

    #[allow(clippy::too_many_lines)]
    fn draw_issues_screen(&mut self, ui: &mut Ui) -> Option<DetailAction> {
        let mut action = None;
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Status")
                    .font(TYPE.secondary_id())
                    .color(MUTED),
            );
            let status_label = if self.issues_filter.open_only {
                "Open"
            } else {
                "All"
            };
            ComboBox::from_id_salt("issues-status-filter")
                .selected_text(RichText::new(status_label).font(TYPE.body_id()).color(TEXT))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.issues_filter.open_only, true, "Open");
                    ui.selectable_value(&mut self.issues_filter.open_only, false, "All");
                });
            ui.label(
                RichText::new("Severity")
                    .font(TYPE.secondary_id())
                    .color(MUTED),
            );
            let severity_label = if self.issues_filter.severity.is_empty() {
                "All".to_owned()
            } else {
                display_label(issue_severity_choices(), &self.issues_filter.severity)
            };
            ComboBox::from_id_salt("issues-severity-filter")
                .selected_text(
                    RichText::new(severity_label)
                        .font(TYPE.body_id())
                        .color(TEXT),
                )
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.issues_filter.severity, String::new(), "All");
                    for (key, label) in issue_severity_choices() {
                        ui.selectable_value(
                            &mut self.issues_filter.severity,
                            (*key).to_owned(),
                            *label,
                        );
                    }
                });
            ui.label(
                RichText::new("Project")
                    .font(TYPE.secondary_id())
                    .color(MUTED),
            );
            let project_label = if self.issues_filter.project_id.is_empty() {
                "All".to_owned()
            } else {
                self.loaded
                    .projects
                    .iter()
                    .find_map(|project| {
                        project.project.as_ref().ok().and_then(|parsed| {
                            (parsed.id == self.issues_filter.project_id)
                                .then(|| parsed.name.clone())
                        })
                    })
                    .unwrap_or_else(|| self.issues_filter.project_id.clone())
            };
            ComboBox::from_id_salt("issues-project-filter")
                .selected_text(
                    RichText::new(project_label)
                        .font(TYPE.body_id())
                        .color(TEXT),
                )
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.issues_filter.project_id, String::new(), "All");
                    for project in &self.loaded.projects {
                        if let Ok(parsed) = &project.project {
                            ui.selectable_value(
                                &mut self.issues_filter.project_id,
                                parsed.id.clone(),
                                parsed.name.as_str(),
                            );
                        }
                    }
                });
        });
        ui.add_space(8.0);
        let rows = collect_portfolio_issues(&self.loaded, &self.issues_filter);
        if rows.is_empty() {
            ui.label(
                RichText::new("No issues match these filters.")
                    .font(TYPE.body_id())
                    .color(MUTED),
            );
            return None;
        }
        let status_width = status_chip_column_width(ui, issue_status_choices());
        let severity_width = 110.0_f32;
        let status_col = status_width + 8.0;
        let project_width = 140.0_f32;
        ui.horizontal(|ui| {
            ui.add_sized(
                [severity_width, 18.0],
                Label::new(
                    RichText::new("Severity")
                        .font(TYPE.secondary_id())
                        .color(MUTED),
                ),
            );
            ui.add_sized(
                [status_col, 18.0],
                Label::new(
                    RichText::new("Status")
                        .font(TYPE.secondary_id())
                        .color(MUTED),
                ),
            );
            ui.add_sized(
                [project_width, 18.0],
                Label::new(
                    RichText::new("Project")
                        .font(TYPE.secondary_id())
                        .color(MUTED),
                ),
            );
            ui.label(
                RichText::new("Title")
                    .font(TYPE.secondary_id())
                    .color(MUTED),
            );
        });
        ui.add_space(4.0);
        ScrollArea::vertical().show(ui, |ui| {
            for row in &rows {
                ui.horizontal(|ui| {
                    ui.allocate_ui_with_layout(
                        Vec2::new(severity_width, ui.spacing().interact_size.y),
                        Layout::left_to_right(Align::Center),
                        |ui| {
                            draw_badge(ui, &row.severity, None);
                        },
                    );
                    ui.allocate_ui_with_layout(
                        Vec2::new(status_col, ui.spacing().interact_size.y),
                        Layout::left_to_right(Align::Center),
                        |ui| {
                            if let Some(status) = status_menu(
                                ui,
                                &format!("portfolio-issue-{}", row.issue_id),
                                row.status.label,
                                status_colour(&row.status),
                                issue_status_choices(),
                                status_width,
                            ) {
                                action = Some(DetailAction::SetIssueStatus {
                                    project_index: Some(row.project_index),
                                    issue_id: row.issue_id.clone(),
                                    status: status.to_owned(),
                                    title: row.title.clone(),
                                });
                            }
                        },
                    );
                    let project_clicked = ui
                        .add_sized(
                            [project_width, ui.spacing().interact_size.y],
                            Button::new(
                                RichText::new(&row.project_name)
                                    .font(TYPE.body_id())
                                    .color(TEXT),
                            )
                            .fill(BACKGROUND)
                            .stroke(Stroke::NONE),
                        )
                        .clicked();
                    let title_response = ui.add(
                        Label::new(RichText::new(&row.title).font(TYPE.body_id()).color(TEXT))
                            .wrap()
                            .sense(Sense::click()),
                    );
                    if project_clicked || title_response.clicked() {
                        self.open_project_issue(row.project_index, &row.issue_id);
                    }
                });
                ui.add_space(2.0);
            }
        });
        action
    }
}

fn apply_theme(ctx: &Context) {
    ctx.set_theme(ThemePreference::Dark);
    ctx.set_style_of(Theme::Dark, theme::style());
}

fn draw_sidebar_paths(ui: &mut Ui, portfolio: &PortfolioView) {
    if let Some(error) = &portfolio.workspace_error {
        ui.colored_label(SEVERITY_CRITICAL, error);
        return;
    }
    if let Some(path) = &portfolio.workspace_path {
        ui.label(
            RichText::new(format!("Workspace\n{path}"))
                .font(TYPE.secondary_id())
                .color(MUTED),
        )
        .on_hover_text(path);
    }
    if let Some(path) = &portfolio.database_path {
        ui.label(
            RichText::new(format!("Database\n{path}"))
                .font(TYPE.secondary_id())
                .color(MUTED),
        )
        .on_hover_text(path);
    }
    if portfolio.workspace_path.is_none() && portfolio.database_path.is_none() {
        ui.label(
            RichText::new("No workspace or database path resolved.")
                .font(TYPE.secondary_id())
                .color(MUTED),
        );
    }
    if let Some(message) = &portfolio.database_message {
        ui.label(
            RichText::new(message)
                .font(TYPE.secondary_id())
                .color(RUNNING),
        );
    }
}

impl CraftwardApp {
    fn draw_overview_screen(&mut self, ui: &mut Ui) -> Option<usize> {
        if let Some(banner) = self.overview.banner.clone() {
            ui.horizontal(|ui| {
                ui.colored_label(SEVERITY_CRITICAL, banner);
                if ui
                    .add(Button::new(
                        RichText::new("Dismiss")
                            .font(TYPE.secondary_id())
                            .color(MUTED),
                    ))
                    .clicked()
                {
                    self.overview.banner = None;
                }
            });
            ui.add_space(8.0);
        }
        if self.post_setup_tip {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(
                        "Setup complete. Use New project or Add existing to fill the portfolio.",
                    )
                    .font(TYPE.body_id())
                    .color(TEXT),
                );
                if ui
                    .add(Button::new(
                        RichText::new("Dismiss")
                            .font(TYPE.secondary_id())
                            .color(MUTED),
                    ))
                    .clicked()
                {
                    self.post_setup_tip = false;
                }
            });
            ui.add_space(8.0);
        }
        ui.horizontal(|ui| {
            if ui
                .add(Button::new(
                    RichText::new("New project")
                        .font(TYPE.secondary_id())
                        .color(ACCENT),
                ))
                .clicked()
            {
                self.overview.add_project = None;
                self.overview.new_project = Some(NewProjectForm::default());
                self.post_setup_tip = false;
            }
            if ui
                .add(Button::new(
                    RichText::new("Add existing")
                        .font(TYPE.secondary_id())
                        .color(ACCENT),
                ))
                .clicked()
            {
                self.overview.new_project = None;
                self.overview.add_project = Some(AddProjectForm::default());
                self.post_setup_tip = false;
            }
        });
        if let Some(form) = self.overview.new_project.clone() {
            self.draw_new_project_form(ui, form);
        }
        if let Some(form) = self.overview.add_project.clone() {
            self.draw_add_project_form(ui, form);
        }
        ui.add_space(CARD_GAP);
        let (open, card_action) = draw_overview(ui, &self.portfolio, &self.overview);
        match card_action {
            Some(CardAction::RemoveFromList { id, name }) => {
                self.remove_from_list(&id, &name, false, false);
            }
            Some(CardAction::PromptDeleteFile { id, name, index }) => {
                let path = self
                    .loaded
                    .projects
                    .get(index)
                    .map_or_default(|project| project.path.display().to_string());
                self.overview.delete_file_prompt = Some(DeleteFilePrompt {
                    id,
                    name,
                    path,
                    return_overview: false,
                });
            }
            Some(CardAction::Open | CardAction::None) | None => {}
        }
        open
    }

    fn draw_delete_file_prompt(&mut self, ui: &mut Ui, prompt: DeleteFilePrompt) {
        ui.group(|ui| {
            ui.label(
                RichText::new(format!("Delete craftward.json for {}?", prompt.name))
                    .font(TYPE.section_id())
                    .color(HEADING)
                    .strong(),
            );
            ui.colored_label(
                SEVERITY_CRITICAL,
                format!(
                    "This removes the project from the list and permanently deletes only this file. It cannot be undone:\n{}",
                    prompt.path
                ),
            );
            ui.horizontal(|ui| {
                if compact_button(ui, "Delete file", SEVERITY_CRITICAL).clicked() {
                    self.overview.delete_file_prompt = Some(prompt);
                    self.confirm_delete_file();
                    return;
                }
                if compact_button(ui, "Cancel", MUTED).clicked() {
                    self.overview.delete_file_prompt = None;
                    return;
                }
                self.overview.delete_file_prompt = Some(prompt);
            });
        });
        ui.add_space(8.0);
    }

    #[allow(clippy::too_many_lines)]
    fn draw_new_project_form(&mut self, ui: &mut Ui, mut form: NewProjectForm) {
        let taken = self.taken_project_ids();
        ui.group(|ui| {
            ui.label(
                RichText::new("New project")
                    .font(TYPE.section_id())
                    .color(HEADING)
                    .strong(),
            );
            ui.label(
                RichText::new("Folder for craftward.json")
                    .font(TYPE.secondary_id())
                    .color(MUTED),
            );
            ui.label(
                RichText::new(
                    "Must exist and must not already contain craftward.json. This is where the project file is written.",
                )
                .font(TYPE.secondary_id())
                .color(MUTED),
            );
            ui.horizontal(|ui| {
                ui.add(
                    TextEdit::singleline(&mut form.directory)
                        .font(TYPE.body_id())
                        .desired_width(320.0),
                );
                if compact_button(ui, "Browse…", ACCENT).clicked() {
                    self.picker
                        .pick_folder(ui.ctx().clone(), self.picker_inbox.sender());
                }
            });
            field_error(ui, form.directory_error());

            ui.label(
                RichText::new("Source folder (if the code lives elsewhere)")
                    .font(TYPE.secondary_id())
                    .color(MUTED),
            );
            ui.label(
                RichText::new(
                    "Optional. When set, lockfiles and Android detect_from resolve from this folder instead of the craftward.json folder.",
                )
                .font(TYPE.secondary_id())
                .color(MUTED),
            );
            ui.horizontal(|ui| {
                ui.add(
                    TextEdit::singleline(&mut form.source_dir)
                        .font(TYPE.body_id())
                        .desired_width(320.0),
                );
                if compact_button(ui, "Browse…", ACCENT).clicked() {
                    self.picker
                        .pick_source_folder(ui.ctx().clone(), self.picker_inbox.sender());
                }
            });
            field_error(ui, form.source_dir_error());

            ui.horizontal(|ui| {
                ui.label(RichText::new("Name").font(TYPE.secondary_id()).color(MUTED));
                let name_before = form.name.clone();
                ui.add(
                    TextEdit::singleline(&mut form.name)
                        .font(TYPE.body_id())
                        .desired_width(220.0),
                );
                if form.name != name_before {
                    form.suggest_id_from_name();
                }
            });
            field_error(ui, form.name_error());

            ui.horizontal(|ui| {
                ui.label(RichText::new("Id").font(TYPE.secondary_id()).color(MUTED));
                let id_before = form.id.clone();
                ui.add(
                    TextEdit::singleline(&mut form.id)
                        .font(TYPE.body_id())
                        .desired_width(220.0),
                );
                if form.id != id_before {
                    form.mark_id_manual();
                }
            });
            ui.label(
                RichText::new(
                    "Used in commands such as craftward show <id>. Prefer not to change it later.",
                )
                .font(TYPE.secondary_id())
                .color(MUTED),
            );
            field_error(ui, form.id_error(&taken));

            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Status")
                        .font(TYPE.secondary_id())
                        .color(MUTED),
                );
                let selected = project_status_choices()
                    .iter()
                    .find(|(key, _)| *key == form.status)
                    .map_or(form.status.as_str(), |(_, label)| *label);
                ComboBox::from_id_salt("new-project-status")
                    .selected_text(RichText::new(selected).font(TYPE.body_id()).color(TEXT))
                    .show_ui(ui, |ui| {
                        for (key, label) in project_status_choices() {
                            ui.selectable_value(&mut form.status, (*key).to_owned(), *label)
                                .on_hover_text(project_status_hint(key));
                        }
                    })
                    .response
                    .on_hover_text(project_status_hint(&form.status));
            });
            field_error(ui, form.status_error());

            ui.horizontal(|ui| {
                let can = form.can_save(&taken);
                if ui
                    .add_enabled(
                        can,
                        Button::new(RichText::new("Create").font(TYPE.body_id()).color(ACCENT)),
                    )
                    .clicked()
                {
                    let form = form.clone();
                    self.run_workspace_save(|path, fingerprint| {
                        save_new_project(path, fingerprint, &form)
                    });
                    self.overview.new_project = None;
                    return;
                }
                if compact_button(ui, "Cancel", MUTED).clicked() {
                    self.overview.new_project = None;
                    return;
                }
                self.overview.new_project = Some(form);
            });
        });
    }

    fn draw_add_project_form(&mut self, ui: &mut Ui, mut form: AddProjectForm) {
        ui.group(|ui| {
            ui.label(
                RichText::new("Add existing craftward.json")
                    .font(TYPE.section_id())
                    .color(HEADING)
                    .strong(),
            );
            ui.horizontal(|ui| {
                ui.add(
                    TextEdit::singleline(&mut form.path)
                        .font(TYPE.body_id())
                        .desired_width(320.0),
                );
                if compact_button(ui, "Browse folder…", ACCENT).clicked() {
                    self.picker
                        .pick_project_folder(ui.ctx().clone(), self.picker_inbox.sender());
                }
                if compact_button(ui, "Browse file…", ACCENT).clicked() {
                    self.picker
                        .pick_project_file(ui.ctx().clone(), self.picker_inbox.sender());
                }
            });
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        form.can_save(),
                        Button::new(RichText::new("Add").font(TYPE.body_id()).color(ACCENT)),
                    )
                    .clicked()
                {
                    let path = form.path.clone();
                    self.run_workspace_save(|workspace, fingerprint| {
                        save_add_project(workspace, fingerprint, &path)
                    });
                    self.overview.add_project = None;
                    return;
                }
                if compact_button(ui, "Cancel", MUTED).clicked() {
                    self.overview.add_project = None;
                    return;
                }
                self.overview.add_project = Some(form);
            });
        });
    }
}

fn draw_overview(
    ui: &mut Ui,
    portfolio: &PortfolioView,
    overview: &OverviewEdit,
) -> (Option<usize>, Option<CardAction>) {
    let mut open = None;
    let mut remove = None;
    ScrollArea::vertical().show(ui, |ui| {
        draw_summary(ui, &portfolio.summary);
        ui.add_space(CARD_GAP);

        if portfolio.cards.is_empty()
            && overview.new_project.is_none()
            && overview.add_project.is_none()
        {
            ui.label(
                RichText::new("No projects in the workspace yet. Use New project or Add existing.")
                    .font(TYPE.body_id())
                    .color(MUTED),
            );
            return;
        }
        if portfolio.cards.is_empty() {
            return;
        }

        let columns = grid_columns(ui.available_width());
        let mut row: Vec<&ProjectCard> = Vec::with_capacity(columns);
        for card in &portfolio.cards {
            row.push(card);
            if row.len() == columns {
                let (opened, action) = draw_card_row(ui, &row, columns);
                if opened.is_some() {
                    open = opened;
                }
                if action.is_some() {
                    remove = action;
                }
                row.clear();
            }
        }
        if !row.is_empty() {
            let (opened, action) = draw_card_row(ui, &row, columns);
            if opened.is_some() {
                open = opened;
            }
            if action.is_some() {
                remove = action;
            }
        }
    });
    (open, remove)
}

fn draw_card_row(
    ui: &mut Ui,
    cards: &[&ProjectCard],
    columns: usize,
) -> (Option<usize>, Option<CardAction>) {
    let mut open = None;
    let mut action = None;
    let slots = grid_row_column_slots(columns, cards.len());
    ui.columns(slots, |cols| {
        for (column, card) in cols.iter_mut().zip(cards.iter()) {
            match draw_card(column, card) {
                CardAction::Open => open = Some(card.index),
                next
                @ (CardAction::RemoveFromList { .. } | CardAction::PromptDeleteFile { .. }) => {
                    action = Some(next);
                }
                CardAction::None => {}
            }
        }
    });
    ui.add_space(CARD_GAP);
    (open, action)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CardAction {
    None,
    Open,
    RemoveFromList {
        id: String,
        name: String,
    },
    PromptDeleteFile {
        id: String,
        name: String,
        index: usize,
    },
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

#[allow(clippy::too_many_lines)]
fn draw_card(ui: &mut Ui, card: &ProjectCard) -> CardAction {
    let mut menu_action = CardAction::None;
    // Card click sense is registered *under* children (UiBuilder::sense), so the
    // ··· menu_button keeps pointer/keyboard priority and can open its popup.
    let response = ui
        .scope_builder(
            UiBuilder::new()
                .id_salt(("project_card", card.id.as_str()))
                .sense(Sense::click()),
            |ui| {
                card_frame()
                    .stroke(Stroke::new(1.0, LINE))
                    .show(ui, |ui| {
                        ui.set_min_height(168.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(&card.name)
                                    .font(TYPE.title_id())
                                    .color(TEXT)
                                    .strong(),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.menu_button(
                                    RichText::new("···")
                                        .font(TYPE.secondary_id())
                                        .color(MUTED),
                                    |ui| {
                                        if ui
                                            .button(
                                                RichText::new("Remove from list")
                                                    .font(TYPE.body_id())
                                                    .color(TEXT),
                                            )
                                            .on_hover_text(
                                                "Removes the project from the list. Undo restores it. craftward.json stays on disk.",
                                            )
                                            .clicked()
                                        {
                                            menu_action = CardAction::RemoveFromList {
                                                id: card.id.clone(),
                                                name: card.name.clone(),
                                            };
                                            ui.close();
                                        }
                                        if ui
                                            .button(
                                                RichText::new("Remove and delete craftward.json…")
                                                    .font(TYPE.body_id())
                                                    .color(TEXT),
                                            )
                                            .on_hover_text(
                                                "Asks for confirmation. Deleting craftward.json cannot be undone.",
                                            )
                                            .clicked()
                                        {
                                            menu_action = CardAction::PromptDeleteFile {
                                                id: card.id.clone(),
                                                name: card.name.clone(),
                                                index: card.index,
                                            };
                                            ui.close();
                                        }
                                    },
                                );
                            });
                        });
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
                                RichText::new(format!(
                                    "Next due: {}{remaining}",
                                    format_date_for_display(due)
                                ))
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
                    });
            },
        )
        .response;

    if !matches!(menu_action, CardAction::None) {
        return menu_action;
    }
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    // Enter on the focused card opens the project; Enter on the focused ···
    // button is handled by egui's menu_button and opens the menu instead.
    if response.clicked()
        || (response.has_focus() && ui.input(|input| input.key_pressed(Key::Enter)))
    {
        CardAction::Open
    } else {
        CardAction::None
    }
}

fn draw_detail(
    ui: &mut Ui,
    detail: &ProjectDetail,
    loaded: &LoadedProject,
    edit: &mut DetailEdit,
    focus_issue_id: Option<&str>,
) -> Option<DetailAction> {
    let mut action = None;
    ScrollArea::vertical().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(&detail.name)
                    .font(TYPE.title_id())
                    .color(TEXT)
                    .strong(),
            );
            if detail.load_error.is_none() && !detail.id.is_empty() {
                if compact_button(ui, "Remove from list", MUTED)
                    .on_hover_text(
                        "Removes the project from the list. Undo restores it. craftward.json stays on disk.",
                    )
                    .clicked()
                {
                    action = Some(DetailAction::RemoveFromList {
                        id: detail.id.clone(),
                        name: detail.name.clone(),
                    });
                }
                if compact_button(ui, "Delete craftward.json…", MUTED)
                    .on_hover_text("Asks for confirmation. Deleting the file cannot be undone.")
                    .clicked()
                {
                    action = Some(DetailAction::PromptDeleteFile {
                        id: detail.id.clone(),
                        name: detail.name.clone(),
                        path: detail.path.clone(),
                    });
                }
            }
        });
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
                if let Some(next) = draw_detail_left(&mut cols[0], detail, edit, editable) {
                    action = Some(next);
                }
                if let Some(next) =
                    draw_detail_right(&mut cols[1], detail, edit, editable, focus_issue_id)
                {
                    action = Some(next);
                }
            });
        } else {
            if let Some(next) = draw_detail_left(ui, detail, edit, editable) {
                action = Some(next);
            }
            ui.add_space(CARD_GAP);
            if let Some(next) = draw_detail_right(ui, detail, edit, editable, focus_issue_id) {
                action = Some(next);
            }
        }
    });
    action
}

fn draw_detail_left(
    ui: &mut Ui,
    detail: &ProjectDetail,
    edit: &mut DetailEdit,
    editable: bool,
) -> Option<DetailAction> {
    let mut action = None;
    section(ui, "Overview", false, |ui| {
        if edit.header_form.is_some() {
            if let Some(next) = draw_header_form(ui, edit) {
                action = Some(next);
            }
        } else {
            draw_overview_readonly(ui, detail, edit, editable);
        }
        ui.add_space(6.0);
        draw_progress(ui, detail.progress);
    });

    ui.add_space(CARD_GAP);
    let stack_empty = detail.components.is_empty()
        && detail.lockfiles.is_empty()
        && detail.android.is_none()
        && edit.component_form.is_none()
        && edit.component_edit.is_none()
        && edit.lockfile_form.is_none()
        && edit.android_form.is_none();
    section(ui, "Stack", stack_empty && !editable, |ui| {
        if let Some(next) = draw_stack_section(ui, detail, edit, editable) {
            action = Some(next);
        }
    });

    ui.add_space(CARD_GAP);
    let deadlines_empty =
        detail.deadlines.is_empty() && edit.watch_form.is_none() && edit.watch_edit.is_none();
    section(
        ui,
        "Watch items & deadlines",
        deadlines_empty && !editable,
        |ui| {
            if let Some(next) = draw_deadlines_section(ui, detail, edit, editable) {
                action = Some(next);
            }
        },
    );

    ui.add_space(CARD_GAP);
    section(ui, "Open findings", detail.findings.is_empty(), |ui| {
        for finding in &detail.findings {
            draw_finding(ui, finding);
        }
    });
    action
}

#[allow(clippy::too_many_lines)]
fn draw_stack_section(
    ui: &mut Ui,
    detail: &ProjectDetail,
    edit: &mut DetailEdit,
    editable: bool,
) -> Option<DetailAction> {
    let mut action = None;
    if edit.component_edit.is_some() {
        if let Some(next) = draw_component_edit_form(ui, edit) {
            action = Some(next);
        }
        return action;
    }
    if edit.component_form.is_some() {
        if let Some(next) = draw_component_form(ui, edit) {
            action = Some(next);
        }
        return action;
    }
    if edit.lockfile_form.is_some() {
        if let Some(next) = draw_lockfile_form(ui, detail, edit) {
            action = Some(next);
        }
        return action;
    }
    if edit.android_form.is_some() {
        if let Some(next) = draw_android_form(ui, edit) {
            action = Some(next);
        }
        return action;
    }

    for component in &detail.components {
        ui.horizontal(|ui| {
            draw_badge(ui, &component.kind, None);
            let version = component
                .version
                .as_ref()
                .map_or(String::new(), |value| format!(" {value}"));
            let eol = component
                .eol_product
                .as_ref()
                .map_or(String::new(), |value| format!(" · support data: {value}"));
            ui.add(
                Label::new(
                    RichText::new(format!("{}{version}{eol}", component.name))
                        .font(TYPE.body_id())
                        .color(TEXT),
                )
                .wrap(),
            );
            if editable {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.menu_button(
                        RichText::new("···").font(TYPE.secondary_id()).color(MUTED),
                        |ui| {
                            if ui.button("Edit").clicked() {
                                edit.close_forms();
                                edit.component_edit = Some((
                                    component.name.clone(),
                                    ComponentEditForm::from_component(component),
                                ));
                                ui.close();
                            }
                            if ui.button("Remove").clicked() {
                                action = Some(DetailAction::RemoveComponent {
                                    name: component.name.clone(),
                                });
                                ui.close();
                            }
                        },
                    );
                });
            }
        });
    }

    if !detail.lockfiles.is_empty() {
        ui.add_space(4.0);
        ui.label(
            RichText::new("Lockfiles")
                .font(TYPE.secondary_id())
                .color(HEADING)
                .strong(),
        );
        for path in &detail.lockfiles {
            ui.horizontal(|ui| {
                ui.add(Label::new(RichText::new(path).font(TYPE.body_id()).color(TEXT)).wrap());
                if editable {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.menu_button(
                            RichText::new("···").font(TYPE.secondary_id()).color(MUTED),
                            |ui| {
                                if ui.button("Remove").clicked() {
                                    action =
                                        Some(DetailAction::RemoveLockfile { path: path.clone() });
                                    ui.close();
                                }
                            },
                        );
                    });
                }
            });
        }
    }

    ui.add_space(4.0);
    ui.label(
        RichText::new("Android")
            .font(TYPE.secondary_id())
            .color(HEADING)
            .strong(),
    );
    if let Some(android) = &detail.android {
        ui.label(
            RichText::new(format!(
                "targetSdk {} · minSdk {} · {} · {}",
                android
                    .target_sdk
                    .map_or_else(|| "—".to_owned(), |value| value.to_string()),
                android
                    .min_sdk
                    .map_or_else(|| "—".to_owned(), |value| value.to_string()),
                display_label(form_factor_choices(), &android.form_factor),
                display_label(android_store_choices(), &android.store)
            ))
            .font(TYPE.body_id())
            .color(TEXT),
        );
        if !android.detect_from.is_empty() {
            ui.label(
                RichText::new(format!("detect_from: {}", android.detect_from.join(", ")))
                    .font(TYPE.secondary_id())
                    .color(MUTED),
            );
        }
    } else {
        ui.label(
            RichText::new("Not set")
                .font(TYPE.secondary_id())
                .color(MUTED),
        );
    }

    if editable {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if compact_button(ui, "Add component", ACCENT).clicked() {
                edit.close_forms();
                edit.component_form = Some(ComponentForm::default());
            }
            if compact_button(ui, "Add lockfile", ACCENT).clicked() {
                edit.close_forms();
                edit.lockfile_form = Some(LockfileForm::default());
            }
            if compact_button(ui, "Edit Android", ACCENT).clicked() {
                edit.close_forms();
                edit.android_form = Some(AndroidForm::from_detail(detail.android.as_ref()));
            }
        });
    }
    action
}

fn draw_deadlines_section(
    ui: &mut Ui,
    detail: &ProjectDetail,
    edit: &mut DetailEdit,
    editable: bool,
) -> Option<DetailAction> {
    let mut action = None;
    if edit.watch_edit.is_some() {
        if let Some(next) = draw_watch_edit_form(ui, edit) {
            action = Some(next);
        }
        return action;
    }
    if edit.watch_form.is_some() {
        if let Some(next) = draw_watch_form(ui, edit) {
            action = Some(next);
        }
        return action;
    }

    for row in &detail.deadlines {
        ui.horizontal(|ui| {
            draw_badge(ui, &row.kind, None);
            let text = format!(
                "{} — {} ({})",
                row.title,
                format_date_for_display(&row.due),
                row.remaining
            );
            if editable && row.watch_id.is_some() {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if let Some(watch_id) = &row.watch_id {
                        ui.menu_button(
                            RichText::new("···").font(TYPE.secondary_id()).color(MUTED),
                            |ui| {
                                if ui.button("Edit").clicked() {
                                    edit.close_forms();
                                    edit.watch_edit =
                                        Some((watch_id.clone(), WatchEditForm::from_deadline(row)));
                                    ui.close();
                                }
                                if ui.button("Remove").clicked() {
                                    action = Some(DetailAction::RemoveWatch {
                                        watch_id: watch_id.clone(),
                                        title: row.title.clone(),
                                    });
                                    ui.close();
                                }
                            },
                        );
                    }
                    ui.add(Label::new(RichText::new(text).font(TYPE.body_id()).color(TEXT)).wrap());
                });
            } else {
                ui.add(Label::new(RichText::new(text).font(TYPE.body_id()).color(TEXT)).wrap());
            }
        });
    }

    if editable {
        ui.add_space(6.0);
        if compact_button(ui, "Add watch item", ACCENT).clicked() {
            edit.close_forms();
            edit.watch_form = Some(WatchForm::default());
        }
    }
    action
}

fn draw_overview_readonly(
    ui: &mut Ui,
    detail: &ProjectDetail,
    edit: &mut DetailEdit,
    editable: bool,
) {
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
            ui.add(Label::new(RichText::new(description).font(TYPE.body_id()).color(MUTED)).wrap());
        });
    }
    if editable && compact_button(ui, "Edit", ACCENT).clicked() {
        edit.close_forms();
        edit.header_form = Some(HeaderForm::from_detail(detail));
    }
}

fn draw_detail_right(
    ui: &mut Ui,
    detail: &ProjectDetail,
    edit: &mut DetailEdit,
    editable: bool,
    focus_issue_id: Option<&str>,
) -> Option<DetailAction> {
    let mut action = draw_phases_panel(ui, detail, edit, editable);
    ui.add_space(CARD_GAP);
    if let Some(next) = draw_issues_panel(ui, detail, edit, editable, focus_issue_id) {
        action = Some(next);
    }
    action
}

#[allow(clippy::too_many_lines)]
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
                ui.horizontal(|ui| {
                    ui.add(
                        Label::new(
                            RichText::new(&phase.id)
                                .font(TYPE.secondary_id())
                                .color(MUTED),
                        )
                        .wrap(),
                    )
                    .on_hover_text(format!("Phase id: {}", phase.id));
                    if editable {
                        ui.menu_button(
                            RichText::new("···").font(TYPE.secondary_id()).color(MUTED),
                            |ui| {
                                if ui
                                    .button(RichText::new("Edit").font(TYPE.body_id()).color(TEXT))
                                    .clicked()
                                {
                                    edit.close_forms();
                                    edit.phase_edit =
                                        Some((phase.id.clone(), PhaseForm::from_phase(phase)));
                                    ui.close();
                                }
                                if ui
                                    .button(
                                        RichText::new("Remove").font(TYPE.body_id()).color(TEXT),
                                    )
                                    .clicked()
                                {
                                    action = Some(DetailAction::RemovePhase {
                                        phase_id: phase.id.clone(),
                                        title: phase.title.clone(),
                                        task_count: phase.tasks.len(),
                                    });
                                    ui.close();
                                }
                            },
                        );
                    }
                });
                if editable
                    && edit
                        .phase_edit
                        .as_ref()
                        .is_some_and(|(id, _)| id == &phase.id)
                {
                    if let Some(next) = draw_phase_edit_form(ui, edit) {
                        action = Some(next);
                    }
                } else if let Some(goal) = &phase.goal {
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
                    } else if compact_button(ui, "Add task", ACCENT).clicked() {
                        edit.close_forms();
                        edit.task_form = Some(TaskForm::new(phase.id.clone()));
                    }
                }
            });
            ui.add_space(4.0);
        }
        if editable {
            if edit.phase_form.is_some() {
                if let Some(next) = draw_phase_add_form(ui, edit) {
                    action = Some(next);
                }
            } else if compact_button(ui, "Add phase", ACCENT).clicked() {
                edit.close_forms();
                edit.phase_form = Some(PhaseForm::default());
            }
        }
    });
    action
}

fn draw_task_row(
    ui: &mut Ui,
    phase_id: &str,
    task: &crate::view::TaskRow,
    edit: &mut DetailEdit,
    editable: bool,
    status_width: f32,
) -> Option<DetailAction> {
    let mut action = None;
    let title_colour = if task_title_is_muted(task.status.key) {
        MUTED
    } else {
        TEXT
    };
    let editing = edit
        .task_edit
        .as_ref()
        .is_some_and(|(p, t, _)| p == phase_id && t == &task.id);
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
                    title: task.title.clone(),
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
                RichText::new(format!("due {}", format_date_for_display(due)))
                    .font(TYPE.secondary_id())
                    .color(MUTED),
            );
        }
        if editable {
            ui.menu_button(
                RichText::new("···").font(TYPE.secondary_id()).color(MUTED),
                |ui| {
                    if ui
                        .button(RichText::new("Edit").font(TYPE.body_id()).color(TEXT))
                        .clicked()
                    {
                        edit.close_forms();
                        edit.task_edit = Some((
                            phase_id.to_owned(),
                            task.id.clone(),
                            TaskEditForm::from_task(task),
                        ));
                        ui.close();
                    }
                    if ui
                        .button(RichText::new("Remove").font(TYPE.body_id()).color(TEXT))
                        .clicked()
                    {
                        action = Some(DetailAction::RemoveTask {
                            phase_id: phase_id.to_owned(),
                            task_id: task.id.clone(),
                            title: task.title.clone(),
                        });
                        ui.close();
                    }
                },
            );
        }
    });
    if editing && let Some(next) = draw_task_edit_form(ui, edit) {
        action = Some(next);
    }
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

#[allow(clippy::too_many_lines)]
fn draw_issues_panel(
    ui: &mut Ui,
    detail: &ProjectDetail,
    edit: &mut DetailEdit,
    editable: bool,
    focus_issue_id: Option<&str>,
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
            let focused = focus_issue_id == Some(issue.id.as_str());
            let editing = edit
                .issue_edit
                .as_ref()
                .is_some_and(|(id, _)| id == &issue.id);
            ui.horizontal(|ui| {
                if focused {
                    ui.label(
                        RichText::new("›")
                            .font(TYPE.body_id())
                            .color(ACCENT)
                            .strong(),
                    );
                }
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
                            project_index: None,
                            issue_id: issue.id.clone(),
                            status: status.to_owned(),
                            title: issue.title.clone(),
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
                let title_colour = if focused { ACCENT } else { TEXT };
                ui.add(
                    Label::new(
                        RichText::new(&issue.title)
                            .font(TYPE.body_id())
                            .color(title_colour),
                    )
                    .wrap(),
                )
                .on_hover_text(format!("Issue id: {}", issue.id));
                if editable {
                    ui.menu_button(
                        RichText::new("···").font(TYPE.secondary_id()).color(MUTED),
                        |ui| {
                            if ui
                                .button(RichText::new("Edit").font(TYPE.body_id()).color(TEXT))
                                .clicked()
                            {
                                edit.close_forms();
                                edit.issue_edit =
                                    Some((issue.id.clone(), IssueEditForm::from_issue(issue)));
                                ui.close();
                            }
                            if ui
                                .button(RichText::new("Remove").font(TYPE.body_id()).color(TEXT))
                                .clicked()
                            {
                                action = Some(DetailAction::RemoveIssue {
                                    issue_id: issue.id.clone(),
                                    title: issue.title.clone(),
                                });
                                ui.close();
                            }
                        },
                    );
                }
            });
            if editing && let Some(next) = draw_issue_edit_form(ui, edit) {
                action = Some(next);
            }
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
            } else if compact_button(ui, "Add issue", ACCENT).clicked() {
                edit.close_forms();
                edit.issue_form = Some(IssueForm::default());
            }
        }
    });
    action
}

#[allow(clippy::too_many_lines)]
fn draw_header_form(ui: &mut Ui, edit: &mut DetailEdit) -> Option<DetailAction> {
    let mut action = None;
    let mut cancel = false;
    let can_save;
    let name_error;
    let status_error;
    let links_error;
    {
        let form = edit.header_form.as_mut()?;
        ui.label(
            RichText::new("Edit project")
                .font(TYPE.secondary_id())
                .color(HEADING)
                .strong(),
        );
        form_labelled_line(ui, "Name", |ui| {
            ui.add(
                TextEdit::singleline(&mut form.name)
                    .font(TYPE.body_id())
                    .desired_width(220.0),
            );
        });
        name_error = form.name_error();
        field_error(ui, name_error);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Status")
                    .font(TYPE.secondary_id())
                    .color(MUTED),
            );
            let selected = project_status_choices()
                .iter()
                .find(|(key, _)| *key == form.status)
                .map_or(form.status.as_str(), |(_, label)| *label);
            ComboBox::from_id_salt("header-status")
                .selected_text(RichText::new(selected).font(TYPE.body_id()).color(TEXT))
                .show_ui(ui, |ui| {
                    for (key, label) in project_status_choices() {
                        ui.selectable_value(&mut form.status, (*key).to_owned(), *label);
                    }
                });
        });
        status_error = form.status_error();
        field_error(ui, status_error);
        form_labelled_line(ui, "Version", |ui| {
            ui.add(
                TextEdit::singleline(&mut form.version)
                    .font(TYPE.body_id())
                    .desired_width(160.0),
            );
        });
        form_labelled_line(ui, "Description", |ui| {
            ui.add(
                TextEdit::multiline(&mut form.description)
                    .font(TYPE.body_id())
                    .desired_width(f32::INFINITY)
                    .desired_rows(2),
            );
        });
        form_labelled_line(ui, "Tags", |ui| {
            ui.add(
                TextEdit::singleline(&mut form.tags)
                    .font(TYPE.body_id())
                    .hint_text("comma-separated")
                    .desired_width(220.0),
            );
        });
        ui.label(
            RichText::new("Source folder (if the code lives elsewhere)")
                .font(TYPE.secondary_id())
                .color(MUTED),
        );
        ui.label(
            RichText::new(
                "Optional. Lockfiles and Android detect_from resolve from this folder when set.",
            )
            .font(TYPE.secondary_id())
            .color(MUTED),
        );
        ui.horizontal(|ui| {
            ui.add(
                TextEdit::singleline(&mut form.source_dir)
                    .font(TYPE.body_id())
                    .desired_width(220.0),
            );
            if compact_button(ui, "Browse…", ACCENT).clicked() {
                action = Some(DetailAction::BrowseHeaderSource);
            }
        });
        ui.label(
            RichText::new("Links (Label|URL per line)")
                .font(TYPE.secondary_id())
                .color(MUTED),
        );
        ui.add(
            TextEdit::multiline(&mut form.links)
                .font(TYPE.body_id())
                .desired_width(f32::INFINITY)
                .desired_rows(3),
        );
        links_error = form.links_error();
        field_error(ui, links_error);
        can_save = form.can_save();
    }
    // No Enter shortcut: description and links are multiline.
    draw_form_actions(
        ui,
        can_save,
        &mut action,
        DetailAction::SaveHeader,
        &mut cancel,
    );
    if cancel {
        edit.header_form = None;
        edit.clear_message();
        return None;
    }
    if matches!(&edit.error_site, Some(ErrorSite::HeaderForm)) {
        draw_inline_error(ui, edit, &mut action);
    }
    action
}

fn draw_phase_add_form(ui: &mut Ui, edit: &mut DetailEdit) -> Option<DetailAction> {
    let mut action = None;
    let mut cancel = false;
    let can_save;
    let title_error;
    {
        let form = edit.phase_form.as_mut()?;
        ui.add_space(6.0);
        ui.label(
            RichText::new("New phase")
                .font(TYPE.secondary_id())
                .color(HEADING)
                .strong(),
        );
        form_labelled_line(ui, "Id", |ui| {
            ui.add(
                TextEdit::singleline(&mut form.id)
                    .font(TYPE.body_id())
                    .hint_text("optional")
                    .desired_width(160.0),
            );
        });
        form_labelled_line(ui, "Title", |ui| {
            ui.add(
                TextEdit::singleline(&mut form.title)
                    .font(TYPE.body_id())
                    .desired_width(220.0),
            );
        });
        title_error = form.title_error();
        field_error(ui, title_error);
        form_labelled_line(ui, "Goal", |ui| {
            ui.add(
                TextEdit::singleline(&mut form.goal)
                    .font(TYPE.body_id())
                    .desired_width(220.0),
            );
        });
        can_save = form.can_save();
    }
    if can_save && ui.input(|input| input.key_pressed(Key::Enter)) {
        action = Some(DetailAction::SavePhase);
    }
    draw_form_actions(
        ui,
        can_save,
        &mut action,
        DetailAction::SavePhase,
        &mut cancel,
    );
    if cancel {
        edit.phase_form = None;
        edit.clear_message();
        return None;
    }
    if matches!(&edit.error_site, Some(ErrorSite::PhaseForm)) {
        draw_inline_error(ui, edit, &mut action);
    }
    action
}

fn draw_phase_edit_form(ui: &mut Ui, edit: &mut DetailEdit) -> Option<DetailAction> {
    let mut action = None;
    let mut cancel = false;
    let can_save;
    let title_error;
    {
        let (_, form) = edit.phase_edit.as_mut()?;
        ui.add_space(4.0);
        ui.label(
            RichText::new("Edit phase")
                .font(TYPE.secondary_id())
                .color(HEADING)
                .strong(),
        );
        form_labelled_line(ui, "Title", |ui| {
            ui.add(
                TextEdit::singleline(&mut form.title)
                    .font(TYPE.body_id())
                    .desired_width(220.0),
            );
        });
        title_error = form.title_error();
        field_error(ui, title_error);
        form_labelled_line(ui, "Goal", |ui| {
            ui.add(
                TextEdit::singleline(&mut form.goal)
                    .font(TYPE.body_id())
                    .desired_width(220.0),
            );
        });
        can_save = form.can_save();
    }
    if can_save && ui.input(|input| input.key_pressed(Key::Enter)) {
        action = Some(DetailAction::SavePhaseEdit);
    }
    draw_form_actions(
        ui,
        can_save,
        &mut action,
        DetailAction::SavePhaseEdit,
        &mut cancel,
    );
    if cancel {
        edit.phase_edit = None;
        edit.clear_message();
        return None;
    }
    if matches!(&edit.error_site, Some(ErrorSite::PhaseForm)) {
        draw_inline_error(ui, edit, &mut action);
    }
    action
}

fn draw_task_form(ui: &mut Ui, edit: &mut DetailEdit) -> Option<DetailAction> {
    let mut action = None;
    let mut cancel = false;
    let can_save;
    let title_error;
    let due_error;
    {
        let form = edit.task_form.as_mut()?;
        ui.add_space(6.0);
        ui.label(
            RichText::new("New task")
                .font(TYPE.secondary_id())
                .color(HEADING)
                .strong(),
        );
        form_labelled_line(ui, "Title", |ui| {
            ui.add(
                TextEdit::singleline(&mut form.title)
                    .font(TYPE.body_id())
                    .desired_width(220.0),
            );
        });
        title_error = if form.title.trim().is_empty() {
            Some("Title is required")
        } else {
            None
        };
        field_error(ui, title_error);
        form_labelled_line(ui, "Due", |ui| {
            ui.add(
                TextEdit::singleline(&mut form.due)
                    .font(TYPE.body_id())
                    .hint_text(date_hint())
                    .desired_width(120.0),
            );
        });
        due_error = form.validation_message().filter(|_| title_error.is_none());
        field_error(ui, due_error);
        can_save = form.can_save();
    }
    if can_save && ui.input(|input| input.key_pressed(Key::Enter)) {
        action = Some(DetailAction::SaveTask);
    }
    draw_form_actions(
        ui,
        can_save,
        &mut action,
        DetailAction::SaveTask,
        &mut cancel,
    );
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

fn draw_task_edit_form(ui: &mut Ui, edit: &mut DetailEdit) -> Option<DetailAction> {
    let mut action = None;
    let mut cancel = false;
    let can_save;
    let title_error;
    let due_error;
    {
        let (_, _, form) = edit.task_edit.as_mut()?;
        ui.add_space(4.0);
        ui.label(
            RichText::new("Edit task")
                .font(TYPE.secondary_id())
                .color(HEADING)
                .strong(),
        );
        form_labelled_line(ui, "Title", |ui| {
            ui.add(
                TextEdit::singleline(&mut form.title)
                    .font(TYPE.body_id())
                    .desired_width(220.0),
            );
        });
        title_error = form.title_error();
        field_error(ui, title_error);
        form_labelled_line(ui, "Due", |ui| {
            ui.add(
                TextEdit::singleline(&mut form.due)
                    .font(TYPE.body_id())
                    .hint_text(date_hint())
                    .desired_width(120.0),
            );
        });
        due_error = form.due_error();
        field_error(ui, due_error);
        form_labelled_line(ui, "Note", |ui| {
            ui.add(
                TextEdit::singleline(&mut form.note)
                    .font(TYPE.body_id())
                    .desired_width(220.0),
            );
        });
        can_save = form.can_save();
    }
    if can_save && ui.input(|input| input.key_pressed(Key::Enter)) {
        action = Some(DetailAction::SaveTaskEdit);
    }
    draw_form_actions(
        ui,
        can_save,
        &mut action,
        DetailAction::SaveTaskEdit,
        &mut cancel,
    );
    if cancel {
        edit.task_edit = None;
        edit.clear_message();
        return None;
    }
    if matches!(&edit.error_site, Some(ErrorSite::TaskEdit { .. })) {
        draw_inline_error(ui, edit, &mut action);
    }
    action
}

fn draw_issue_form(ui: &mut Ui, edit: &mut DetailEdit) -> Option<DetailAction> {
    let mut action = None;
    let mut cancel = false;
    let can_save;
    let title_error;
    let severity_error;
    let url_error;
    {
        let form = edit.issue_form.as_mut()?;
        ui.add_space(6.0);
        ui.label(
            RichText::new("New issue")
                .font(TYPE.secondary_id())
                .color(HEADING)
                .strong(),
        );
        form_labelled_line(ui, "Title", |ui| {
            ui.add(
                TextEdit::singleline(&mut form.title)
                    .font(TYPE.body_id())
                    .desired_width(220.0),
            );
        });
        title_error = if form.title.trim().is_empty() {
            Some("Title is required")
        } else {
            None
        };
        field_error(ui, title_error);
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
        severity_error = form
            .validation_message()
            .filter(|message| title_error.is_none() && message.contains("Severity"));
        field_error(ui, severity_error);
        form_labelled_line(ui, "URL", |ui| {
            ui.add(
                TextEdit::singleline(&mut form.url)
                    .font(TYPE.body_id())
                    .desired_width(220.0),
            );
        });
        url_error = form.validation_message().filter(|message| {
            title_error.is_none() && severity_error.is_none() && message.contains("URL")
        });
        field_error(ui, url_error);
        form_labelled_line(ui, "Note", |ui| {
            ui.add(
                TextEdit::singleline(&mut form.note)
                    .font(TYPE.body_id())
                    .desired_width(220.0),
            );
        });
        can_save = form.can_save();
    }
    if can_save && ui.input(|input| input.key_pressed(Key::Enter)) {
        action = Some(DetailAction::SaveIssue);
    }
    draw_form_actions(
        ui,
        can_save,
        &mut action,
        DetailAction::SaveIssue,
        &mut cancel,
    );
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

fn draw_issue_edit_form(ui: &mut Ui, edit: &mut DetailEdit) -> Option<DetailAction> {
    let mut action = None;
    let mut cancel = false;
    let can_save;
    let title_error;
    let severity_error;
    let url_error;
    {
        let (_, form) = edit.issue_edit.as_mut()?;
        ui.add_space(4.0);
        ui.label(
            RichText::new("Edit issue")
                .font(TYPE.secondary_id())
                .color(HEADING)
                .strong(),
        );
        form_labelled_line(ui, "Title", |ui| {
            ui.add(
                TextEdit::singleline(&mut form.title)
                    .font(TYPE.body_id())
                    .desired_width(220.0),
            );
        });
        title_error = form.title_error();
        field_error(ui, title_error);
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
            ComboBox::from_id_salt("edit-issue-severity")
                .selected_text(RichText::new(selected).font(TYPE.body_id()).color(TEXT))
                .show_ui(ui, |ui| {
                    for (key, label) in issue_severity_choices() {
                        ui.selectable_value(&mut form.severity, (*key).to_owned(), *label);
                    }
                });
        });
        severity_error = form.severity_error();
        field_error(ui, severity_error);
        form_labelled_line(ui, "URL", |ui| {
            ui.add(
                TextEdit::singleline(&mut form.url)
                    .font(TYPE.body_id())
                    .desired_width(220.0),
            );
        });
        url_error = form.url_error();
        field_error(ui, url_error);
        form_labelled_line(ui, "Note", |ui| {
            ui.add(
                TextEdit::singleline(&mut form.note)
                    .font(TYPE.body_id())
                    .desired_width(220.0),
            );
        });
        can_save = form.can_save();
    }
    if can_save && ui.input(|input| input.key_pressed(Key::Enter)) {
        action = Some(DetailAction::SaveIssueEdit);
    }
    draw_form_actions(
        ui,
        can_save,
        &mut action,
        DetailAction::SaveIssueEdit,
        &mut cancel,
    );
    if cancel {
        edit.issue_edit = None;
        edit.clear_message();
        return None;
    }
    if matches!(&edit.error_site, Some(ErrorSite::IssueEdit { .. })) {
        draw_inline_error(ui, edit, &mut action);
    }
    action
}

fn draw_component_form(ui: &mut Ui, edit: &mut DetailEdit) -> Option<DetailAction> {
    let mut action = None;
    let mut cancel = false;
    let can_save;
    {
        let form = edit.component_form.as_mut()?;
        ui.add_space(6.0);
        ui.label(
            RichText::new("New component")
                .font(TYPE.secondary_id())
                .color(HEADING)
                .strong(),
        );
        ui.horizontal(|ui| {
            ui.label(RichText::new("Kind").font(TYPE.secondary_id()).color(MUTED));
            let selected = component_kind_choices()
                .iter()
                .find(|(key, _)| *key == form.kind)
                .map_or(form.kind.as_str(), |(_, label)| *label)
                .to_owned();
            ComboBox::from_id_salt("new-component-kind")
                .selected_text(RichText::new(selected).font(TYPE.body_id()).color(TEXT))
                .show_ui(ui, |ui| {
                    for (key, label) in component_kind_choices() {
                        ui.selectable_value(&mut form.kind, (*key).to_owned(), *label);
                    }
                });
        });
        form_labelled_line(ui, "Name", |ui| {
            ui.add(
                TextEdit::singleline(&mut form.name)
                    .font(TYPE.body_id())
                    .desired_width(220.0),
            );
        });
        field_error(ui, form.validation_message());
        form_labelled_line(ui, "Version", |ui| {
            ui.add(
                TextEdit::singleline(&mut form.version)
                    .font(TYPE.body_id())
                    .desired_width(220.0),
            );
        });
        form_labelled_line(ui, "eol_product", |ui| {
            ui.add(
                TextEdit::singleline(&mut form.eol_product)
                    .font(TYPE.body_id())
                    .desired_width(220.0),
            )
            .on_hover_text("Product id on endoflife.date for support-end checks");
        });
        can_save = form.can_save();
    }
    if can_save && ui.input(|input| input.key_pressed(Key::Enter)) {
        action = Some(DetailAction::SaveComponent);
    }
    draw_form_actions(
        ui,
        can_save,
        &mut action,
        DetailAction::SaveComponent,
        &mut cancel,
    );
    if cancel {
        edit.component_form = None;
        edit.clear_message();
        return None;
    }
    if matches!(&edit.error_site, Some(ErrorSite::ComponentForm)) {
        draw_inline_error(ui, edit, &mut action);
    }
    action
}

fn draw_component_edit_form(ui: &mut Ui, edit: &mut DetailEdit) -> Option<DetailAction> {
    let mut action = None;
    let mut cancel = false;
    let can_save;
    let name;
    {
        let (component_name, form) = edit.component_edit.as_mut()?;
        name = component_name.clone();
        ui.add_space(6.0);
        ui.label(
            RichText::new(format!("Edit {name}"))
                .font(TYPE.secondary_id())
                .color(HEADING)
                .strong(),
        );
        form_labelled_line(ui, "Version", |ui| {
            ui.add(
                TextEdit::singleline(&mut form.version)
                    .font(TYPE.body_id())
                    .desired_width(220.0),
            );
        });
        form_labelled_line(ui, "Note", |ui| {
            ui.add(
                TextEdit::singleline(&mut form.note)
                    .font(TYPE.body_id())
                    .desired_width(220.0),
            );
        });
        form_labelled_line(ui, "eol_product", |ui| {
            ui.add(
                TextEdit::singleline(&mut form.eol_product)
                    .font(TYPE.body_id())
                    .desired_width(220.0),
            )
            .on_hover_text("Product id on endoflife.date for support-end checks");
        });
        can_save = ComponentEditForm::can_save();
    }
    let _ = name;
    if can_save && ui.input(|input| input.key_pressed(Key::Enter)) {
        action = Some(DetailAction::SaveComponentEdit);
    }
    draw_form_actions(
        ui,
        can_save,
        &mut action,
        DetailAction::SaveComponentEdit,
        &mut cancel,
    );
    if cancel {
        edit.component_edit = None;
        edit.clear_message();
        return None;
    }
    if matches!(&edit.error_site, Some(ErrorSite::ComponentForm)) {
        draw_inline_error(ui, edit, &mut action);
    }
    action
}

fn draw_lockfile_form(
    ui: &mut Ui,
    detail: &ProjectDetail,
    edit: &mut DetailEdit,
) -> Option<DetailAction> {
    let mut action = None;
    let mut cancel = false;
    let can_save;
    {
        let form = edit.lockfile_form.as_mut()?;
        ui.add_space(6.0);
        ui.label(
            RichText::new("Add lockfile")
                .font(TYPE.secondary_id())
                .color(HEADING)
                .strong(),
        );
        if detail.source_dir.is_none() {
            ui.label(
                RichText::new("source_dir is not set; paths resolve from the project file folder")
                    .font(TYPE.secondary_id())
                    .color(MUTED),
            );
        }
        form_labelled_line(ui, "Path", |ui| {
            ui.add(
                TextEdit::singleline(&mut form.path)
                    .font(TYPE.body_id())
                    .desired_width(200.0),
            );
            if compact_button(ui, "Browse…", MUTED).clicked() {
                action = Some(DetailAction::BrowseLockfile);
            }
        });
        let project_file = std::path::Path::new(&detail.path);
        let resolved = match detail.source_dir.as_deref() {
            Some(stored) if craftward_core::is_portable_absolute(stored) => {
                std::path::PathBuf::from(stored)
            }
            Some(stored) => project_file.parent().unwrap_or(project_file).join(stored),
            None => project_file.parent().map_or_else(
                || std::path::PathBuf::from("."),
                std::path::Path::to_path_buf,
            ),
        };
        field_error(ui, form.validation_message(&resolved));
        can_save = form.can_save(&resolved);
    }
    if can_save && ui.input(|input| input.key_pressed(Key::Enter)) {
        action = Some(DetailAction::SaveLockfile);
    }
    draw_form_actions(
        ui,
        can_save,
        &mut action,
        DetailAction::SaveLockfile,
        &mut cancel,
    );
    if cancel {
        edit.lockfile_form = None;
        edit.clear_message();
        return None;
    }
    if matches!(&edit.error_site, Some(ErrorSite::LockfileForm)) {
        draw_inline_error(ui, edit, &mut action);
    }
    action
}

fn draw_android_form(ui: &mut Ui, edit: &mut DetailEdit) -> Option<DetailAction> {
    let mut action = None;
    let mut cancel = false;
    let can_save;
    {
        let form = edit.android_form.as_mut()?;
        ui.add_space(6.0);
        ui.label(
            RichText::new("Android")
                .font(TYPE.secondary_id())
                .color(HEADING)
                .strong(),
        );
        form_labelled_line(ui, "target_sdk", |ui| {
            ui.add(
                TextEdit::singleline(&mut form.target_sdk)
                    .font(TYPE.body_id())
                    .desired_width(100.0),
            );
        });
        form_labelled_line(ui, "min_sdk", |ui| {
            ui.add(
                TextEdit::singleline(&mut form.min_sdk)
                    .font(TYPE.body_id())
                    .desired_width(100.0),
            );
        });
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Form factor")
                    .font(TYPE.secondary_id())
                    .color(MUTED),
            );
            let selected = form_factor_choices()
                .iter()
                .find(|(key, _)| *key == form.form_factor)
                .map_or(form.form_factor.as_str(), |(_, label)| *label)
                .to_owned();
            ComboBox::from_id_salt("android-form-factor")
                .selected_text(RichText::new(selected).font(TYPE.body_id()).color(TEXT))
                .show_ui(ui, |ui| {
                    for (key, label) in form_factor_choices() {
                        ui.selectable_value(&mut form.form_factor, (*key).to_owned(), *label);
                    }
                });
        });
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Store")
                    .font(TYPE.secondary_id())
                    .color(MUTED),
            );
            let selected = android_store_choices()
                .iter()
                .find(|(key, _)| *key == form.store)
                .map_or(form.store.as_str(), |(_, label)| *label)
                .to_owned();
            ComboBox::from_id_salt("android-store")
                .selected_text(RichText::new(selected).font(TYPE.body_id()).color(TEXT))
                .show_ui(ui, |ui| {
                    for (key, label) in android_store_choices() {
                        ui.selectable_value(&mut form.store, (*key).to_owned(), *label);
                    }
                });
        });
        ui.label(
            RichText::new("detect_from (one path per line)")
                .font(TYPE.secondary_id())
                .color(MUTED),
        );
        ui.add(
            TextEdit::multiline(&mut form.detect_from)
                .font(TYPE.body_id())
                .desired_width(260.0)
                .desired_rows(3),
        );
        field_error(ui, form.validation_message());
        can_save = form.can_save();
    }
    if can_save && ui.input(|input| input.key_pressed(Key::Enter)) {
        action = Some(DetailAction::SaveAndroid);
    }
    draw_form_actions(
        ui,
        can_save,
        &mut action,
        DetailAction::SaveAndroid,
        &mut cancel,
    );
    if cancel {
        edit.android_form = None;
        edit.clear_message();
        return None;
    }
    if matches!(&edit.error_site, Some(ErrorSite::AndroidForm)) {
        draw_inline_error(ui, edit, &mut action);
    }
    action
}

fn draw_watch_form(ui: &mut Ui, edit: &mut DetailEdit) -> Option<DetailAction> {
    let mut action = None;
    let mut cancel = false;
    let can_save;
    {
        let form = edit.watch_form.as_mut()?;
        ui.add_space(6.0);
        ui.label(
            RichText::new("New watch item")
                .font(TYPE.secondary_id())
                .color(HEADING)
                .strong(),
        );
        ui.horizontal(|ui| {
            ui.label(RichText::new("Kind").font(TYPE.secondary_id()).color(MUTED));
            let selected = watch_kind_choices()
                .iter()
                .find(|(key, _)| *key == form.kind)
                .map_or(form.kind.as_str(), |(_, label)| *label)
                .to_owned();
            ComboBox::from_id_salt("new-watch-kind")
                .selected_text(RichText::new(selected).font(TYPE.body_id()).color(TEXT))
                .show_ui(ui, |ui| {
                    for (key, label) in watch_kind_choices() {
                        ui.selectable_value(&mut form.kind, (*key).to_owned(), *label);
                    }
                });
        });
        form_labelled_line(ui, "Title", |ui| {
            ui.add(
                TextEdit::singleline(&mut form.title)
                    .font(TYPE.body_id())
                    .desired_width(220.0),
            );
        });
        form_labelled_line(ui, "Due", |ui| {
            ui.add(
                TextEdit::singleline(&mut form.due)
                    .font(TYPE.body_id())
                    .desired_width(120.0)
                    .hint_text(date_hint()),
            );
        });
        form_labelled_line(ui, "Note", |ui| {
            ui.add(
                TextEdit::singleline(&mut form.note)
                    .font(TYPE.body_id())
                    .desired_width(220.0),
            );
        });
        field_error(ui, form.validation_message());
        can_save = form.can_save();
    }
    if can_save && ui.input(|input| input.key_pressed(Key::Enter)) {
        action = Some(DetailAction::SaveWatch);
    }
    draw_form_actions(
        ui,
        can_save,
        &mut action,
        DetailAction::SaveWatch,
        &mut cancel,
    );
    if cancel {
        edit.watch_form = None;
        edit.clear_message();
        return None;
    }
    if matches!(&edit.error_site, Some(ErrorSite::WatchForm)) {
        draw_inline_error(ui, edit, &mut action);
    }
    action
}

fn draw_watch_edit_form(ui: &mut Ui, edit: &mut DetailEdit) -> Option<DetailAction> {
    let mut action = None;
    let mut cancel = false;
    let can_save;
    {
        let (_id, form) = edit.watch_edit.as_mut()?;
        ui.add_space(6.0);
        ui.label(
            RichText::new("Edit watch item")
                .font(TYPE.secondary_id())
                .color(HEADING)
                .strong(),
        );
        ui.horizontal(|ui| {
            ui.label(RichText::new("Kind").font(TYPE.secondary_id()).color(MUTED));
            let selected = watch_kind_choices()
                .iter()
                .find(|(key, _)| *key == form.kind)
                .map_or(form.kind.as_str(), |(_, label)| *label)
                .to_owned();
            ComboBox::from_id_salt("edit-watch-kind")
                .selected_text(RichText::new(selected).font(TYPE.body_id()).color(TEXT))
                .show_ui(ui, |ui| {
                    for (key, label) in watch_kind_choices() {
                        ui.selectable_value(&mut form.kind, (*key).to_owned(), *label);
                    }
                });
        });
        form_labelled_line(ui, "Title", |ui| {
            ui.add(
                TextEdit::singleline(&mut form.title)
                    .font(TYPE.body_id())
                    .desired_width(220.0),
            );
        });
        form_labelled_line(ui, "Due", |ui| {
            ui.add(
                TextEdit::singleline(&mut form.due)
                    .font(TYPE.body_id())
                    .desired_width(120.0)
                    .hint_text(date_hint()),
            );
        });
        form_labelled_line(ui, "Note", |ui| {
            ui.add(
                TextEdit::singleline(&mut form.note)
                    .font(TYPE.body_id())
                    .desired_width(220.0),
            );
        });
        field_error(ui, form.validation_message());
        can_save = form.can_save();
    }
    if can_save && ui.input(|input| input.key_pressed(Key::Enter)) {
        action = Some(DetailAction::SaveWatchEdit);
    }
    draw_form_actions(
        ui,
        can_save,
        &mut action,
        DetailAction::SaveWatchEdit,
        &mut cancel,
    );
    if cancel {
        edit.watch_edit = None;
        edit.clear_message();
        return None;
    }
    if matches!(&edit.error_site, Some(ErrorSite::WatchEdit { .. })) {
        draw_inline_error(ui, edit, &mut action);
    }
    action
}

fn form_labelled_line(ui: &mut Ui, label: &str, add_contents: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).font(TYPE.secondary_id()).color(MUTED));
        add_contents(ui);
    });
}

fn field_error(ui: &mut Ui, message: Option<&str>) {
    if let Some(message) = message {
        ui.colored_label(SEVERITY_CRITICAL, message);
    }
}

/// Button that stays content-sized even inside a justified column.
fn compact_button(ui: &mut Ui, label: &str, colour: Color32) -> egui::Response {
    let mut response = None;
    ui.horizontal(|ui| {
        response = Some(ui.add(Button::new(
            RichText::new(label).font(TYPE.secondary_id()).color(colour),
        )));
    });
    response.expect("horizontal always runs")
}

fn draw_form_actions(
    ui: &mut Ui,
    can_save: bool,
    action: &mut Option<DetailAction>,
    save: DetailAction,
    cancel: &mut bool,
) {
    ui.horizontal(|ui| {
        if ui
            .add_enabled(
                can_save,
                Button::new(RichText::new("Save").font(TYPE.body_id()).color(ACCENT)),
            )
            .clicked()
        {
            *action = Some(save);
        }
        if ui
            .add(Button::new(
                RichText::new("Cancel").font(TYPE.body_id()).color(MUTED),
            ))
            .clicked()
        {
            *cancel = true;
        }
    });
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
    // The chip itself is the control surface (no nested frame around a button).
    ui.add_sized(
        Vec2::new(fixed_width, ui.spacing().interact_size.y),
        Button::new(
            RichText::new(label)
                .font(TYPE.secondary_id())
                .color(colour)
                .strong(),
        )
        .fill(CARD_INNER)
        .stroke(Stroke::new(1.0, LINE))
        .corner_radius(CornerRadius::same(6))
        .sense(Sense::hover()),
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
            Vec2::new(fixed_width, ui.spacing().interact_size.y),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.set_min_width(fixed_width);
                ui.set_max_width(fixed_width);
                // Badge *is* the menu button — no outer frame (avoids a double border).
                let response = ui
                    .menu_button(
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
                    .response;
                let width = response.rect.width();
                if width < fixed_width {
                    ui.add_space(fixed_width - width);
                }
                response.on_hover_text(format!("{label} — change status"));
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
        "open" | "idea" | "unknown" | "notice" => OPEN_TEXT,
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
