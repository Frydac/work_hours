use crate::ui;
use crate::ui::digitwise_number_editor::{request_digitwise_editor_focus, DigitwiseEditorFocusDirection, DigitwiseEditorFocusTrigger};
use chrono::NaiveDate;
use egui::{Align, Layout, RichText};

// A single visible work day in the UI: target, enabled flag, date, and an
// ordered list of time ranges for that day.

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Day {
    pub durations: Vec<ui::Duration>,
    pub name: String,
    total_target: time::Duration,
    pub enabled: bool,
    pub date: NaiveDate,
}

impl Day {
    /// Default daily target for newly created weekdays.
    pub fn default_target() -> time::Duration {
        time::Duration::hours(7) + time::Duration::minutes(36)
    }

    /// Creates a new enabled day with the default target.
    pub fn new(name: String) -> Self {
        Day {
            name,
            enabled: true,
            total_target: Self::default_target(),
            ..Default::default()
        }
    }

    #[allow(dead_code)]
    pub fn with_target(mut self, target: time::Duration) -> Self {
        self.total_target = target;
        self
    }

    /// Returns the configured target even when the day is disabled.
    ///
    /// This is useful for persistence and editing, where we still want to keep
    /// the stored target around even if the day should not count toward totals.
    pub fn configured_target(&self) -> time::Duration {
        self.total_target
    }

    /// Returns the effective target that should count toward totals.
    ///
    /// Disabled days contribute zero target so weekends or skipped days can stay
    /// in the model without affecting week totals.
    pub fn target(&self) -> time::Duration {
        if !self.enabled {
            return time::Duration::ZERO;
        }
        self.total_target
    }

    pub fn set_target(&mut self, target: time::Duration) {
        self.total_target = target;
    }

    /// Returns the sum of all durations that should count for this day.
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

    pub fn add_duration(&mut self, previous_note: &str) {
        let note = self.durations.last().map(|entry| entry.note()).unwrap_or(previous_note).to_owned();
        let mut duration = ui::Duration::default();
        duration.set_note(note);
        self.durations.push(duration);
    }

    /// Renders a card at the assigned size and returns its natural outer height before added blank space.
    pub fn ui(&mut self, ui: &mut egui::Ui, outer_width: f32, outer_height: f32, previous_note: &str) -> egui::InnerResponse<f32> {
        for duration in &self.durations {
            duration.reserve_row_id();
        }

        let frame = egui::Frame::group(ui.style()).inner_margin(egui::Margin::same(10));
        let margins = frame.total_margin().sum();
        let content_width = (outer_width - margins.x).max(0.0);
        frame.show(ui, |ui| {
            ui.set_width(content_width);
            ui.with_layout(Layout::top_down(Align::Min), |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.checkbox(&mut self.enabled, "");
                    ui.label(RichText::new(&self.name).size(16.0));
                });
                ui.label(RichText::new(self.date.to_string()).size(12.0));
                ui.separator();

                let todo = self.total_target - self.duration();
                let sign = if todo.is_negative() { "-" } else { "" };
                let totals = [
                    (
                        "Target:",
                        ui::duration::format_duration(self.total_target, ui::duration::DURATION_FORMAT),
                    ),
                    (
                        "Done:",
                        ui::duration::format_duration(self.duration(), ui::duration::DURATION_FORMAT),
                    ),
                    (
                        "Todo:",
                        format!(
                            "{}{}",
                            sign,
                            ui::duration::format_duration(todo.abs(), ui::duration::DURATION_FORMAT)
                        ),
                    ),
                ];
                for (index, (label, value)) in totals.into_iter().enumerate() {
                    let fill = if index == 1 {
                        ui.visuals().faint_bg_color
                    } else {
                        egui::Color32::TRANSPARENT
                    };
                    egui::Frame::new().fill(fill).show(ui, |ui| {
                        ui.set_width(content_width);
                        if content_width >= 120.0 {
                            ui.horizontal(|ui| {
                                ui.label(label);
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    ui.label(value);
                                });
                            });
                        } else {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(label);
                                ui.label(value);
                            });
                        }
                    });
                }
                ui.separator();

                ui.horizontal_wrapped(|ui| {
                    if ui
                        .button("Add +")
                        .on_hover_text(format!("Add a new duration to {}", self.name))
                        .clicked()
                    {
                        self.add_duration(previous_note);
                    }
                    if ui
                        .add_enabled(!self.durations.is_empty(), egui::Button::new("Clear"))
                        .on_hover_text(format!("Remove all durations for {}", self.name))
                        .clicked()
                    {
                        self.durations.clear();
                    }
                });
                ui.separator();

                // the durations
                {
                    let row_ids: Vec<u64> = self.durations.iter().map(ui::Duration::row_id).collect();
                    let mut remove_ix = None;
                    let mut defer_focus_to_row_start = None;
                    for (ix, duration) in self.durations.iter_mut().enumerate() {
                        if let Some(target_row_id) = defer_focus_to_row_start.take() {
                            request_digitwise_editor_focus(ui.ctx(), egui::Id::new((target_row_id, "start")).with("hour"), 0);
                        }
                        let row_layout = if content_width < 180.0 {
                            Layout::top_down(Align::Min)
                        } else {
                            Layout::left_to_right(Align::Center)
                        };
                        let time_output = ui.with_layout(row_layout, |ui| {
                            let output = duration.ui(ui);
                            if ui.add(egui::Button::new("×").corner_radius(10.0)).on_hover_text("Remove duration").clicked() {
                                remove_ix = Some(ix);
                            }
                            output
                        }).inner;
                        let focus_note = matches!(time_output.focus_transfer, Some(transfer)
                            if transfer.direction == DigitwiseEditorFocusDirection::Next && transfer.trigger == DigitwiseEditorFocusTrigger::Tab);
                        let note_output = duration.note_ui(ui, focus_note, ix + 1 < row_ids.len());
                        let transfer = if focus_note { note_output.focus_transfer } else { time_output.focus_transfer.or(note_output.focus_transfer) };
                        if let Some(transfer) = transfer {
                            match (transfer.direction, transfer.trigger) {
                                (DigitwiseEditorFocusDirection::Next, DigitwiseEditorFocusTrigger::Tab) => {
                                    if let Some(next_row_id) = row_ids.get(ix + 1) {
                                        request_digitwise_editor_focus(ui.ctx(), egui::Id::new((*next_row_id, "start")).with("hour"), 0);
                                    }
                                }
                                (DigitwiseEditorFocusDirection::Next, DigitwiseEditorFocusTrigger::TypedCompletion) => {
                                    if let Some(next_row_id) = row_ids.get(ix + 1) {
                                        defer_focus_to_row_start = Some(*next_row_id);
                                    }
                                }
                                (DigitwiseEditorFocusDirection::Previous, DigitwiseEditorFocusTrigger::Tab) => {
                                    if ix > 0 {
                                        ui.memory_mut(|memory| {
                                            let note_id = egui::Id::new((row_ids[ix - 1], "note"));
                                            memory.request_focus(note_id);
                                            memory.set_focus_lock_filter(note_id, egui::EventFilter {
                                                tab: true, horizontal_arrows: true, vertical_arrows: true, ..Default::default()
                                            });
                                        });
                                    }
                                }
                                (DigitwiseEditorFocusDirection::Previous, DigitwiseEditorFocusTrigger::TypedCompletion) => {}
                            }
                        }
                    }
                    // We assume only 1 remove button could have been clicked during the loop
                    if let Some(my_ix) = remove_ix {
                        self.durations.remove(my_ix);
                    }
                }
            });
            // Measure before adding blank space, so the shared height can
            // decrease when entries are removed or the layout gets wider.
            let natural_height = ui.min_size().y + margins.y;
            ui.expand_to_include_rect(egui::Rect::from_min_size(
                ui.min_rect().min,
                egui::vec2(content_width, (outer_height - margins.y).max(0.0)),
            ));
            natural_height
        })
    }
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Datelike, NaiveDate, Utc, Weekday};

    fn render_note_test(ctx: &egui::Context, day: &mut super::Day, events: Vec<egui::Event>) {
        // A settled frame installs egui's focus filter on a newly focused
        // widget, as happens between keyboard events in the running app.
        for events in [events, vec![]] {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(600.0, 700.0))),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        day.ui(ui, 230.0, 0.0, "");
                    });
                },
            );
        }
    }

    fn tab(shift: bool) -> Vec<egui::Event> {
        vec![egui::Event::Key {
            key: egui::Key::Tab,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: if shift { egui::Modifiers::SHIFT } else { egui::Modifiers::NONE },
        }]
    }

    #[test]
    fn keyboard_navigation_includes_notes_and_preserves_time_completion() {
        use crate::ui::digitwise_number_editor::request_digitwise_editor_focus;
        let ctx = egui::Context::default();
        let mut day = super::Day::new("Monday".to_owned());
        day.durations = vec![crate::ui::Duration::default(), crate::ui::Duration::default()];
        let first_id = day.durations[0].row_id();
        let second_id = day.durations[1].row_id();
        let end_id = egui::Id::new((first_id, "end")).with("minute");
        let editor = |row, clock, field| egui::Id::new(egui::Id::new((row, clock)).with(field)).with("editor");
        request_digitwise_editor_focus(&ctx, end_id, 1);
        render_note_test(&ctx, &mut day, vec![]);
        render_note_test(&ctx, &mut day, tab(false));
        assert!(
            ctx.memory(|m| m.has_focus(day.durations[0].note_id())),
            "focused={:?}, note={:?}",
            ctx.memory(|m| m.focused()),
            day.durations[0].note_id()
        );
        render_note_test(&ctx, &mut day, vec![egui::Event::Text("PROJ-123".to_owned())]);
        assert_eq!(day.durations[0].note(), "PROJ-123");
        render_note_test(
            &ctx,
            &mut day,
            vec![egui::Event::Key {
                key: egui::Key::Z,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers {
                    ctrl: true,
                    command: true,
                    ..Default::default()
                },
            }],
        );
        assert_eq!(day.durations[0].note(), "");
        render_note_test(&ctx, &mut day, vec![egui::Event::Text("PROJ-123".to_owned())]);
        render_note_test(&ctx, &mut day, tab(false));
        assert!(ctx.memory(|m| m.has_focus(editor(second_id, "start", "hour"))));
        render_note_test(&ctx, &mut day, tab(true));
        assert!(
            ctx.memory(|m| m.has_focus(day.durations[0].note_id())),
            "focused={:?}, note={:?}",
            ctx.memory(|m| m.focused()),
            day.durations[0].note_id()
        );
        render_note_test(&ctx, &mut day, tab(true));
        assert!(ctx.memory(|m| m.has_focus(editor(first_id, "end", "minute"))));
        // Render the deferred digit selection, then type the current last digit.
        render_note_test(&ctx, &mut day, vec![]);
        let digit = (day.durations[0].end_clock().minute() % 10).to_string();
        render_note_test(&ctx, &mut day, vec![egui::Event::Text(digit)]);
        assert!(ctx.memory(|m| m.has_focus(editor(second_id, "start", "hour"))));
        request_digitwise_editor_focus(&ctx, egui::Id::new((second_id, "end")).with("minute"), 1);
        render_note_test(&ctx, &mut day, vec![]);
        render_note_test(&ctx, &mut day, tab(false));
        assert!(ctx.memory(|m| m.has_focus(day.durations[1].note_id())));
        render_note_test(&ctx, &mut day, tab(true));
        assert!(ctx.memory(|m| m.has_focus(editor(second_id, "end", "minute"))));
    }

    #[test]
    fn adding_a_duration_copies_only_the_previous_note() {
        let mut day = super::Day::new("Monday".to_owned());
        day.add_duration("PROJ-123");
        assert_eq!(day.durations[0].note(), "PROJ-123");
        day.durations[0] = day.durations[0]
            .clone()
            .with_metadata(serde_json::json!({"note": "PROJ-456", "source": "import"}));
        day.add_duration("older note");
        assert_eq!(day.durations[1].note(), "PROJ-456");
        assert_ne!(day.durations[0].row_id(), day.durations[1].row_id());
        assert!(day.durations[1].metadata().get("source").is_none());
        day.durations[1].set_note(String::new());
        day.add_duration("older nonempty note");
        assert_eq!(day.durations[2].note(), "");
    }

    #[test]
    fn test_date() {
        let now_utc: DateTime<Utc> = Utc::now();
        println!("now_utc: {}", now_utc);
        let today = now_utc.date_naive();
        println!("today: {}", today);

        let week_nr = today.iso_week().week();
        println!("week_nr: {}", week_nr);

        let week_day = today.weekday();
        println!("week_day: {}", week_day);

        let my_year = 2026;
        let my_week_nr = 10;
        let my_week_nr_date = NaiveDate::from_isoywd_opt(my_year, my_week_nr, Weekday::Mon).unwrap();
        println!("my_week_nr_date: {}", my_week_nr_date);
    }
}
