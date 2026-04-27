// Top-level egui rendering for the application shell. Lower-level widgets such
// as day/time editors stay in `src/ui/*`; this file only arranges the app-wide
// panels and delegates actions back into `TemplateApp`.

use crate::ui::duration;
use egui::{Color32, RichText};

use super::state::current_iso_week_and_year;
use super::TemplateApp;

pub(crate) fn render(app: &mut TemplateApp, ctx: &egui::Context, frame: &mut eframe::Frame) {
    render_menu_bar(app, ctx);
    render_header_bar(app, ctx);
    render_login_window(app, ctx);
    render_main_panel(app, ctx, frame);
}

fn render_menu_bar(app: &mut TemplateApp, ctx: &egui::Context) {
    egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
        egui::menu::bar(ui, |ui| {
            let is_web = cfg!(target_arch = "wasm32");
            if !is_web {
                ui.menu_button("File", |ui| {
                    if ui.button("Reset state").clicked() {
                        app.reset_state();
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
}

fn render_header_bar(app: &mut TemplateApp, ctx: &egui::Context) {
    egui::TopBottomPanel::top("top_panel_2").show(ctx, |ui| {
        let can_undo = app.undoer.has_undo(&app.state);
        let can_redo = app.undoer.has_redo(&app.state);
        let can_change_week = app.sync.can_change_week(&app.state);
        let logged_in = app.sync.is_logged_in();
        let is_busy = app.sync.is_busy();

        egui::Frame::new().inner_margin(egui::Margin::same(5)).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui
                    .button("Reset state")
                    .on_hover_text("Remove all stored data and start fresh. Can't be undone")
                    .clicked()
                {
                    app.reset_state();
                }
                ui.separator();
                let undo = ui.add_enabled(can_undo, egui::Button::new("⟲ Undo")).clicked();
                let redo = ui.add_enabled(can_redo, egui::Button::new("⟳ Redo")).clicked();

                if undo {
                    if let Some(prev_state) = app.undoer.undo(&app.state) {
                        app.state = prev_state.clone();
                    }
                }
                if redo {
                    if let Some(redo_state) = app.undoer.redo(&app.state) {
                        app.state = redo_state.clone();
                    }
                }

                ui.separator();

                let mut year = app.state.cur_year();
                ui.label("Year:");
                if ui.add_enabled(can_change_week, egui::DragValue::new(&mut year).speed(1)).changed() {
                    app.navigate_to_week(ctx.clone(), year, app.state.cur_week_nr() as i32);
                }

                let mut week_nr = app.state.cur_week_nr() as i32;
                ui.label("Week:");
                if ui
                    .add_enabled(can_change_week, egui::DragValue::new(&mut week_nr).speed(1).range(1..=999))
                    .changed()
                {
                    app.navigate_to_week(ctx.clone(), app.state.cur_year(), week_nr);
                }

                if ui.add_enabled(can_change_week, egui::Button::new("<")).clicked() {
                    app.navigate_to_week(ctx.clone(), app.state.cur_year(), app.state.cur_week_nr() as i32 - 1);
                }
                if ui.add_enabled(can_change_week, egui::Button::new(">")).clicked() {
                    app.navigate_to_week(ctx.clone(), app.state.cur_year(), app.state.cur_week_nr() as i32 + 1);
                }
                if ui.add_enabled(can_change_week, egui::Button::new("This week")).clicked() {
                    let (week_nr, year) = current_iso_week_and_year();
                    app.navigate_to_week(ctx.clone(), year, week_nr as i32);
                }

                ui.separator();
                let status = if logged_in {
                    format!("Logged in: {}", app.sync.session_label())
                } else {
                    "Not logged in".to_string()
                };
                ui.label(RichText::new(status).strong());

                if logged_in {
                    if ui
                        .add_enabled(app.sync.can_refresh_week(&app.state), egui::Button::new("Refresh"))
                        .clicked()
                    {
                        app.request_visible_week_load(ctx.clone());
                    }
                    if ui
                        .add_enabled(
                            app.sync.is_week_dirty(&app.state) && app.sync.in_flight_save_week().is_none(),
                            egui::Button::new("Save"),
                        )
                        .clicked()
                    {
                        app.save_visible_week(ctx.clone());
                    }
                    if ui.add_enabled(!is_busy, egui::Button::new("Log out")).clicked() {
                        app.logout();
                    }
                } else if ui
                    .add_enabled(
                        app.sync.config_available(app.config.as_ref()) && !app.sync.in_flight_auth(),
                        egui::Button::new("Log in"),
                    )
                    .clicked()
                {
                    app.ui_state.set_show_login_window(true);
                }

                if app.sync.is_week_dirty(&app.state) {
                    ui.colored_label(Color32::YELLOW, "Unsaved changes");
                }
            });
        });

        app.undoer.feed_state(ui.ctx().input(|input| input.time), &app.state);
    });
}

fn render_login_window(app: &mut TemplateApp, ctx: &egui::Context) {
    if !app.ui_state.show_login_window() {
        return;
    }

    let mut open = app.ui_state.show_login_window();
    egui::Window::new("Log in to Supabase")
        .collapsible(false)
        .resizable(false)
        .open(&mut open)
        .show(ctx, |ui| {
            ui.label("Email");
            ui.text_edit_singleline(app.ui_state.login_email_mut());
            ui.label("Password");
            ui.add(egui::TextEdit::singleline(app.ui_state.login_password_mut()).password(true));

            if let Some(error) = app.ui_state.error_message() {
                ui.colored_label(Color32::RED, error);
            }

            if ui.add_enabled(!app.sync.in_flight_auth(), egui::Button::new("Log in")).clicked() {
                app.start_login(ctx.clone());
            }
        });
    app.ui_state.set_show_login_window(open);
}

fn render_main_panel(app: &mut TemplateApp, ctx: &egui::Context, _frame: &mut eframe::Frame) {
    egui::CentralPanel::default().show(ctx, |ui| {
        if let Some(status) = app.ui_state.status_message() {
            ui.label(status);
        }
        if let Some(error) = app.ui_state.error_message() {
            ui.colored_label(Color32::RED, error);
        }
        if app.ui_state.status_message().is_some() || app.ui_state.error_message().is_some() {
            ui.separator();
        }

        ui.horizontal_wrapped(|ui| {
            for day in app.state.days_mut() {
                ui.separator();
                ui.vertical(|ui| {
                    day.ui(ui);
                });
            }
            ui.separator();
        });

        ui.separator();

        egui::Grid::new("total_grid").striped(true).min_col_width(80.0).show(ui, |ui| {
            let duration_days = app.duration();
            let target_days = app.total_target();
            ui.label("Week Target:");
            ui.label(duration::format_duration(target_days, duration::DURATION_FORMAT));
            ui.end_row();
            ui.label("Week Total:");
            ui.label(duration::format_duration(duration_days, duration::DURATION_FORMAT));
            ui.end_row();
            ui.label("Week Todo:");
            let todo = target_days - duration_days;
            let sign = if todo.is_negative() { "-" } else { "" };
            ui.label(format!(
                "{}{}",
                sign,
                duration::format_duration(todo.abs(), duration::DURATION_FORMAT)
            ));
            ui.end_row();
        });

        ui.separator();

        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
            powered_by_egui_and_eframe(ui);
            egui::warn_if_debug_build(ui);
        });
    });
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
