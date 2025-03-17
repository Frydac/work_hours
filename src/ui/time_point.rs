use crate::ui::SingleDigit;

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct TimePoint {
    pub time: time::OffsetDateTime,
    h10_ui: SingleDigit, // hours tens
    h01_ui: SingleDigit, // hours ones
    m10_ui: SingleDigit, // minutes tens
    m01_ui: SingleDigit, // minutes ones
}

impl TimePoint {
    pub fn now() -> Self {
        let now = time::OffsetDateTime::now_local().unwrap();
        Self {
            time: now,
            h10_ui: SingleDigit::new(now.hour() / 10, "h10").range(0..=2),
            h01_ui: SingleDigit::new(now.hour() % 10, "h01"),
            m10_ui: SingleDigit::new(now.minute() / 10, "m10").range(0..=5),
            m01_ui: SingleDigit::new(now.minute() % 10, "m01").surrender_focus(false),
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let h10_response = self.h10_ui.ui(ui);
            let h01_response = self.h01_ui.ui(ui);
            ui.label(":");
            let m10_response = self.m10_ui.ui(ui);
            let m01_response = self.m01_ui.ui(ui);

            // Whenever a correct digit is entered, the SingleDigit loses focus, here we decide the
            // order.
            if h10_response.lost_focus() {
                h01_response.request_focus();
            }
            if h01_response.lost_focus() {
                m10_response.request_focus();
            }
            if m10_response.lost_focus() {
                m01_response.request_focus();
            }
        });

        let hour = self.h10_ui.value * 10 + self.h01_ui.value;
        let minute = self.m10_ui.value * 10 + self.m01_ui.value;
        self.time = self.time.replace_hour(hour).unwrap().replace_minute(minute).unwrap();
    }
}
