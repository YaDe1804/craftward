//! First-run setup wizard for the desktop app.

use eframe::egui::{Button, Context, RichText, TextEdit, Ui, ViewportCommand};

use crate::picker::{PathPicker, PickKind, PickerInbox};
use crate::storage::{OnboardingDefaults, OnboardingDraft, finish_onboarding};
use crate::theme::{ACCENT, HEADING, MUTED, TEXT, TYPE};

/// Wizard step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnboardingStep {
    /// Short welcome.
    Welcome,
    /// Storage locations.
    Paths,
    /// Operating mode (Local only for now).
    Mode,
    /// Create files and finish.
    Finish,
}

/// Mutable first-run state.
#[derive(Debug, Clone)]
pub struct OnboardingState {
    /// Current step.
    pub step: OnboardingStep,
    /// Draft paths.
    pub draft: OnboardingDraft,
    /// Whether env vars override the chosen paths.
    pub workspace_env: bool,
    pub data_env: bool,
    pub cache_env: bool,
    /// Last error message.
    pub error: Option<String>,
}

impl OnboardingState {
    /// Builds a wizard from platform defaults.
    pub fn start() -> Result<Self, String> {
        let defaults = OnboardingDefaults::load()?;
        Ok(Self {
            step: OnboardingStep::Welcome,
            draft: OnboardingDraft::from_defaults(&defaults),
            workspace_env: defaults.workspace_env,
            data_env: defaults.data_env,
            cache_env: defaults.cache_env,
            error: None,
        })
    }
}

/// Outcome of one frame of the wizard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnboardingAction {
    /// Keep showing the wizard.
    Continue,
    /// Setup finished; show the portfolio with a tip.
    Finished,
}

/// Draws the wizard and returns when setup completes.
pub fn draw_onboarding(
    ui: &mut Ui,
    state: &mut OnboardingState,
    picker: &dyn PathPicker,
    inbox: &PickerInbox,
) -> OnboardingAction {
    ui.label(
        RichText::new("Welcome to Craftward")
            .font(TYPE.title_id())
            .color(HEADING)
            .strong(),
    );
    ui.add_space(12.0);

    match state.step {
        OnboardingStep::Welcome => draw_welcome(ui, state),
        OnboardingStep::Paths => draw_paths(ui, state, picker, inbox),
        OnboardingStep::Mode => draw_mode(ui, state),
        OnboardingStep::Finish => draw_finish(ui, state),
    }
}

fn draw_welcome(ui: &mut Ui, state: &mut OnboardingState) -> OnboardingAction {
    ui.label(
        RichText::new(
            "Craftward keeps a local portfolio of your projects: plans, issues, \
             watch items, and a maintenance radar for vulnerabilities and support ends. \
             Nothing leaves this computer unless you run a network check.",
        )
        .font(TYPE.body_id())
        .color(TEXT),
    );
    ui.add_space(16.0);
    footer_nav(ui, state, false, true, |state| {
        state.step = OnboardingStep::Paths;
    });
    OnboardingAction::Continue
}

fn draw_paths(
    ui: &mut Ui,
    state: &mut OnboardingState,
    picker: &dyn PathPicker,
    inbox: &PickerInbox,
) -> OnboardingAction {
    ui.label(
        RichText::new("Choose where Craftward stores its files.")
            .font(TYPE.body_id())
            .color(TEXT),
    );
    ui.add_space(10.0);

    let ctx = ui.ctx().clone();
    path_row(
        ui,
        "Workspace file",
        "The list of projects (workspace.json).",
        &mut state.draft.workspace,
        state.workspace_env,
        "CRAFTWARD_WORKSPACE",
        || picker.pick_workspace_folder(ctx.clone(), inbox.sender()),
    );
    ui.add_space(8.0);
    path_row(
        ui,
        "Data folder",
        "The findings database (craftward.db) and related data.",
        &mut state.draft.data_dir,
        state.data_env,
        "CRAFTWARD_DATA_DIR",
        || picker.pick_data_folder(ctx.clone(), inbox.sender()),
    );
    ui.add_space(8.0);
    path_row(
        ui,
        "Cache folder",
        "Cached HTTP responses from OSV and endoflife.date.",
        &mut state.draft.cache_dir,
        state.cache_env,
        "CRAFTWARD_CACHE_DIR",
        || picker.pick_cache_folder(ctx, inbox.sender()),
    );

    ui.add_space(16.0);
    footer_nav(ui, state, true, true, |state| {
        state.step = OnboardingStep::Mode;
    });
    OnboardingAction::Continue
}

fn path_row(
    ui: &mut Ui,
    title: &str,
    help: &str,
    value: &mut String,
    env_set: bool,
    env_name: &str,
    browse: impl FnOnce(),
) {
    ui.label(
        RichText::new(title)
            .font(TYPE.section_id())
            .color(HEADING)
            .strong(),
    );
    ui.label(RichText::new(help).font(TYPE.secondary_id()).color(MUTED));
    if env_set {
        ui.label(
            RichText::new(format!(
                "{env_name} is set and takes precedence over this choice until it is unset."
            ))
            .font(TYPE.secondary_id())
            .color(MUTED),
        );
    }
    ui.horizontal(|ui| {
        ui.add(
            TextEdit::singleline(value)
                .desired_width(420.0)
                .font(TYPE.body_id()),
        );
        if ui
            .add(Button::new(
                RichText::new("Browse…").font(TYPE.body_id()).color(ACCENT),
            ))
            .clicked()
        {
            browse();
        }
    });
}

fn draw_mode(ui: &mut Ui, state: &mut OnboardingState) -> OnboardingAction {
    ui.label(
        RichText::new("Operating mode")
            .font(TYPE.section_id())
            .color(HEADING)
            .strong(),
    );
    ui.add_space(6.0);
    ui.label(
        RichText::new("Local")
            .font(TYPE.body_id())
            .color(TEXT)
            .strong(),
    );
    ui.label(
        RichText::new(
            "Projects and the database stay on this computer. A shared team folder \
             is not offered yet.",
        )
        .font(TYPE.secondary_id())
        .color(MUTED),
    );
    ui.add_space(16.0);
    footer_nav(ui, state, true, true, |state| {
        state.step = OnboardingStep::Finish;
    });
    OnboardingAction::Continue
}

fn draw_finish(ui: &mut Ui, state: &mut OnboardingState) -> OnboardingAction {
    ui.label(
        RichText::new(
            "Craftward will create the folders, an empty workspace file, and a \
             settings file with these locations.",
        )
        .font(TYPE.body_id())
        .color(TEXT),
    );
    if let Some(error) = &state.error {
        ui.add_space(8.0);
        ui.colored_label(crate::theme::SEVERITY_CRITICAL, error);
    }
    ui.add_space(16.0);
    let mut finished = false;
    ui.horizontal(|ui| {
        if ui
            .add(Button::new(
                RichText::new("Back").font(TYPE.body_id()).color(MUTED),
            ))
            .clicked()
        {
            state.error = None;
            state.step = OnboardingStep::Mode;
        }
        if ui
            .add(Button::new(
                RichText::new("Cancel").font(TYPE.body_id()).color(MUTED),
            ))
            .clicked()
        {
            request_close(ui.ctx());
        }
        if ui
            .add(Button::new(
                RichText::new("Finish").font(TYPE.body_id()).color(ACCENT),
            ))
            .clicked()
        {
            match finish_onboarding(&state.draft) {
                Ok(()) => finished = true,
                Err(error) => state.error = Some(error),
            }
        }
    });
    if finished {
        OnboardingAction::Finished
    } else {
        OnboardingAction::Continue
    }
}

fn footer_nav(
    ui: &mut Ui,
    state: &mut OnboardingState,
    back: bool,
    next: bool,
    on_next: impl FnOnce(&mut OnboardingState),
) {
    ui.horizontal(|ui| {
        if back
            && ui
                .add(Button::new(
                    RichText::new("Back").font(TYPE.body_id()).color(MUTED),
                ))
                .clicked()
        {
            state.step = match state.step {
                OnboardingStep::Mode => OnboardingStep::Paths,
                OnboardingStep::Finish => OnboardingStep::Mode,
                OnboardingStep::Paths | OnboardingStep::Welcome => OnboardingStep::Welcome,
            };
        }
        if ui
            .add(Button::new(
                RichText::new("Cancel").font(TYPE.body_id()).color(MUTED),
            ))
            .clicked()
        {
            request_close(ui.ctx());
        }
        if next
            && ui
                .add(Button::new(
                    RichText::new("Next").font(TYPE.body_id()).color(ACCENT),
                ))
                .clicked()
        {
            on_next(state);
        }
    });
}

fn request_close(ctx: &Context) {
    ctx.send_viewport_cmd(ViewportCommand::Close);
}

/// Applies a folder pick to the onboarding draft.
pub fn apply_pick(state: &mut OnboardingState, kind: PickKind, path: &std::path::Path) {
    let text = path.display().to_string();
    match kind {
        PickKind::WorkspaceFolder => {
            state.draft.workspace = path.join("workspace.json").display().to_string();
        }
        PickKind::DataFolder => state.draft.data_dir = text,
        PickKind::CacheFolder => state.draft.cache_dir = text,
        _ => {}
    }
}
