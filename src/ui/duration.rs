use crate::ui;

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Duration {
    start: ui::TimePoint,
    end: ui::TimePoint,
}

impl Default for Duration {
    fn default() -> Self {
        Self {
            start: ui::TimePoint::now(),
            end: ui::TimePoint::now(),
        }
    }
}

impl Duration {
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        self.start.ui(ui);
        ui.label("->");
        self.end.ui(ui);
    }

    pub fn duration(&self) -> time::Duration {
        self.end.time - self.start.time
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
