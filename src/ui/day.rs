use crate::ui;
use crate::ui::digitwise_number_editor::{request_digitwise_editor_focus, DigitwiseEditorFocusDirection, DigitwiseEditorFocusTrigger};
use egui::{Align, Layout, RichText};

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Day {
    pub durations: Vec<ui::Duration>,
    pub name: String,
    total_target: time::Duration,
    pub enabled: bool,
}

impl Day {
    pub fn new(name: String) -> Self {
        Day {
            name,
            enabled: true,
            ..Default::default()
        }
    }
    pub fn with_target(mut self, target: time::Duration) -> Self {
        self.total_target = target;
        self
    }

    pub fn target(&self) -> time::Duration {
        if !self.enabled {
            return time::Duration::ZERO;
        }
        self.total_target
    }

    pub fn duration(&self) -> time::Duration {
        if !self.enabled {
            return time::Duration::ZERO;
        }

        let mut duration = time::Duration::ZERO;
        for dur in &self.durations {
            duration += dur.duration();
        }
        duration
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        for duration in &self.durations {
            duration.reserve_row_id();
        }

        let frame_width = 180.0;
        egui::Frame::new()
            // .stroke(egui::Stroke::new(1.0, egui::Color32::LIGHT_GRAY))
            .inner_margin(egui::Margin::same(10))
            .show(ui, |ui| {
                ui.set_width(frame_width);
                // Center-aligned text
                ui.with_layout(Layout::top_down(Align::Center), |ui| {
                    // ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.enabled, "");
                        ui.label(RichText::new(&self.name).size(16.0));
                    });
                    // ui.with_layout(Layout::left_to_right(Align::RIGHT), |ui| {
                    // });
                    // });

                    ui.separator();

                    // totals grid
                    let nr_cols = 2;
                    egui::Grid::new(&self.name)
                        .striped(true)
                        .min_col_width(frame_width / nr_cols as f32)
                        // .max_col_width(200.0)
                        .show(ui, |ui| {
                            {
                                ui.label("Target:");
                                ui.label(ui::duration::format_duration(self.total_target, ui::duration::DURATION_FORMAT));
                                ui.end_row();
                            }
                            {
                                ui.label("Done:");
                                ui.label(ui::duration::format_duration(self.duration(), ui::duration::DURATION_FORMAT));
                                ui.end_row();
                            }
                            {
                                ui.label("Todo:");
                                let todo = self.total_target - self.duration();
                                let sign = if todo.is_negative() { "-" } else { "" };
                                ui.label(format!(
                                    "{}{}",
                                    sign,
                                    ui::duration::format_duration(todo.abs(), ui::duration::DURATION_FORMAT)
                                ));
                                // ui.label(format_duration(todo, DURATION_FORMAT));
                                ui.end_row();
                            }
                        });

                    ui.separator();

                    // Add/Clear buttons
                    ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
                        let add_button = egui::Button::new("Add +"); // Create the button instance
                        if ui
                            .add(add_button)
                            .on_hover_text(format!("Add a new duration to {}", self.name))
                            .clicked()
                        {
                            self.durations.push(ui::Duration::default());
                        }
                        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                            if ui
                                .add_enabled(!self.durations.is_empty(), egui::Button::new("Clear"))
                                .on_hover_text(format!("Remove all durations for {}", self.name))
                                .clicked()
                            {
                                self.durations.clear();
                            }
                        });
                    });

                    ui.separator();

                    // the durations
                    {
                        let row_ids: Vec<u64> = self.durations.iter().map(ui::Duration::row_id).collect();
                        let mut remove_ix = None;
                        let mut defer_focus_to_row_start = None;
                        for (ix, duration) in &mut self.durations.iter_mut().enumerate() {
                            if let Some(target_row_id) = defer_focus_to_row_start.take() {
                                request_digitwise_editor_focus(ui.ctx(), egui::Id::new((target_row_id, "start")).with("hour"), 0);
                            }
                            // if ix == 0 {
                            //     ui.separator();
                            // }
                            ui.horizontal(|ui| {
                                // add duration
                                let duration_output = duration.ui(ui);

                                if let Some(transfer) = duration_output.focus_transfer {
                                    match (transfer.direction, transfer.trigger) {
                                        (DigitwiseEditorFocusDirection::Next, DigitwiseEditorFocusTrigger::Tab) => {
                                            if let Some(next_row_id) = row_ids.get(ix + 1) {
                                                request_digitwise_editor_focus(
                                                    ui.ctx(),
                                                    egui::Id::new((*next_row_id, "start")).with("hour"),
                                                    0,
                                                );
                                            }
                                        }
                                        (DigitwiseEditorFocusDirection::Next, DigitwiseEditorFocusTrigger::TypedCompletion) => {
                                            if let Some(next_row_id) = row_ids.get(ix + 1) {
                                                defer_focus_to_row_start = Some(*next_row_id);
                                            }
                                        }
                                        (DigitwiseEditorFocusDirection::Previous, DigitwiseEditorFocusTrigger::Tab) => {
                                            if ix > 0 {
                                                let previous_row_id = row_ids[ix - 1];
                                                request_digitwise_editor_focus(
                                                    ui.ctx(),
                                                    egui::Id::new((previous_row_id, "end")).with("minute"),
                                                    1,
                                                );
                                            }
                                        }
                                        (DigitwiseEditorFocusDirection::Previous, DigitwiseEditorFocusTrigger::TypedCompletion) => {}
                                    }
                                }
                                // duration.ui_text_edit(ui);
                                // duration.ui2(ui);
                                // add remove button
                                if ui
                                    // ⊗ ⛒ 🗙 ×
                                    // .add(egui::Button::new("×").rounding(10.0))
                                    .add(egui::Button::new("×").corner_radius(10.0))
                                    .on_hover_text("Remove duration")
                                    .clicked()
                                {
                                    remove_ix = Some(ix);
                                }
                            });
                        }
                        // We assume only 1 remove button could have been clicked during the loop
                        if let Some(my_ix) = remove_ix {
                            self.durations.remove(my_ix);
                        }
                    }

                    // ui.separator();
                });

                // Add the margin around the label
                // ui.add(margin, egui::Label::new("Hello, egui!"));
            });
    }
}
