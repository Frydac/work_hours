use std::collections::HashMap;

// Supabase session and synchronization state. This file owns login/session
// lifecycle, dirty tracking, background load/save orchestration, and the
// translation between async results and app state updates.

use crate::config::AppConfig;
use crate::supabase::{StoredSession, SupabaseClient, WorkDayDraft};
use crate::ui;
use anyhow::{Context, Error, Result};
use tracing::{debug, info, warn};

use super::state::{State, WeekKey};
use super::tasks::{spawn_async_task, AsyncResults};
use super::ui_state::AppUiState;

pub(crate) struct ProcessAsyncContext<'a> {
    pub state: &'a mut State,
    pub undoer: &'a mut egui::util::undoer::Undoer<State>,
    pub ui_state: &'a mut AppUiState,
    pub config: Option<&'a AppConfig>,
    pub async_results: &'a AsyncResults<AsyncResult>,
    pub ctx: &'a egui::Context,
}

#[derive(Debug, Clone)]
struct WeekSyncSnapshot {
    week: WeekKey,
    drafts: Vec<WorkDayDraft>,
}

#[derive(Debug, Default)]
struct InFlightOps {
    auth: bool,
    load_week: Option<WeekKey>,
    save_week: Option<WeekKey>,
}

#[derive(Debug)]
pub(crate) enum AsyncResult {
    Login(Result<StoredSession, Error>),
    RefreshSession(Result<StoredSession, Error>),
    LoadWeek {
        week: WeekKey,
        result: Result<Vec<WorkDayDraft>, String>,
    },
    SaveWeek {
        week: WeekKey,
        result: Result<Vec<WorkDayDraft>, String>,
    },
}

#[derive(serde::Deserialize, serde::Serialize, Default)]
#[serde(default)]
pub struct SyncState {
    pub stored_session: Option<StoredSession>,
    #[serde(skip)]
    synced_week: Option<WeekSyncSnapshot>,
    #[serde(skip)]
    in_flight: InFlightOps,
}

impl SyncState {
    pub(crate) fn initialize_session(
        &mut self,
        state: &State,
        ui_state: &mut AppUiState,
        config: Option<&AppConfig>,
        async_results: &AsyncResults<AsyncResult>,
        ctx: egui::Context,
    ) {
        if self.stored_session.is_none() {
            debug!(target = "sync", "no stored session to initialize");
            return;
        }

        if config.is_none() {
            warn!(target = "sync", "stored session exists but public config is missing");
            ui_state.set_error_message("Supabase config missing; login is unavailable.".to_string());
            return;
        }

        let session = self.stored_session.clone().unwrap();
        info!(
            target = "sync",
            user_id = %session.user_id,
            email = session.email.as_deref().unwrap_or("unknown"),
            has_refresh_token = !session.refresh_token.is_empty(),
            "initializing persisted session"
        );
        if session.is_expired_or_near_expiry(chrono::Utc::now().timestamp()) {
            self.start_refresh_session(ui_state, config, async_results, ctx, session.refresh_token);
        } else {
            // A still-valid saved session lets the app come back online without
            // asking the user to log in again.
            ui_state.set_status_message(format!("Logged in as {}", self.session_label()));
            self.request_visible_week_load(state, ui_state, config, async_results, ctx);
        }
    }

    pub(crate) fn clear_synced_week(&mut self) {
        self.synced_week = None;
    }

    pub(crate) fn is_logged_in(&self) -> bool {
        self.stored_session.is_some()
    }

    pub(crate) fn is_busy(&self) -> bool {
        self.in_flight.auth || self.in_flight.load_week.is_some() || self.in_flight.save_week.is_some()
    }

    pub(crate) fn session_label(&self) -> String {
        self.stored_session
            .as_ref()
            .and_then(|session| session.email.clone())
            .unwrap_or_else(|| {
                self.stored_session
                    .as_ref()
                    .map(|session| session.user_id.clone())
                    .unwrap_or_else(|| "unknown user".to_string())
            })
    }

    fn current_week_drafts(&self, state: &State) -> Result<Vec<WorkDayDraft>> {
        state.days().iter().map(WorkDayDraft::from_ui_day).collect()
    }

    pub(crate) fn is_week_dirty(&self, state: &State) -> bool {
        if !self.is_logged_in() || self.in_flight.load_week.is_some() || self.in_flight.save_week.is_some() {
            return false;
        }

        match self.current_week_drafts(state) {
            // Compare against the last successful server snapshot rather than
            // trying to infer dirtiness from UI events alone.
            Ok(current) => match &self.synced_week {
                Some(snapshot) if snapshot.week == state.current_week_key() => current != snapshot.drafts,
                _ => true,
            },
            Err(_) => false,
        }
    }

    pub(crate) fn can_change_week(&self, state: &State) -> bool {
        !self.in_flight.auth
            && self.in_flight.load_week.is_none()
            && self.in_flight.save_week.is_none()
            && !(self.is_logged_in() && self.is_week_dirty(state))
    }

    pub(crate) fn can_refresh_week(&self, state: &State) -> bool {
        self.is_logged_in() && self.in_flight.load_week.is_none() && self.in_flight.save_week.is_none() && !self.is_week_dirty(state)
    }

    pub(crate) fn config_available(&self, config: Option<&AppConfig>) -> bool {
        config.is_some()
    }

    pub(crate) fn in_flight_auth(&self) -> bool {
        self.in_flight.auth
    }

    pub(crate) fn in_flight_save_week(&self) -> Option<WeekKey> {
        self.in_flight.save_week
    }

    pub(crate) fn start_login(
        &mut self,
        ui_state: &mut AppUiState,
        config: Option<&AppConfig>,
        async_results: &AsyncResults<AsyncResult>,
        ctx: egui::Context,
    ) {
        if self.in_flight.auth {
            debug!(target = "sync", "ignoring login request while auth is already in flight");
            return;
        }

        let Some(config) = self.require_config(ui_state, config, "Supabase config missing; login is unavailable.") else {
            return;
        };

        let email = ui_state.login_email().trim().to_string();
        let password = ui_state.login_password().to_string();
        if email.is_empty() || password.is_empty() {
            ui_state.set_error_message("Enter both email and password.".to_string());
            return;
        }

        info!(target = "auth", %email, "starting password login");
        ui_state.clear_error();
        self.in_flight.auth = true;
        ui_state.set_status_message("Logging in...".to_string());
        let results = async_results.clone();
        spawn_async_task(ctx, results, async move {
            let client = supabase_client(&config);
            AsyncResult::Login(
                client
                    .sign_in_password(&email, &password)
                    .await
                    .map(StoredSession::from),
            )
        });
    }

    fn start_refresh_session(
        &mut self,
        ui_state: &mut AppUiState,
        config: Option<&AppConfig>,
        async_results: &AsyncResults<AsyncResult>,
        ctx: egui::Context,
        refresh_token: String,
    ) {
        if self.in_flight.auth {
            debug!(target = "auth", "ignoring refresh request while auth is already in flight");
            return;
        }

        let Some(config) = self.require_config(ui_state, config, "Supabase config missing; session refresh is unavailable.") else {
            return;
        };

        self.in_flight.auth = true;
        info!(target = "auth", "refreshing persisted session");
        ui_state.set_status_message("Refreshing session...".to_string());
        let results = async_results.clone();
        spawn_async_task(ctx, results, async move {
            let client = supabase_client(&config);
            AsyncResult::RefreshSession(
                client
                    .refresh_session(&refresh_token)
                    .await
                    .map(StoredSession::from),
            )
        });
    }

    pub(crate) fn request_visible_week_load(
        &mut self,
        state: &State,
        ui_state: &mut AppUiState,
        config: Option<&AppConfig>,
        async_results: &AsyncResults<AsyncResult>,
        ctx: egui::Context,
    ) {
        if !self.is_logged_in() || self.in_flight.load_week.is_some() || self.in_flight.save_week.is_some() {
            debug!(
                target = "sync",
                is_logged_in = self.is_logged_in(),
                has_in_flight_load = self.in_flight.load_week.is_some(),
                has_in_flight_save = self.in_flight.save_week.is_some(),
                "skipping week load request"
            );
            return;
        }

        let Some(config) = self.require_config(ui_state, config, "Supabase config missing; loading is unavailable.") else {
            return;
        };
        let Some(session) = self.stored_session.clone() else {
            return;
        };

        let week = state.current_week_key();
        let (start_date, end_date) = state.current_week_range();
        info!(
            target = "sync",
            year = week.year,
            week = week.week_nr,
            %start_date,
            %end_date,
            "requesting visible week load"
        );
        ui_state.clear_error();
        self.in_flight.load_week = Some(week);
        ui_state.set_status_message(format!("Loading week {}...", week.week_nr));
        let results = async_results.clone();
        spawn_async_task(ctx, results, async move {
            let client = supabase_client(&config);
            let result = client
                .get_work_days_range(&session.access_token, start_date, end_date)
                .await
                .map(|days| days.into_iter().map(WorkDayDraft::from).collect())
                .map_err(|err| err.to_string());
            AsyncResult::LoadWeek { week, result }
        });
    }

    pub(crate) fn save_visible_week(
        &mut self,
        state: &State,
        ui_state: &mut AppUiState,
        config: Option<&AppConfig>,
        async_results: &AsyncResults<AsyncResult>,
        ctx: egui::Context,
    ) {
        if !self.is_logged_in() || self.in_flight.save_week.is_some() || self.in_flight.load_week.is_some() {
            debug!(
                target = "sync",
                is_logged_in = self.is_logged_in(),
                has_in_flight_save = self.in_flight.save_week.is_some(),
                has_in_flight_load = self.in_flight.load_week.is_some(),
                "skipping week save request"
            );
            return;
        }

        let Some(config) = self.require_config(ui_state, config, "Supabase config missing; saving is unavailable.") else {
            return;
        };
        let Some(session) = self.stored_session.clone() else {
            return;
        };

        let week = state.current_week_key();
        let drafts = match self.current_week_drafts(state) {
            Ok(drafts) => drafts,
            Err(err) => {
                warn!(target = "sync", error = %err, year = week.year, week = week.week_nr, "failed to build save drafts");
                ui_state.set_error_message(err.to_string());
                return;
            }
        };

        let total_entries: usize = drafts.iter().map(|draft| draft.work_entries.len()).sum();
        info!(
            target = "sync",
            year = week.year,
            week = week.week_nr,
            day_count = drafts.len(),
            total_entries,
            "saving visible week"
        );
        ui_state.clear_error();
        self.in_flight.save_week = Some(week);
        ui_state.set_status_message(format!("Saving week {}...", week.week_nr));
        let results = async_results.clone();
        spawn_async_task(ctx, results, async move {
            let client = supabase_client(&config);
            let result = save_week_drafts(&client, &session.access_token, drafts)
                .await
                .map_err(|err| err.to_string());
            AsyncResult::SaveWeek { week, result }
        });
    }

    pub(crate) fn logout(&mut self, ui_state: &mut AppUiState) {
        info!(
            target = "auth",
            was_logged_in = self.stored_session.is_some(),
            "logging out and clearing stored session"
        );
        self.stored_session = None;
        self.synced_week = None;
        ui_state.clear_login_password();
        ui_state.set_status_message("Logged out.".to_string());
        ui_state.clear_error();
        ui_state.set_show_login_window(false);
    }

    fn apply_loaded_drafts(&mut self, state: &mut State, drafts: Vec<WorkDayDraft>) -> Result<Vec<WorkDayDraft>> {
        debug!(target = "sync", server_day_count = drafts.len(), "applying loaded week drafts");
        let mut by_date = HashMap::new();
        for draft in drafts {
            by_date.insert(draft.work_date, draft);
        }

        let mut days = Vec::with_capacity(5);
        for date in state.current_week_dates() {
            if let Some(draft) = by_date.remove(&date) {
                days.push(draft.into_ui_day()?);
            } else {
                // Missing server rows mean "no data for this date", not "drop
                // the date from the visible work week".
                days.push(default_day_for_date(date));
            }
        }

        state.replace_current_week_days(days);
        self.current_week_drafts(state)
    }

    pub(crate) fn process_async_results(&mut self, results: Vec<AsyncResult>, runtime: ProcessAsyncContext<'_>) {
        let ProcessAsyncContext {
            state,
            undoer,
            ui_state,
            config,
            async_results,
            ctx,
        } = runtime;
        for result in results {
            match result {
                AsyncResult::Login(result) => {
                    self.in_flight.auth = false;
                    ui_state.clear_login_password();
                    match result {
                        Ok(session) => {
                            info!(
                                target = "auth",
                                user_id = %session.user_id,
                                email = session.email.as_deref().unwrap_or("unknown"),
                                "login completed successfully"
                            );
                            if let Some(email) = &session.email {
                                ui_state.set_login_email(email.clone());
                            }
                            self.stored_session = Some(session);
                            ui_state.set_status_message(format!("Logged in as {}", self.session_label()));
                            ui_state.clear_error();
                            ui_state.set_show_login_window(false);
                            self.synced_week = None;
                            self.request_visible_week_load(state, ui_state, config, async_results, ctx.clone());
                        }
                        Err(err) => {
                            warn!(target = "auth", error = %err, "login failed");
                            ui_state.set_error_message(format!("Login failed: {}", describe_auth_error(&err)));
                            ui_state.set_status_message("Login failed.".to_string());
                        }
                    }
                }
                AsyncResult::RefreshSession(result) => {
                    self.in_flight.auth = false;
                    match result {
                        Ok(session) => {
                            info!(
                                target = "auth",
                                user_id = %session.user_id,
                                email = session.email.as_deref().unwrap_or("unknown"),
                                "session refresh completed successfully"
                            );
                            if let Some(email) = &session.email {
                                ui_state.set_login_email(email.clone());
                            }
                            self.stored_session = Some(session);
                            ui_state.set_status_message(format!("Logged in as {}", self.session_label()));
                            ui_state.clear_error();
                            self.request_visible_week_load(state, ui_state, config, async_results, ctx.clone());
                        }
                        Err(err) => {
                            warn!(target = "auth", error = %err, "session refresh failed");
                            self.stored_session = None;
                            self.synced_week = None;
                            ui_state.set_error_message(format!(
                                "Session refresh failed: {}",
                                describe_auth_error(&err)
                            ));
                            ui_state.set_status_message("Please log in again.".to_string());
                        }
                    }
                }
                AsyncResult::LoadWeek { week, result } => {
                    if self.in_flight.load_week == Some(week) {
                        self.in_flight.load_week = None;
                    }
                    if state.current_week_key() != week {
                        // The user navigated away while the request was in
                        // flight, so ignore the stale payload.
                        debug!(
                            target = "sync",
                            year = week.year,
                            week = week.week_nr,
                            "ignoring stale loaded week result"
                        );
                        continue;
                    }

                    match result {
                        Ok(drafts) => match self.apply_loaded_drafts(state, drafts) {
                            Ok(snapshot_drafts) => {
                                info!(
                                    target = "sync",
                                    year = week.year,
                                    week = week.week_nr,
                                    day_count = snapshot_drafts.len(),
                                    "loaded visible week successfully"
                                );
                                self.synced_week = Some(WeekSyncSnapshot {
                                    week,
                                    drafts: snapshot_drafts,
                                });
                                *undoer = Default::default();
                                ui_state.set_status_message(format!("Loaded week {}", week.week_nr));
                                ui_state.clear_error();
                            }
                            Err(err) => {
                                warn!(target = "sync", error = %err, year = week.year, week = week.week_nr, "failed to apply loaded week");
                                ui_state.set_error_message(format!("Failed to apply loaded week: {err}"));
                            }
                        },
                        Err(err) => {
                            warn!(target = "sync", error = %err, year = week.year, week = week.week_nr, "failed to load visible week");
                            ui_state.set_error_message(format!("Failed to load week: {err}"));
                            ui_state.set_status_message("Week load failed.".to_string());
                        }
                    }
                }
                AsyncResult::SaveWeek { week, result } => {
                    if self.in_flight.save_week == Some(week) {
                        self.in_flight.save_week = None;
                    }
                    if state.current_week_key() != week {
                        debug!(
                            target = "sync",
                            year = week.year,
                            week = week.week_nr,
                            "ignoring stale saved week result"
                        );
                        continue;
                    }

                    match result {
                        Ok(saved_drafts) => {
                            let snapshot_drafts = if saved_drafts.is_empty() {
                                match self.current_week_drafts(state) {
                                    Ok(drafts) => drafts,
                                    Err(err) => {
                                        warn!(target = "sync", error = %err, year = week.year, week = week.week_nr, "save succeeded but snapshot rebuild failed");
                                        ui_state.set_error_message(format!("Saved, but local snapshot failed: {err}"));
                                        continue;
                                    }
                                }
                            } else {
                                saved_drafts
                            };
                            self.synced_week = Some(WeekSyncSnapshot {
                                week,
                                drafts: snapshot_drafts,
                            });
                            info!(
                                target = "sync",
                                year = week.year,
                                week = week.week_nr,
                                "saved visible week successfully"
                            );
                            ui_state.set_status_message(format!("Saved week {}", week.week_nr));
                            ui_state.clear_error();
                        }
                        Err(err) => {
                            warn!(target = "sync", error = %err, year = week.year, week = week.week_nr, "failed to save visible week");
                            ui_state.set_error_message(format!("Failed to save week: {}", summarize_save_error(&err)));
                            ui_state.set_status_message("Week save failed.".to_string());
                        }
                    }
                }
            }
        }
    }

    fn require_config(&mut self, ui_state: &mut AppUiState, config: Option<&AppConfig>, message: &str) -> Option<AppConfig> {
        match config.cloned() {
            Some(config) => Some(config),
            None => {
                ui_state.set_error_message(message.to_string());
                None
            }
        }
    }
}

fn default_day_for_date(date: chrono::NaiveDate) -> ui::Day {
    let mut day = ui::Day::new(date.format("%A").to_string());
    day.date = date;
    day
}

fn supabase_client(config: &AppConfig) -> SupabaseClient {
    SupabaseClient::new(config.supabase_url.clone(), config.supabase_anon_key.clone())
}

fn describe_auth_error(err: &Error) -> String {
    let message = err.to_string();
    if message.contains("failed before response") {
        return format!("request failed before response; {message}");
    }
    if message.contains("Supabase request failed with status") {
        return format!("Supabase rejected request; {message}");
    }
    message
}

async fn save_week_drafts(client: &SupabaseClient, access_token: &str, drafts: Vec<WorkDayDraft>) -> Result<Vec<WorkDayDraft>> {
    let mut saved = Vec::with_capacity(drafts.len());
    for draft in drafts {
        debug!(
            target = "sync",
            work_date = %draft.work_date,
            entry_count = draft.work_entries.len(),
            enabled = draft.enabled,
            target_minutes = draft.target_minutes,
            "saving work day draft"
        );
        let saved_day = client
            .save_work_day(access_token, &draft)
            .await
            .with_context(|| format!("failed to save {}", draft.work_date))?;
        saved.push(WorkDayDraft::from(saved_day));
    }
    saved.sort_by_key(|draft| draft.work_date);
    Ok(saved)
}

fn summarize_save_error(error: &str) -> String {
    if error.contains("save_work_day_with_entries") && error.contains("function") {
        return "database save RPC is missing or mismatched".to_string();
    }
    if let Some(date) = extract_failed_work_date(error) {
        if error.contains("Supabase request failed with status") {
            return format!("server rejected {}", date);
        }
        if error.contains("local date/time") || error.contains("minute precision") {
            return format!("could not convert local time for {}", date);
        }
    }
    error.to_string()
}

fn extract_failed_work_date(error: &str) -> Option<&str> {
    error
        .split_whitespace()
        .collect::<Vec<_>>()
        .windows(2)
        .find_map(|window| if window[0] == "save" { Some(window[1]) } else { None })
}

#[cfg(test)]
mod tests {
    use super::{AsyncResult, SyncState, WeekSyncSnapshot};
    use crate::app::state::{State, WeekKey};
    use crate::supabase::WorkDayDraft;
    use chrono::NaiveDate;

    #[test]
    fn dirty_state_is_false_when_snapshot_matches() {
        let state = State::default();
        let mut sync = SyncState::default();
        sync.stored_session = Some(crate::supabase::StoredSession {
            access_token: "a".to_string(),
            refresh_token: "r".to_string(),
            expires_at: None,
            user_id: "u".to_string(),
            email: None,
        });
        let drafts: Vec<WorkDayDraft> = state
            .days()
            .iter()
            .map(WorkDayDraft::from_ui_day)
            .collect::<Result<_, _>>()
            .unwrap();
        sync.synced_week = Some(WeekSyncSnapshot {
            week: state.current_week_key(),
            drafts,
        });

        assert!(!sync.is_week_dirty(&state));
        let _ = AsyncResult::Login(Ok(sync.stored_session.clone().unwrap()));
    }

    #[test]
    fn dirty_state_is_true_when_snapshot_week_differs() {
        let state = State::default();
        let mut sync = SyncState::default();
        sync.stored_session = Some(crate::supabase::StoredSession {
            access_token: "a".to_string(),
            refresh_token: "r".to_string(),
            expires_at: None,
            user_id: "u".to_string(),
            email: None,
        });
        sync.synced_week = Some(WeekSyncSnapshot {
            week: WeekKey { year: 2020, week_nr: 1 },
            drafts: vec![WorkDayDraft {
                work_date: NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
                target_minutes: 1,
                enabled: true,
                work_entries: vec![],
            }],
        });

        assert!(sync.is_week_dirty(&state));
    }
}
