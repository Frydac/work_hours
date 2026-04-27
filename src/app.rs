#![allow(dead_code)]

// App composition root. This wires together the pure week/day state, the
// Supabase sync state, the async result queue, and the top-level egui shell.

mod state;
mod sync;
mod tasks;
mod ui_shell;
mod ui_state;

use crate::config::AppConfig;
use state::State;
use sync::{AsyncResult, SyncState};
use tasks::{new_async_results, take_async_results, AsyncResults};
use tracing::{debug, info, warn};
use ui_state::AppUiState;

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct TemplateApp {
    pub state: State,
    pub undoer: egui::util::undoer::Undoer<State>,
    pub sync: SyncState,
    pub ui_state: AppUiState,
    #[serde(skip)]
    config: Option<AppConfig>,
    #[serde(skip)]
    async_results: AsyncResults<AsyncResult>,
}

impl Default for TemplateApp {
    fn default() -> Self {
        Self {
            state: State::default(),
            undoer: Default::default(),
            sync: Default::default(),
            ui_state: Default::default(),
            config: None,
            async_results: new_async_results(),
        }
    }
}

impl TemplateApp {
    /// Restores persisted state, rebuilds non-persisted runtime helpers, and
    /// kicks off session bootstrap if a Supabase session was saved earlier.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        debug!(target = "app", has_storage = cc.storage.is_some(), "building TemplateApp");
        let mut app: Self = if let Some(storage) = cc.storage {
            let mut app: Self = eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
            app.state.populate_missing_dates();
            app
        } else {
            Default::default()
        };

        app.async_results = new_async_results();
        app.config = AppConfig::load_public().ok();
        info!(
            target = "app",
            has_config = app.config.is_some(),
            has_stored_session = app.sync.stored_session.is_some(),
            "initialized runtime app state"
        );
        app.ui_state.adopt_stored_session_email(app.sync.stored_session.as_ref());
        app.sync.initialize_session(
            &app.state,
            &mut app.ui_state,
            app.config.as_ref(),
            &app.async_results,
            cc.egui_ctx.clone(),
        );
        app
    }

    fn process_async_results(&mut self, ctx: &egui::Context) {
        let results = take_async_results(&self.async_results);
        if !results.is_empty() {
            debug!(target = "app", count = results.len(), "processing async results");
        }
        self.sync.process_async_results(
            results,
            &mut self.state,
            &mut self.undoer,
            &mut self.ui_state,
            self.config.as_ref(),
            &self.async_results,
            ctx,
        );
    }

    fn reset_state(&mut self) {
        self.state = State::default();
        self.undoer = Default::default();
        self.sync.clear_synced_week();
    }

    pub fn duration(&self) -> time::Duration {
        self.state.duration()
    }

    pub fn total_target(&self) -> time::Duration {
        self.state.total_target()
    }

    fn start_login(&mut self, ctx: egui::Context) {
        self.sync
            .start_login(&mut self.ui_state, self.config.as_ref(), &self.async_results, ctx);
    }

    fn request_visible_week_load(&mut self, ctx: egui::Context) {
        self.sync
            .request_visible_week_load(&self.state, &mut self.ui_state, self.config.as_ref(), &self.async_results, ctx);
    }

    fn save_visible_week(&mut self, ctx: egui::Context) {
        self.sync
            .save_visible_week(&self.state, &mut self.ui_state, self.config.as_ref(), &self.async_results, ctx);
    }

    fn logout(&mut self) {
        self.sync.logout(&mut self.ui_state);
    }

    fn navigate_to_week(&mut self, ctx: egui::Context, year: i32, week_nr: i32) {
        if !self.sync.can_change_week(&self.state) {
            warn!(
                target = "sync",
                year,
                week_nr,
                current_week = self.state.current_week_key().week_nr,
                "blocked week navigation because current week cannot change"
            );
            self.ui_state
                .set_error_message("Save the current week before changing weeks.".to_string());
            return;
        }

        self.ui_state.clear_error();
        debug!(target = "sync", year, week_nr, "navigating to week");
        self.state.set_current_week_normalized(year, week_nr);
        self.sync.clear_synced_week();
        if self.sync.is_logged_in() {
            self.request_visible_week_load(ctx);
        }
    }
}

impl eframe::App for TemplateApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.state.save_current_week();
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.process_async_results(ctx);
        ui_shell::render(self, ctx, frame);
    }
}
