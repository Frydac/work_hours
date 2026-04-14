#![allow(dead_code)]

use std::collections::HashMap;

use crate::ui;
use anyhow::Result;
use chrono::{Datelike, NaiveDate, Weekday};
use time::format_description::BorrowedFormatItem;
use time::macros::format_description;

#[allow(unused)]
const FORMAT: &[BorrowedFormatItem<'_>] = format_description!("[hour]:[minute]");

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, PartialEq)]
pub struct State {
    days: Vec<ui::Day>,

    all_days: HashMap<NaiveDate, ui::Day>,

    cur_week_nr: u32,
    cur_year: i32,
}

fn current_work_week_monday() -> NaiveDate {
    let today = chrono::Local::now().date_naive();
    let week_nr = today.iso_week().week();
    let year = today.year();
    NaiveDate::from_isoywd_opt(year, week_nr, Weekday::Mon).unwrap()
}

fn current_iso_week_and_year() -> (u32, i32) {
    let today = chrono::Local::now().date_naive();
    let iso_week = today.iso_week();
    (iso_week.week(), iso_week.year())
}

fn last_iso_week_of_year(year: i32) -> u32 {
    NaiveDate::from_ymd_opt(year, 12, 28) // always in last ISO week
        .unwrap()
        .iso_week()
        .week()
}

impl State {
    fn populate_missing_dates(&mut self) {
        let monday = current_work_week_monday();

        for (day_ix, day) in self.days.iter_mut().enumerate() {
            // Older persisted state did not contain a date field, so serde filled
            // it with the type default. Rehydrate those entries from the work-week index.
            if day.date.year() < 2000 {
                day.date = monday + chrono::Duration::days(day_ix as i64);
            }
        }
    }

    fn set_current_week(&mut self, week_nr: u32, year: i32) -> Result<()> {
        self.save_current_week();
        self.cur_week_nr = week_nr;
        self.cur_year = year;
        let cur_monday = NaiveDate::from_isoywd_opt(year, week_nr, Weekday::Mon).ok_or(anyhow::anyhow!("invalid date"))?;
        self.days = (0..5)
            .map(|day_ix| {
                let date = cur_monday + chrono::Duration::days(day_ix);
                let mut day = ui::Day::new(date.format("%A").to_string());
                day.date = date;
                self.all_days.entry(date).or_insert(day).clone()
            })
            .collect();
        Ok(())
    }

    fn save_current_week(&mut self) {
        for day in self.days.iter_mut() {
            self.all_days.insert(day.date, day.clone());
        }
    }

    fn normalize_iso_year_week(mut year: i32, mut week_nr: i32) -> (i32, u32) {
        loop {
            let max_week = last_iso_week_of_year(year) as i32;
            if week_nr < 1 {
                year -= 1;
                week_nr += last_iso_week_of_year(year) as i32;
                continue;
            }
            if week_nr > max_week {
                week_nr -= max_week;
                year += 1;
                continue;
            }
            return (year, week_nr as u32);
        }
    }

    fn set_current_week_normalized(&mut self, year: i32, week_nr: i32) {
        let (year, week_nr) = Self::normalize_iso_year_week(year, week_nr);
        let _ = self.set_current_week(week_nr, year);
    }

    fn shift_weeks(&mut self, nr_weeks: i32) {
        let monday = NaiveDate::from_isoywd_opt(self.cur_year, self.cur_week_nr, Weekday::Mon).unwrap();
        // This should skip years properly
        let next = monday + chrono::Duration::weeks(nr_weeks.into());
        let week = next.iso_week();
        let _ = self.set_current_week(week.week(), week.year());
    }

    fn jump_to_current_week(&mut self) {
        let (week_nr, year) = current_iso_week_and_year();
        let _ = self.set_current_week(week_nr, year);
    }
}

impl Default for State {
    fn default() -> Self {
        let mut res = State {
            days: vec![],
            all_days: HashMap::new(),
            cur_week_nr: 0,
            cur_year: 0,
        };
        let (cur_week_nr, cur_year) = current_iso_week_and_year();
        let _ = res.set_current_week(cur_week_nr, cur_year);
        res
    }
}

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(Default, serde::Deserialize, serde::Serialize)]
// #[derive(serde::Deserialize, serde::Serialize)]
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
            let mut app: Self = eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
            app.state.populate_missing_dates();
            return app;
        }

        Default::default()
    }
}

impl TemplateApp {
    pub fn duration(&self) -> time::Duration {
        self.state.days.iter().fold(time::Duration::ZERO, |sum, day| sum + day.duration())
    }

    pub fn total_target(&self) -> time::Duration {
        self.state.days.iter().fold(time::Duration::ZERO, |sum, day| sum + day.target())
    }

    pub fn current_week_number(&self) -> u32 {
        chrono::Local::now().date_naive().iso_week().week()
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
                        if ui.button("Reset state").clicked() {
                            *self = TemplateApp::default();
                        }
                        ui.separator();
                        if ui.button("Quit").clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                    ui.add_space(16.0);
                }

                egui::widgets::global_theme_preference_buttons(ui);
            });
        });
        egui::TopBottomPanel::top("top_panel_2").show(ctx, |ui| {
            let can_undo = self.undoer.has_undo(&self.state);
            let can_redo = self.undoer.has_redo(&self.state);

            egui::Frame::new().inner_margin(egui::Margin::same(5)).show(ui, |ui| {
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

                    ui.separator();

                    let mut year = self.state.cur_year;
                    ui.label("Year:");
                    if ui.add(egui::DragValue::new(&mut year).speed(1)).changed() {
                        self.state.set_current_week_normalized(year, self.state.cur_week_nr as i32);
                    }

                    let mut week_nr = self.state.cur_week_nr as i32;
                    ui.label("Week:");
                    if ui.add(egui::DragValue::new(&mut week_nr).speed(1).range(1..=999)).changed() {
                        self.state.set_current_week_normalized(self.state.cur_year, week_nr);
                    }

                    if ui.button("<").clicked() {
                        self.state.shift_weeks(-1);
                    }
                    if ui.button(">").clicked() {
                        self.state.shift_weeks(1);
                    }
                    if ui.button("This week").clicked() {
                        self.state.jump_to_current_week();
                    }
                });
            });

            self.undoer.feed_state(ui.ctx().input(|input| input.time), &self.state);
            // let _ = ui.button("test");
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            // The central panel the region left after adding TopPanel's and SidePanel's
            // ui.heading("Work Hours Calculator");
            // ui.horizontal(|ui| {
            //     if ui.button("Reset state").clicked() {
            //         *self = TemplateApp::default();
            //     }
            //     //     ui.spacing_mut().item_spacing.x = 0.0;
            //     //     self.state.single_digit.ui(ui);
            //     // self.state.test.ui(ui);
            // });

            // ui.with_layout(Layout::top_down(Align::Center).with_main_wrap(true), |ui| {
            // ui.heading(RichText::new("Work Hours Calculator").strong());
            // ui.separator();

            // ui.horizontal(|ui| {
            //     ui.add_space(
            //         ui.available_width() / 2.0
            //             - ui.fonts(|f| {
            //                 f.layout("Work Hours Calculator".to_string(), egui::FontId::proportional(20.0), egui::Color32::WHITE, 100.0)
            //                     .size()
            //                     .x
            //                     / 2.0
            //             }),
            //     );
            //     ui.heading(RichText::new("Work Hours Calculator").strong());
            //     ui.add_space(ui.available_width() - 100.0); // Adjust for button size
            //     if ui.button("Settings").clicked() {
            //         // Handle button click
            //     }
            // });
            // ui.separator();
            // });

            // if ui.add(egui::Button::new("Reset egui memory")).on_hover_text("Forget all").clicked() {
            //     // ui.ctx().memory_mut(|mem| *mem = Default::default());
            //     *self = TemplateApp::default();
            // }

            // ui.with_layout(Layout::left_to_right(Align::TOP).with_main_wrap(true), |ui| {
            ui.horizontal_wrapped(|ui| {
                for day in &mut self.state.days {
                    ui.separator();
                    ui.vertical(|ui| {
                        day.ui(ui);
                        // ui.separator();
                    });
                }
                ui.separator();
            });

            ui.separator();

            egui::Grid::new("total_grid")
                .striped(true)
                .min_col_width(80.0)
                // .max_col_width(200.0)
                .show(ui, |ui| {
                    ui.label("Week:");
                    ui.label(self.current_week_number().to_string());
                    ui.end_row();

                    let duration_days = self.duration();
                    let target_days = self.total_target();
                    ui.label("Week Target:");
                    ui.label(ui::duration::format_duration(target_days, ui::duration::DURATION_FORMAT));
                    ui.end_row();
                    ui.label("Week Total:");
                    ui.label(ui::duration::format_duration(duration_days, ui::duration::DURATION_FORMAT));
                    ui.end_row();
                    ui.label("Week Todo:");
                    let todo = target_days - duration_days;
                    let sign = if todo.is_negative() { "-" } else { "" };
                    ui.label(format!(
                        "{}{}",
                        sign,
                        ui::duration::format_duration(todo.abs(), ui::duration::DURATION_FORMAT)
                    ));
                    // ui.label(format_duration(todo, DURATION_FORMAT));
                    ui.end_row();
                });
            // });
            ui.separator();

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

#[cfg(test)]
mod tests {
    use super::State;

    #[test]
    fn default_state_has_week_target() {
        let state = State::default();
        let total_target = state.days.iter().fold(time::Duration::ZERO, |sum, day| sum + day.target());
        assert_eq!(total_target, time::Duration::hours(38));
    }

    #[test]
    fn normalize_iso_year_week_keeps_valid_week() {
        assert_eq!(State::normalize_iso_year_week(2026, 10), (2026, 10));
    }

    #[test]
    fn normalize_iso_year_week_wraps_forward() {
        let max_week = super::last_iso_week_of_year(2026) as i32;
        assert_eq!(State::normalize_iso_year_week(2026, max_week + 1), (2027, 1));
    }

    #[test]
    fn normalize_iso_year_week_wraps_backward() {
        let previous_year = 2025;
        let last_week = super::last_iso_week_of_year(previous_year);
        assert_eq!(State::normalize_iso_year_week(2026, 0), (previous_year, last_week));
    }

    #[test]
    fn normalize_iso_year_week_carries_forward_when_year_has_fewer_weeks() {
        assert_eq!(State::normalize_iso_year_week(2025, 53), (2026, 1));
    }
}
