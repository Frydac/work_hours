use crate::ui;
use crate::ui::digitwise_number_editor::{
    request_digitwise_editor_focus, DigitwiseEditorFocusDirection, DigitwiseEditorFocusTransfer, DigitwiseEditorFocusTrigger,
};
use chrono::NaiveDate;
use std::sync::atomic::{AtomicU64, Ordering};

// UI model for one work range inside a day. It stores local clock times and an
// optional overnight offset relative to the owning work day, which keeps the UI
// model explicit while leaving absolute timestamp conversion to the Supabase
// boundary.

static NEXT_DURATION_ROW_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Duration {
    #[serde(default = "next_duration_row_id")]
    row_id: u64,
    start: ui::TimePoint,
    end: ui::TimePoint,
    #[serde(default)]
    end_day_offset: i8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DurationOutput {
    pub focus_transfer: Option<DigitwiseEditorFocusTransfer>,
}

impl Default for Duration {
    fn default() -> Self {
        Self {
            row_id: next_duration_row_id(),
            start: ui::TimePoint::now(),
            end: ui::TimePoint::now(),
            end_day_offset: 0,
        }
    }
}

impl Duration {
    /// Creates a duration from absolute timestamps relative to the owning work day.
    pub fn new(work_date: NaiveDate, start: time::OffsetDateTime, end: time::OffsetDateTime) -> Self {
        let local_end_date = local_date(end);
        let end_day_offset = (local_end_date - work_date).num_days().clamp(0, i64::from(i8::MAX)) as i8;

        Self {
            row_id: next_duration_row_id(),
            start: ui::TimePoint::from_offset_datetime(start),
            end: ui::TimePoint::from_offset_datetime(end),
            end_day_offset,
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) -> DurationOutput {
        reserve_duration_row_id(self.row_id);
        let mut focus_transfer = None;
        let mut defer_focus_to_end_hour = false;

        let start_output = self.start.ui(ui, (self.row_id, "start"));
        if let Some(transfer) = start_output.focus_transfer {
            match (transfer.direction, transfer.trigger) {
                (DigitwiseEditorFocusDirection::Next, DigitwiseEditorFocusTrigger::Tab) => {
                    request_digitwise_editor_focus(ui.ctx(), egui::Id::new((self.row_id, "end")).with("hour"), 0);
                }
                (DigitwiseEditorFocusDirection::Next, DigitwiseEditorFocusTrigger::TypedCompletion) => {
                    // Defer the focus jump until after the second editor has been
                    // created so egui has a stable target to focus.
                    defer_focus_to_end_hour = true;
                }
                (DigitwiseEditorFocusDirection::Previous, _) => {
                    focus_transfer = Some(transfer);
                }
            }
        }

        ui.label("->");
        let end_output = self.end.ui(ui, (self.row_id, "end"));

        if let Some(transfer) = end_output.focus_transfer {
            match (transfer.direction, transfer.trigger) {
                (DigitwiseEditorFocusDirection::Previous, DigitwiseEditorFocusTrigger::Tab) => {
                    request_digitwise_editor_focus(ui.ctx(), egui::Id::new((self.row_id, "start")).with("minute"), 1);
                }
                (DigitwiseEditorFocusDirection::Previous, DigitwiseEditorFocusTrigger::TypedCompletion) => {
                    focus_transfer = Some(transfer);
                }
                (DigitwiseEditorFocusDirection::Next, _) => {
                    focus_transfer = Some(transfer);
                }
            }
        }

        if defer_focus_to_end_hour {
            request_digitwise_editor_focus(ui.ctx(), egui::Id::new((self.row_id, "end")).with("hour"), 0);
        }

        DurationOutput { focus_transfer }
    }

    pub fn reserve_row_id(&self) {
        reserve_duration_row_id(self.row_id);
    }

    pub fn row_id(&self) -> u64 {
        self.row_id
    }

    pub fn start_clock(&self) -> &ui::TimePoint {
        &self.start
    }

    pub fn end_clock(&self) -> &ui::TimePoint {
        &self.end
    }

    pub fn effective_end_day_offset(&self) -> i8 {
        if self.end_day_offset > 0 {
            self.end_day_offset
        } else if self.end.total_minutes() < self.start.total_minutes() {
            1
        } else {
            0
        }
    }

    /// Returns the signed difference between end and start, respecting the
    /// overnight offset relative to the owning work day.
    pub fn duration(&self) -> time::Duration {
        let start_minutes = self.start.total_minutes();
        let end_minutes = self.end.total_minutes() + i64::from(self.effective_end_day_offset()) * 24 * 60;
        time::Duration::minutes(end_minutes - start_minutes)
    }

    /// Returns true when the row still represents an unfilled draft rather
    /// than a meaningful work entry.
    pub fn is_zero_length(&self) -> bool {
        self.duration().is_zero()
    }
}

fn next_duration_row_id() -> u64 {
    NEXT_DURATION_ROW_ID.fetch_add(1, Ordering::Relaxed)
}

fn reserve_duration_row_id(row_id: u64) {
    let mut current = NEXT_DURATION_ROW_ID.load(Ordering::Relaxed);
    while current <= row_id {
        // Keep the global counter ahead of all restored row ids so deserialized
        // durations and newly created ones never collide.
        match NEXT_DURATION_ROW_ID.compare_exchange(current, row_id + 1, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => break,
            Err(observed) => current = observed,
        }
    }
}

fn local_date(value: time::OffsetDateTime) -> NaiveDate {
    NaiveDate::from_ymd_opt(value.year(), value.month() as u32, value.day() as u32).expect("OffsetDateTime should map to a valid NaiveDate")
}

pub const DURATION_FORMAT: &str = "%H:%M";

/// Formats a duration using a tiny `%H/%M/%S` placeholder format used by the UI.
pub fn format_duration(duration: time::Duration, format: &str) -> String {
    let total_seconds = duration.whole_seconds();
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    format
        .replace("%H", &format!("{:02}", hours))
        .replace("%M", &format!("{:02}", minutes))
        .replace("%S", &format!("{:02}", seconds))
}
