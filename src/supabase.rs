use crate::ui;
use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, FixedOffset, NaiveDate, TimeZone, Utc};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, instrument, warn};
#[cfg(target_arch = "wasm32")]
use web_sys::wasm_bindgen::JsValue;

// Supabase transport and conversion layer. This file owns REST DTOs, session
// types, and helpers that translate between the app's UI model and the database
// shape used in Supabase.

const JSON_CONTENT_TYPE: &str = "application/json";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WorkDayRow {
    pub id: String,
    pub user_id: String,
    pub work_date: NaiveDate,
    pub target_minutes: i32,
    pub enabled: bool,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WorkEntryRow {
    pub id: String,
    pub work_day_id: String,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    #[serde(default)]
    pub metadata: serde_json::Value,
    pub sort_index: i32,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WorkDayWithEntries {
    #[serde(flatten)]
    pub day: WorkDayRow,
    #[serde(default)]
    pub work_entries: Vec<WorkEntryRow>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UpsertWorkDayPayload {
    pub work_date: NaiveDate,
    pub target_minutes: i32,
    pub enabled: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct WorkEntryDraft {
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    #[serde(default)]
    pub metadata: serde_json::Value,
    pub sort_index: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct WorkDayDraft {
    pub work_date: NaiveDate,
    pub target_minutes: i32,
    pub enabled: bool,
    #[serde(default)]
    pub work_entries: Vec<WorkEntryDraft>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AuthSession {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub expires_at: Option<i64>,
    pub user: AuthUser,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AuthUser {
    pub id: String,
    pub email: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct StoredSession {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: Option<i64>,
    pub user_id: String,
    pub email: Option<String>,
}

#[derive(Debug, Serialize)]
struct PasswordSignInRequest<'a> {
    email: &'a str,
    password: &'a str,
}

#[derive(Debug, Serialize)]
struct SaveWorkDayRpcRequest<'a> {
    p_work_date: NaiveDate,
    p_target_minutes: i32,
    p_enabled: bool,
    p_entries: &'a [SaveWorkEntryRpcPayload],
}

#[derive(Debug, Serialize)]
struct SaveWorkEntryRpcPayload {
    starts_at: DateTime<Utc>,
    ends_at: DateTime<Utc>,
    metadata: serde_json::Value,
    sort_index: i32,
}

pub struct SupabaseClient {
    pub url: String,
    pub api_key: String,
    http: reqwest::Client,
}

impl SupabaseClient {
    /// Creates a lightweight REST client for the configured Supabase project.
    pub fn new(url: String, api_key: String) -> Self {
        Self {
            url,
            api_key,
            http: reqwest::Client::new(),
        }
    }

    /// Signs in with email/password and returns the raw Supabase auth session.
    #[instrument(name = "supabase_sign_in_password", skip_all, fields(email = %email))]
    pub async fn sign_in_password(&self, email: &str, password: &str) -> Result<AuthSession> {
        let url = format!("{}/auth/v1/token?grant_type=password", self.url);
        info!(target = "supabase", auth_url = %url, "calling Supabase password sign-in");
        let response = self
            .http
            .post(url.clone())
            .header("apikey", &self.api_key)
            .header(CONTENT_TYPE, JSON_CONTENT_TYPE)
            .json(&PasswordSignInRequest { email, password })
            .send()
            .await
            .inspect_err(|err| {
                Self::log_transport_error("password sign-in", &url, err);
            })
            .with_context(|| Self::transport_error_context("password sign-in", &url))?;

        Self::decode_json_response("password sign-in", response).await
    }

    #[instrument(name = "supabase_refresh_session", skip_all)]
    pub async fn refresh_session(&self, refresh_token: &str) -> Result<AuthSession> {
        let url = format!("{}/auth/v1/token?grant_type=refresh_token", self.url);
        info!(target = "supabase", refresh_url = %url, "calling Supabase session refresh");
        let response = self
            .http
            .post(url.clone())
            .header("apikey", &self.api_key)
            .header(CONTENT_TYPE, JSON_CONTENT_TYPE)
            .json(&serde_json::json!({ "refresh_token": refresh_token }))
            .send()
            .await
            .inspect_err(|err| {
                Self::log_transport_error("session refresh", &url, err);
            })
            .with_context(|| Self::transport_error_context("session refresh", &url))?;

        Self::decode_json_response("session refresh", response).await
    }

    /// Loads a single day together with its entries.
    #[instrument(name = "supabase_get_work_day", skip_all, fields(work_date = %work_date))]
    pub async fn get_work_day(&self, access_token: &str, work_date: NaiveDate) -> Result<Vec<WorkDayWithEntries>> {
        let url = format!(
            "{}/rest/v1/work_days?select=*,work_entries(*)&work_date=eq.{}&order=sort_index.asc&work_entries.order=sort_index.asc",
            self.url, work_date
        );
        info!(target = "supabase", "fetching single work day");

        let response = self
            .authed_get(url, access_token)
            .send()
            .await
            .context("failed to fetch work day from Supabase")?;

        Self::decode_json_response("get work day", response).await
    }

    /// Loads all days in the requested range, including their nested entries.
    #[instrument(
        name = "supabase_get_work_days_range",
        skip_all,
        fields(start_date = %start_date, end_date = %end_date)
    )]
    pub async fn get_work_days_range(
        &self,
        access_token: &str,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<Vec<WorkDayWithEntries>> {
        let url = format!(
            "{}/rest/v1/work_days?select=*,work_entries(*)&work_date=gte.{}&work_date=lte.{}&order=work_date.asc&work_entries.order=sort_index.asc",
            self.url, start_date, end_date
        );
        info!(target = "supabase", "fetching work day range");

        let response = self
            .authed_get(url, access_token)
            .send()
            .await
            .context("failed to fetch work day range from Supabase")?;

        Self::decode_json_response("get work day range", response).await
    }

    /// Saves the full day as the app understands it: one day row plus its
    /// complete ordered entry list.
    #[instrument(
        name = "supabase_save_work_day",
        skip_all,
        fields(work_date = %draft.work_date, entry_count = draft.work_entries.len())
    )]
    pub async fn save_work_day(&self, access_token: &str, draft: &WorkDayDraft) -> Result<WorkDayWithEntries> {
        let rpc_payload: Vec<_> = draft
            .work_entries
            .iter()
            .map(|entry| SaveWorkEntryRpcPayload {
                starts_at: entry.starts_at,
                ends_at: entry.ends_at,
                metadata: entry.metadata.clone(),
                sort_index: entry.sort_index,
            })
            .collect();

        let url = format!("{}/rest/v1/rpc/save_work_day_with_entries", self.url);
        info!(
            target = "supabase",
            work_date = %draft.work_date,
            entry_count = draft.work_entries.len(),
            enabled = draft.enabled,
            target_minutes = draft.target_minutes,
            "saving work day through RPC"
        );
        let response = self
            .authed_request(reqwest::Method::POST, url, access_token)
            .json(&SaveWorkDayRpcRequest {
                p_work_date: draft.work_date,
                p_target_minutes: draft.target_minutes,
                p_enabled: draft.enabled,
                p_entries: &rpc_payload,
            })
            .send()
            .await
            .context("failed to save work day through Supabase RPC")?;

        Self::decode_json_response("save work day RPC", response).await
    }

    fn authed_get(&self, url: String, access_token: &str) -> reqwest::RequestBuilder {
        self.authed_request(reqwest::Method::GET, url, access_token)
    }

    fn authed_request(&self, method: reqwest::Method, url: String, access_token: &str) -> reqwest::RequestBuilder {
        self.http
            .request(method, url)
            .header("apikey", &self.api_key)
            .header(AUTHORIZATION, format!("Bearer {access_token}"))
    }

    async fn decode_json_response<T>(purpose: &'static str, response: reqwest::Response) -> Result<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        let response = Self::error_for_status(purpose, response).await?;
        response
            .json::<T>()
            .await
            .with_context(|| format!("failed to decode Supabase JSON response for {purpose}"))
    }

    async fn error_for_status(purpose: &'static str, response: reqwest::Response) -> Result<reqwest::Response> {
        let status = response.status();
        debug!(target = "supabase", purpose, http_status = %status, "received Supabase response");
        if status.is_success() {
            return Ok(response);
        }

        let body = response.text().await.unwrap_or_default();
        warn!(target = "supabase", purpose, http_status = %status, response_body = %body, "Supabase request failed");
        Err(anyhow!(
            "Supabase request failed with status {} during {}: {}",
            status,
            purpose,
            body
        ))
    }

    fn transport_error_context(purpose: &'static str, url: &str) -> String {
        format!(
            "Supabase {} request failed before response from {}",
            purpose,
            Self::loggable_url(url)
        )
    }

    fn loggable_url(url: &str) -> String {
        url.split('?').next().unwrap_or(url).to_string()
    }

    fn log_transport_error(purpose: &'static str, url: &str, err: &reqwest::Error) {
        #[cfg(target_arch = "wasm32")]
        {
            let message = format!(
                "Supabase {} request failed before response from {}: {}",
                purpose,
                Self::loggable_url(url),
                err
            );
            web_sys::console::error_1(&JsValue::from_str(&message));
        }
        warn!(target = "supabase", purpose, error = %err, request_url = %Self::loggable_url(url), "Supabase transport request failed");
    }
}

impl From<AuthSession> for StoredSession {
    fn from(value: AuthSession) -> Self {
        Self {
            access_token: value.access_token,
            refresh_token: value.refresh_token,
            expires_at: value.expires_at,
            user_id: value.user.id,
            email: value.user.email,
        }
    }
}

impl StoredSession {
    /// Returns true when the access token is already expired or close enough to
    /// expiry that the app should refresh it before making requests.
    pub fn is_expired_or_near_expiry(&self, now_unix: i64) -> bool {
        match self.expires_at {
            Some(expires_at) => expires_at <= now_unix + 60,
            None => false,
        }
    }
}

impl WorkDayDraft {
    /// Converts the in-memory UI model into the day shape written to Supabase.
    pub fn from_ui_day(day: &ui::Day) -> Result<Self> {
        debug!(
            target = "supabase",
            work_date = %day.date,
            entry_count = day.durations.len(),
            enabled = day.enabled,
            "converting UI day into Supabase draft"
        );
        let work_entries = day
            .durations
            .iter()
            .filter(|entry| {
                if entry.is_zero_length() {
                    debug!(target = "supabase", work_date = %day.date, "skipping zero-length duration during draft conversion");
                    false
                } else {
                    true
                }
            })
            .enumerate()
            .map(|(ix, entry)| {
                let (starts_at, ends_at) = local_duration_to_utc_range(day.date, entry)?;
                Ok(WorkEntryDraft {
                    starts_at,
                    ends_at,
                    metadata: serde_json::Value::Object(Default::default()),
                    sort_index: i32::try_from(ix).context("too many entries in a single day")?,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(Self {
            work_date: day.date,
            target_minutes: duration_to_minutes(day.configured_target())?,
            enabled: day.enabled,
            work_entries,
        })
    }

    /// Rebuilds the UI day model from a Supabase-oriented draft.
    pub fn into_ui_day(self) -> Result<ui::Day> {
        debug!(
            target = "supabase",
            work_date = %self.work_date,
            entry_count = self.work_entries.len(),
            enabled = self.enabled,
            "converting Supabase draft into UI day"
        );
        let mut day = ui::Day::new(self.work_date.format("%A").to_string());
        day.date = self.work_date;
        day.enabled = self.enabled;
        day.set_target(minutes_to_duration(self.target_minutes));
        day.durations = self
            .work_entries
            .into_iter()
            .map(|entry| {
                Ok(ui::Duration::new(
                    self.work_date,
                    to_local_offset(entry.starts_at)?,
                    to_local_offset(entry.ends_at)?,
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(day)
    }
}

impl From<WorkDayWithEntries> for WorkDayDraft {
    fn from(value: WorkDayWithEntries) -> Self {
        let work_entries = value
            .work_entries
            .into_iter()
            .map(|entry| WorkEntryDraft {
                starts_at: entry.starts_at,
                ends_at: entry.ends_at,
                metadata: entry.metadata,
                sort_index: entry.sort_index,
            })
            .collect();

        Self {
            work_date: value.day.work_date,
            target_minutes: value.day.target_minutes,
            enabled: value.day.enabled,
            work_entries,
        }
    }
}

fn duration_to_minutes(duration: time::Duration) -> Result<i32> {
    if duration.whole_seconds() % 60 != 0 {
        // The database stores targets in whole minutes, so reject values that
        // would lose precision during serialization.
        return Err(anyhow!("duration must be stored at minute precision"));
    }

    i32::try_from(duration.whole_minutes()).context("duration does not fit in i32 minutes")
}

fn minutes_to_duration(minutes: i32) -> time::Duration {
    time::Duration::minutes(i64::from(minutes))
}

fn local_duration_to_utc_range(day_date: NaiveDate, entry: &ui::Duration) -> Result<(DateTime<Utc>, DateTime<Utc>)> {
    let start = local_clock_to_utc(day_date, entry.start_clock(), 0)?;
    let end = local_clock_to_utc(day_date, entry.end_clock(), i64::from(entry.effective_end_day_offset()))?;
    debug!(
        target = "supabase",
        work_date = %day_date,
        start_hour = entry.start_clock().hour(),
        start_minute = entry.start_clock().minute(),
        end_hour = entry.end_clock().hour(),
        end_minute = entry.end_clock().minute(),
        end_day_offset = entry.effective_end_day_offset(),
        "converted local duration into UTC range"
    );
    Ok((start, end))
}

fn local_clock_to_utc(day_date: NaiveDate, time_point: &ui::TimePoint, day_offset: i64) -> Result<DateTime<Utc>> {
    let local_date = day_date + chrono::Duration::days(day_offset);
    let naive = local_date
        .and_hms_opt(u32::from(time_point.hour()), u32::from(time_point.minute()), 0)
        .ok_or_else(|| anyhow!("invalid local date/time for work entry"))?;
    let offset = current_fixed_offset()?;
    let local = offset
        .from_local_datetime(&naive)
        .single()
        .or_else(|| offset.from_local_datetime(&naive).earliest())
        .ok_or_else(|| anyhow!("local date/time is ambiguous or invalid"))?;
    Ok(local.with_timezone(&Utc))
}

fn to_local_offset(value: DateTime<Utc>) -> Result<time::OffsetDateTime> {
    let timestamp = time::OffsetDateTime::from_unix_timestamp(value.timestamp())
        .context("timestamp from Supabase is outside OffsetDateTime range")?
        .replace_nanosecond(value.timestamp_subsec_nanos())
        .context("timestamp nanoseconds from Supabase are invalid")?;
    // Persist timestamps in UTC, but present them in the local offset so the
    // existing time editors continue to behave like local wall-clock fields.
    let local_offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
    Ok(timestamp.to_offset(local_offset))
}

fn current_fixed_offset() -> Result<FixedOffset> {
    let offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
    FixedOffset::east_opt(offset.whole_seconds()).ok_or_else(|| anyhow!("local UTC offset is outside chrono's supported range"))
}

#[cfg(test)]
mod tests {
    use super::{duration_to_minutes, minutes_to_duration, AuthSession, AuthUser, StoredSession, WorkDayDraft};
    use crate::ui;
    use chrono::NaiveDate;
    use serde_json::json;

    #[test]
    fn target_duration_round_trips_through_minutes() {
        let duration = time::Duration::hours(7) + time::Duration::minutes(36);
        let minutes = duration_to_minutes(duration).unwrap();
        assert_eq!(minutes, 456);
        assert_eq!(minutes_to_duration(minutes), duration);
    }

    #[test]
    fn ui_day_round_trips_through_supabase_draft() {
        let date = NaiveDate::from_ymd_opt(2026, 4, 20).unwrap();
        let start = time::OffsetDateTime::from_unix_timestamp(1_776_667_200).unwrap();
        let end = start + time::Duration::hours(2);

        let mut day = ui::Day::new("Monday".to_string());
        day.date = date;
        day.enabled = true;
        day.set_target(time::Duration::hours(8));
        day.durations = vec![ui::Duration::new(date, start, end)];

        let mut draft = WorkDayDraft::from_ui_day(&day).unwrap();
        draft.work_entries[0].metadata = json!({ "note": "pairing" });
        let round_tripped = draft.into_ui_day().unwrap();

        assert_eq!(round_tripped.date, date);
        assert_eq!(round_tripped.enabled, day.enabled);
        assert_eq!(round_tripped.configured_target(), time::Duration::hours(8));
        assert_eq!(round_tripped.durations.len(), 1);
        assert_eq!(round_tripped.durations[0].duration(), time::Duration::hours(2));
    }

    #[test]
    fn overnight_entry_round_trips_through_supabase_draft() {
        let date = NaiveDate::from_ymd_opt(2026, 4, 20).unwrap();
        let start = time::OffsetDateTime::from_unix_timestamp(1_776_718_200).unwrap();
        let end = start + time::Duration::hours(3);

        let mut day = ui::Day::new("Monday".to_string());
        day.date = date;
        day.enabled = true;
        day.durations = vec![ui::Duration::new(date, start, end)];

        let draft = WorkDayDraft::from_ui_day(&day).unwrap();
        let round_tripped = draft.into_ui_day().unwrap();

        assert_eq!(round_tripped.durations.len(), 1);
        assert_eq!(round_tripped.durations[0].duration(), time::Duration::hours(3));
    }

    #[test]
    fn zero_length_entry_is_ignored_during_draft_conversion() {
        let date = NaiveDate::from_ymd_opt(2026, 4, 20).unwrap();
        let start = time::OffsetDateTime::from_unix_timestamp(1_776_667_200).unwrap();

        let mut day = ui::Day::new("Monday".to_string());
        day.date = date;
        day.enabled = true;
        day.durations = vec![ui::Duration::new(date, start, start)];

        let draft = WorkDayDraft::from_ui_day(&day).unwrap();

        assert!(draft.work_entries.is_empty());
    }

    #[test]
    fn stored_session_is_built_from_auth_session() {
        let stored = StoredSession::from(AuthSession {
            access_token: "access".to_string(),
            refresh_token: "refresh".to_string(),
            token_type: "bearer".to_string(),
            expires_in: 3600,
            expires_at: Some(1_800_000_000),
            user: AuthUser {
                id: "user-123".to_string(),
                email: Some("user@example.com".to_string()),
            },
        });

        assert_eq!(stored.user_id, "user-123");
        assert_eq!(stored.email.as_deref(), Some("user@example.com"));
        assert_eq!(stored.refresh_token, "refresh");
    }
}
