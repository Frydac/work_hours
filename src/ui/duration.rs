use crate::ui;
use crate::ui::digitwise_number_editor::{
    request_digitwise_editor_focus, DigitwiseEditorFocusDirection, DigitwiseEditorFocusTransfer, DigitwiseEditorFocusTrigger,
};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_DURATION_ROW_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Duration {
    #[serde(default = "next_duration_row_id")]
    row_id: u64,
    start: ui::TimePoint,
    end: ui::TimePoint,
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
        }
    }
}

impl Duration {
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

    pub fn duration(&self) -> time::Duration {
        self.end.time - self.start.time
    }
}

fn next_duration_row_id() -> u64 {
    NEXT_DURATION_ROW_ID.fetch_add(1, Ordering::Relaxed)
}

fn reserve_duration_row_id(row_id: u64) {
    let mut current = NEXT_DURATION_ROW_ID.load(Ordering::Relaxed);
    while current <= row_id {
        match NEXT_DURATION_ROW_ID.compare_exchange(current, row_id + 1, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => break,
            Err(observed) => current = observed,
        }
    }
}

pub const DURATION_FORMAT: &str = "%H:%M";

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
