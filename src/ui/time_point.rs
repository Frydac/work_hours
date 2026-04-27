use crate::ui::digitwise_number_editor::{
    request_digitwise_editor_focus, DigitwiseEditorFocusDirection, DigitwiseEditorFocusTransfer, DigitwiseEditorFocusTrigger,
    DigitwiseNumberEditor,
};

// Small time-of-day editor used inside a duration row. This stores only local
// clock fields; the owning day date and any overnight interpretation live one
// level up in the duration/day model.

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct TimePoint {
    hour: u8,
    minute: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimePointOutput {
    pub focus_transfer: Option<DigitwiseEditorFocusTransfer>,
}

impl TimePoint {
    /// Creates a time point from the current local wall-clock time.
    pub fn now() -> Self {
        let now = time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
        Self {
            hour: now.hour(),
            minute: now.minute(),
        }
    }

    /// Builds a local clock value from an existing timestamp.
    pub fn from_offset_datetime(value: time::OffsetDateTime) -> Self {
        Self {
            hour: value.hour(),
            minute: value.minute(),
        }
    }

    pub fn hour(&self) -> u8 {
        self.hour
    }

    pub fn minute(&self) -> u8 {
        self.minute
    }

    pub fn total_minutes(&self) -> i64 {
        i64::from(self.hour) * 60 + i64::from(self.minute)
    }

    /// Renders the hour/minute editor and reports whether focus should move to
    /// another digit editor after the current interaction.
    pub fn ui(&mut self, ui: &mut egui::Ui, id_source: impl std::hash::Hash) -> TimePointOutput {
        let id_source = egui::Id::new(id_source);
        let mut hour = u64::from(self.hour);
        let mut minute = u64::from(self.minute);
        let mut focus_transfer = None;
        let mut defer_focus_to_minute = false;

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;

            let hour_output = DigitwiseNumberEditor::new(id_source.with("hour"), &mut hour)
                .digits(2)
                .digit_width(12.0)
                .max(23)
                .dim_leading_zeroes(false)
                .show(ui);

            if let Some(transfer) = hour_output.focus_transfer {
                match (transfer.direction, transfer.trigger) {
                    (DigitwiseEditorFocusDirection::Next, DigitwiseEditorFocusTrigger::Tab) => {
                        request_digitwise_editor_focus(ui.ctx(), id_source.with("minute"), 0);
                    }
                    (DigitwiseEditorFocusDirection::Next, DigitwiseEditorFocusTrigger::TypedCompletion) => {
                        defer_focus_to_minute = true;
                    }
                    (DigitwiseEditorFocusDirection::Previous, _) => {
                        focus_transfer = Some(transfer);
                    }
                }
            }

            ui.label(":");

            let minute_output = DigitwiseNumberEditor::new(id_source.with("minute"), &mut minute)
                .digits(2)
                .digit_width(12.0)
                .max(59)
                .dim_leading_zeroes(false)
                .show(ui);

            if let Some(transfer) = minute_output.focus_transfer {
                match (transfer.direction, transfer.trigger) {
                    (DigitwiseEditorFocusDirection::Previous, DigitwiseEditorFocusTrigger::Tab) => {
                        request_digitwise_editor_focus(ui.ctx(), id_source.with("hour"), 1);
                        hour_output.response.request_focus();
                    }
                    (DigitwiseEditorFocusDirection::Previous, DigitwiseEditorFocusTrigger::TypedCompletion) => {
                        focus_transfer = Some(transfer);
                    }
                    (DigitwiseEditorFocusDirection::Next, _) => {
                        focus_transfer = Some(transfer);
                    }
                }
            }

            if defer_focus_to_minute {
                request_digitwise_editor_focus(ui.ctx(), id_source.with("minute"), 0);
            }
        });

        self.hour = hour as u8;
        self.minute = minute as u8;

        TimePointOutput { focus_transfer }
    }
}

impl Default for TimePoint {
    fn default() -> Self {
        Self::now()
    }
}
