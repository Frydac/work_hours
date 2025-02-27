#![allow(dead_code)]

use egui::{Align, Layout, RichText};
use time;
use time::format_description::BorrowedFormatItem;
use time::macros::format_description;

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Duration {
    start: time::OffsetDateTime,
    end: time::OffsetDateTime,
}

#[allow(unused)]
const FORMAT: &[BorrowedFormatItem<'_>] = format_description!("[hour]:[minute]");

impl Default for Duration {
    fn default() -> Self {
        let now = time::OffsetDateTime::now_local().unwrap();
        Self { start: now, end: now }
    }
}

fn format_duration(duration: time::Duration, format: &str) -> String {
    let total_seconds = duration.whole_seconds();
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    format
        .replace("%H", &format!("{:02}", hours))
        .replace("%M", &format!("{:02}", minutes))
        .replace("%S", &format!("{:02}", seconds))
}

const DURATION_FORMAT: &str = "%H:%M";

impl Duration {
    // Trying to have 1 value for both hours and minutes, and then format it differently.
    // issue seem to arise when parsing hours, we don't know the minutes? maybe take calculte it?
    // pub fn ui2(&mut self, ui: &mut egui::Ui) {
    //     let Self { start, end } = &self;
    //     ui.horizontal(|ui| {
    //         let range = 0..=(60 * 24 - 1);

    //         let mut start_min = start.hour() as u16 * 60 + start.minute() as u16;
    //         let mut end_min = end.hour() as u16 * 60 + end.minute() as u16;
    //         ui.add(egui::DragValue::new(&mut start_min)
    //             .range(range)
    //             .custom_formatter(|n, _| {
    //                 let n = n as i32;
    //                 let hours = n / 60;
    //                 format!("{hours:02}")
    //             })
    //             .custom_parser(|s| {
    //                 if
    //             })
    //             );
    //     });
    // }
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let mut start_h = self.start.hour();
            let mut start_m = self.start.minute();
            let mut end_h = self.end.hour();
            let mut end_m = self.end.minute();

            ui.spacing_mut().button_padding.x = 0.0;
            ui.spacing_mut().interact_size.x *= 0.6;
            ui.spacing_mut().item_spacing.x = 5.0;

            ui.add(egui::DragValue::new(&mut start_h).range(0..=23));
            ui.label(":");
            ui.add(egui::DragValue::new(&mut start_m).range(0..=59));
            ui.label("->");
            ui.add(egui::DragValue::new(&mut end_h).range(0..=23));
            ui.label(":");
            ui.add(egui::DragValue::new(&mut end_m).range(0..=59));
            let mut start_adjusted = false;
            let mut end_adjusted = false;
            if start_h != self.start.hour() {
                self.start = self.start.replace_hour(start_h).unwrap();
                start_adjusted = true;
            } else if start_m != self.start.minute() {
                self.start = self.start.replace_minute(start_m).unwrap();
                start_adjusted = true;
            } else if end_h != self.end.hour() {
                self.end = self.end.replace_hour(end_h).unwrap();
                end_adjusted = true;
            } else if end_m != self.end.minute() {
                self.end = self.end.replace_minute(end_m).unwrap();
                end_adjusted = true;
            }

            if start_adjusted && self.start > self.end {
                self.end = self.start
            } else if end_adjusted && self.end < self.start {
                self.start = self.end
            }
        });
    }

    pub fn duration(&self) -> time::Duration {
        self.end - self.start
    }
}

#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Day {
    pub durations: Vec<Duration>,
    pub name: String,
    pub total_target: time::Duration,
}

impl Day {
    pub fn duration(&self) -> time::Duration {
        let mut duration = time::Duration::ZERO;
        for dur in &self.durations {
            duration += dur.duration();
        }
        duration
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        egui::Frame::default()
            // .stroke(egui::Stroke::new(1.0, egui::Color32::RED))
            // .inner_margin(egui::Margin::same(10.0))
            .show(ui, |ui| {
                ui.set_width(170.0);
                // Center-aligned text
                ui.with_layout(Layout::top_down(Align::Center), |ui| {
                    ui.label(RichText::new(&self.name).size(16.0));

                    ui.separator();

                    ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
                        let add_button = egui::Button::new("Add +"); // Create the button instance
                        if ui.add(add_button).on_hover_text(format!("Add a new duration to {}", self.name)).clicked() {
                            self.durations.push(Duration::default());
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

                    {
                        let mut remove_ix = None;
                        for (ix, duration) in &mut self.durations.iter_mut().enumerate() {
                            if ix == 0 {
                                ui.separator();
                            }
                            ui.horizontal(|ui| {
                                // add duration
                                duration.ui(ui);
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

                    ui.separator();

                    egui::Grid::new(&self.name)
                        .striped(true)
                        .min_col_width(80.0)
                        // .max_col_width(200.0)
                        .show(ui, |ui| {
                            ui.label("Total:");
                            ui.label(format_duration(self.duration(), DURATION_FORMAT));
                            ui.end_row();
                            ui.label("Target:");
                            ui.label(format_duration(self.total_target, DURATION_FORMAT));
                            ui.end_row();
                            ui.label("Todo:");
                            let todo = self.total_target - self.duration();
                            let sign = if todo.is_negative() { "-" } else { "" };
                            ui.label(format!("{}{}", sign, format_duration(todo.abs(), DURATION_FORMAT)));
                            // ui.label(format_duration(todo, DURATION_FORMAT));
                            ui.end_row();
                        });
                });

                // Add the margin around the label
                // ui.add(margin, egui::Label::new("Hello, egui!"));
            });
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, PartialEq)]
pub struct State {
    days: Vec<Day>,
}

impl Default for State {
    fn default() -> Self {
        let day_target = time::Duration::hours(7) + time::Duration::minutes(36);

        Self {
            days: ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday"]
                .iter()
                .map(|&name| Day {
                    name: name.to_owned(),
                    durations: vec![],
                    total_target: day_target,
                })
                .collect(),
        }
    }
}

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct TemplateApp {
    pub state: State,
    pub undoer: egui::util::undoer::Undoer<State>,
}

impl TemplateApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        if let Some(storage) = cc.storage {
            return eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
        }

        Default::default()
    }
}

impl TemplateApp {
    pub fn duration(&self) -> time::Duration {
        self.state.days.iter().fold(time::Duration::ZERO, |sum, day| sum + day.duration())
    }

    pub fn total_target(&self) -> time::Duration {
        self.state.days.iter().fold(time::Duration::ZERO, |sum, day| sum + day.total_target)
    }

    // pub fn
}

impl eframe::App for TemplateApp {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Put your widgets into a `SidePanel`, `TopBottomPanel`, `CentralPanel`, `Window` or `Area`.
        // For inspiration and more examples, go to https://emilk.github.io/egui

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            // The top panel is often a good place for a menu bar:

            egui::menu::bar(ui, |ui| {
                // NOTE: no File->Quit on web pages!
                let is_web = cfg!(target_arch = "wasm32");
                if !is_web {
                    ui.menu_button("File", |ui| {
                        if ui.button("Quit").clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                    ui.add_space(16.0);
                }

                egui::widgets::global_theme_preference_buttons(ui);
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            // The central panel the region left after adding TopPanel's and SidePanel's
            // ui.heading("Work Hours Calculator");
            ui.with_layout(Layout::top_down(Align::Center), |ui| {
                ui.heading(RichText::new("Work Hours Calculator").strong());
            });

            // if ui.add(egui::Button::new("Reset egui memory")).on_hover_text("Forget all").clicked() {
            //     // ui.ctx().memory_mut(|mem| *mem = Default::default());
            //     *self = TemplateApp::default();
            // }

            // ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
            ui.horizontal(|ui| {
                for day in &mut self.state.days {
                    ui.vertical(|ui| {
                        day.ui(ui);
                    });
                }
            });

            ui.separator();

            egui::Grid::new("total_grid")
                .striped(true)
                .min_col_width(80.0)
                // .max_col_width(200.0)
                .show(ui, |ui| {
                    let duration_days = self.duration();
                    let target_days = self.total_target();
                    ui.label("Total:");
                    ui.label(format_duration(duration_days, DURATION_FORMAT));
                    ui.end_row();
                    ui.label("Target:");
                    ui.label(format_duration(target_days, DURATION_FORMAT));
                    ui.end_row();
                    ui.label("Todo:");
                    let todo = target_days - duration_days;
                    let sign = if todo.is_negative() { "-" } else { "" };
                    ui.label(format!("{}{}", sign, format_duration(todo.abs(), DURATION_FORMAT)));
                    // ui.label(format_duration(todo, DURATION_FORMAT));
                    ui.end_row();
                });
            // });
            ui.separator();

            let can_undo = self.undoer.has_undo(&self.state);
            let can_redo = self.undoer.has_redo(&self.state);

            ui.horizontal(|ui| {
                let undo = ui.add_enabled(can_undo, egui::Button::new("⟲ Undo")).clicked();
                let redo = ui.add_enabled(can_redo, egui::Button::new("⟳ Redo")).clicked();

                if undo {
                    if let Some(prev_state) = self.undoer.undo(&self.state) {
                        self.state = prev_state.clone();
                    }
                }
                if redo {
                    if let Some(redo_state) = self.undoer.redo(&self.state) {
                        self.state = redo_state.clone();
                    }
                }
            });

            self.undoer.feed_state(ui.ctx().input(|input| input.time), &self.state);

            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                powered_by_egui_and_eframe(ui);
                egui::warn_if_debug_build(ui);
            });
        });
    }
}

fn powered_by_egui_and_eframe(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.label("Powered by ");
        ui.hyperlink_to("egui", "https://github.com/emilk/egui");
        ui.label(" and ");
        ui.hyperlink_to("eframe", "https://github.com/emilk/egui/tree/master/crates/eframe");
        ui.label(".");
    });
}
