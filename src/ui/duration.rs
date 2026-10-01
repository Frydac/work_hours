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
    #[serde(default)]
    note: String,
    #[serde(default = "empty_metadata")]
    metadata: serde_json::Value,
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
            note: String::new(),
            metadata: empty_metadata(),
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
            note: String::new(),
            metadata: empty_metadata(),
        }
    }

    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.note = metadata
            .get("note")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned();
        self.metadata = metadata;
        self
    }

    pub fn note(&self) -> &str {
        &self.note
    }

    pub fn set_note(&mut self, note: String) {
        self.note = note;
    }

    /// Preserve the original metadata exactly unless the note was edited.
    pub fn metadata(&self) -> serde_json::Value {
        let original_note = self.metadata.get("note").and_then(serde_json::Value::as_str).unwrap_or_default();
        if self.note == original_note {
            return self.metadata.clone();
        }
        let mut metadata = self.metadata.as_object().cloned().unwrap_or_default();
        if self.note.is_empty() {
            metadata.remove("note");
        } else {
            metadata.insert("note".to_owned(), self.note.clone().into());
        }
        metadata.into()
    }

    pub fn note_id(&self) -> egui::Id {
        egui::Id::new((self.row_id, "note"))
    }

    /// Renders the note below the time controls, consuming Tab before TextEdit
    /// handles it so custom time editors remain in the same traversal order.
    pub fn note_ui(&mut self, ui: &mut egui::Ui, focus_from_end: bool, has_next_row: bool) -> DurationOutput {
        let id = self.note_id();
        let mut focus_transfer = None;
        let mut focus_end = false;
        if ui.memory(|memory| memory.has_focus(id)) {
            focus_end = ui.input_mut(|input| input.consume_key(egui::Modifiers::SHIFT, egui::Key::Tab));
            if has_next_row && !focus_end && ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Tab)) {
                focus_transfer = Some(DigitwiseEditorFocusTransfer {
                    direction: DigitwiseEditorFocusDirection::Next,
                    trigger: DigitwiseEditorFocusTrigger::Tab,
                });
            }
        }
        let width = ui.available_width();
        let rect = egui::Rect::from_min_size(ui.next_widget_position(), egui::vec2(width, 0.0));
        // egui 0.31 also allocates the full galley width for clipped single-line
        // text. Isolate that allocation so long notes cannot grow the day card.
        let mut editor_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        editor_ui.set_clip_rect(
            ui.clip_rect()
                .intersect(egui::Rect::from_x_y_ranges(rect.x_range(), ui.clip_rect().y_range())),
        );
        let response = editor_ui.add(
            egui::TextEdit::singleline(&mut self.note)
                .id(id)
                .hint_text("Note / Jira key")
                .desired_width(width)
                .lock_focus(has_next_row),
        );
        ui.advance_cursor_after_rect(response.rect);
        if focus_from_end {
            response.request_focus();
            ui.memory_mut(|memory| {
                memory.set_focus_lock_filter(
                    id,
                    egui::EventFilter {
                        tab: has_next_row,
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        ..Default::default()
                    },
                )
            });
        }
        if focus_end {
            let end_id = egui::Id::new((self.row_id, "end")).with("minute");
            request_digitwise_editor_focus(ui.ctx(), end_id, 1);
            ui.memory_mut(|memory| {
                let editor_id = egui::Id::new(end_id).with("editor");
                memory.request_focus(editor_id);
                memory.set_focus_lock_filter(
                    editor_id,
                    egui::EventFilter {
                        tab: true,
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        ..Default::default()
                    },
                );
            });
        }
        DurationOutput { focus_transfer }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) -> DurationOutput {
        reserve_duration_row_id(self.row_id);
        let mut focus_transfer = None;
        let mut defer_focus_to_end_hour = false;

        // Stack the time editors when a narrow card cannot fit the complete range.
        let stacked = ui.available_width() < 180.0;
        let start_output = if stacked {
            ui.horizontal(|ui| self.start.ui(ui, (self.row_id, "start"))).inner
        } else {
            self.start.ui(ui, (self.row_id, "start"))
        };
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

        if !stacked {
            ui.label("->");
        }
        let end_output = if stacked {
            ui.horizontal(|ui| self.end.ui(ui, (self.row_id, "end"))).inner
        } else {
            self.end.ui(ui, (self.row_id, "end"))
        };

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

fn empty_metadata() -> serde_json::Value {
    serde_json::json!({})
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

#[cfg(test)]
mod tests {
    use super::Duration;
    use serde_json::json;

    #[test]
    fn legacy_durations_without_note_fields_still_load() {
        let duration: Duration =
            serde_json::from_value(json!({"start": {"hour": 9, "minute": 0}, "end": {"hour": 10, "minute": 30}})).unwrap();
        assert_eq!(duration.note(), "");
        assert_eq!(duration.metadata(), json!({}));
        assert_eq!(duration.duration(), time::Duration::minutes(90));
    }

    #[test]
    fn undo_and_redo_include_note_edits() {
        let mut duration = Duration::default();
        let mut undoer = egui::util::undoer::Undoer::default();
        undoer.feed_state(0.0, &duration);
        duration.set_note("PROJ-123".to_owned());
        let undone = undoer.undo(&duration).unwrap().clone();
        assert_eq!(undone.note(), "");
        let redone = undoer.redo(&undone).unwrap();
        assert_eq!(redone.note(), "PROJ-123");
    }
}
