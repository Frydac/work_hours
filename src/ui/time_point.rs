use crate::ui::digitwise_number_editor::{
    request_digitwise_editor_focus, DigitwiseEditorFocusDirection, DigitwiseEditorFocusTransfer, DigitwiseEditorFocusTrigger,
    DigitwiseNumberEditor,
};

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct TimePoint {
    pub time: time::OffsetDateTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimePointOutput {
    pub focus_transfer: Option<DigitwiseEditorFocusTransfer>,
}

impl TimePoint {
    pub fn now() -> Self {
        let now = time::OffsetDateTime::now_local().unwrap();
        Self { time: now }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, id_source: impl std::hash::Hash) -> TimePointOutput {
        let id_source = egui::Id::new(id_source);
        let mut hour = u64::from(self.time.hour());
        let mut minute = u64::from(self.time.minute());
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

        self.time = self.time.replace_hour(hour as u8).unwrap().replace_minute(minute as u8).unwrap();

        TimePointOutput { focus_transfer }
    }
}

impl Default for TimePoint {
    fn default() -> Self {
        Self::now()
    }
}
