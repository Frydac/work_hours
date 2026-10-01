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

        week_scroll_area(ui, |ui| {
            render_day_cards(ui, &mut app.state);
            render_week_summary(app, ui);
            ui.separator();
            powered_by_egui_and_eframe(ui);
            egui::warn_if_debug_build(ui);
        });
    });
}

fn week_scroll_area<R>(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui) -> R) -> egui::scroll_area::ScrollAreaOutput<R> {
    // Reserve the full scrollbar gutter even before it appears. This keeps
    // columns stable and prevents a floating scrollbar from covering a card.
    let scroll = ui.spacing().scroll;
    let gutter = scroll.bar_inner_margin + scroll.bar_width + scroll.bar_outer_margin;
    let content_width = (ui.available_width() - gutter).max(0.0);
    egui::ScrollArea::vertical()
        .id_salt("week_content")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.set_width(content_width.min(ui.available_width()));
            add_contents(ui)
        })
}

const DAY_CARD_WIDTH: f32 = 230.0;
const DAY_CARD_GAP: f32 = 8.0;

fn day_card_columns(available_width: f32, day_count: usize) -> usize {
    (((available_width + DAY_CARD_GAP) / (DAY_CARD_WIDTH + DAY_CARD_GAP)).floor() as usize)
        .max(1)
        .min(day_count.max(1))
}

/// Allocate complete cards before rendering their contents. All cards share the
/// same parent UI, so their date-based IDs stay stable when they move between rows.
fn render_day_cards(ui: &mut egui::Ui, state: &mut super::state::State) -> Vec<egui::Rect> {
    let available_width = ui.available_width();
    let day_count = state.days().len();
    let columns = day_card_columns(available_width, day_count);
    let width = DAY_CARD_WIDTH.min(available_width).max(0.0);
    let height_id = ui.id().with("day_card_height");
    let height = ui.ctx().data(|data| data.get_temp::<f32>(height_id)).unwrap_or(0.0);
    let mut natural_height = 0.0_f32;
    let origin = ui.next_widget_position();
    let mut top = origin.y;
    let mut bounds = egui::Rect::NOTHING;
    let mut cards = Vec::with_capacity(day_count);
    for row_start in (0..day_count).step_by(columns) {
        let mut bottom = top;
        for column in 0..columns.min(day_count - row_start) {
            let index = row_start + column;
            let previous_note = state.previous_duration_note(state.days()[index].date).to_owned();
            let day = &mut state.days_mut()[index];
            let position = egui::pos2(origin.x + column as f32 * (width + DAY_CARD_GAP), top);
            let rect = egui::Rect::from_min_size(position, egui::vec2(width, 0.0));
            let response = ui.allocate_new_ui(
                egui::UiBuilder::new()
                    .id_salt(("day_card", day.date))
                    .max_rect(rect)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
                |ui| day.ui(ui, width, height, &previous_note),
            );
            natural_height = natural_height.max(response.inner.inner);
            let rect = response.inner.response.rect;
            bottom = bottom.max(rect.bottom());
            bounds = bounds.union(rect);
            cards.push(rect);
        }
        top = bottom + DAY_CARD_GAP;
    }
    if day_count > 0 {
        ui.advance_cursor_after_rect(bounds);
        if (natural_height - height).abs() > 0.1 {
            ui.ctx().data_mut(|data| data.insert_temp(height_id, natural_height));
            // Redraw in the same frame using the measured maximum. Only size
            // changes need another pass; normal frames render once.
            ui.ctx().request_discard("day cards changed height");
        }
    }
    cards
}

fn render_week_summary(app: &TemplateApp, ui: &mut egui::Ui) {
    ui.separator();

    let todo = app.total_target() - app.duration();
    let sign = if todo.is_negative() { "-" } else { "" };
    let totals = [
        (
            "Week Target:",
            duration::format_duration(app.total_target(), duration::DURATION_FORMAT),
        ),
        ("Week Total:", duration::format_duration(app.duration(), duration::DURATION_FORMAT)),
        (
            "Week Todo:",
            format!("{}{}", sign, duration::format_duration(todo.abs(), duration::DURATION_FORMAT)),
        ),
    ];
    if ui.available_width() >= 180.0 {
        egui::Grid::new("total_grid").striped(true).min_col_width(80.0).show(ui, |ui| {
            for (label, value) in totals {
                ui.label(label);
                ui.label(value);
                ui.end_row();
            }
        });
    } else {
        for (label, value) in totals {
            ui.horizontal_wrapped(|ui| {
                ui.label(label);
                ui.label(value);
            });
        }
    }
}

fn powered_by_egui_and_eframe(ui: &mut egui::Ui) {
    ui.horizontal_wrapped(|ui| {
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
    use super::*;
    use crate::ui::digitwise_number_editor::request_digitwise_editor_focus;

    #[test]
    fn columns_change_only_when_complete_cards_fit() {
        for columns in 2..=5 {
            let exact = columns as f32 * DAY_CARD_WIDTH + (columns - 1) as f32 * DAY_CARD_GAP;
            assert_eq!(day_card_columns(exact - 0.5, 5), columns - 1);
            assert_eq!(day_card_columns(exact, 5), columns);
            assert_eq!(day_card_columns(exact + 0.5, 5), columns);
        }
        assert_eq!(day_card_columns(0.0, 5), 1);
        assert_eq!(day_card_columns(150.0, 5), 1);
        assert_eq!(day_card_columns(2000.0, 5), 5);
    }

    #[test]
    fn cards_grow_and_shrink_together_in_the_same_frame() {
        let ctx = egui::Context::default();
        let mut app = TemplateApp::default();
        for day in app.state.days_mut() {
            day.durations.clear();
        }
        let render = |app: &mut TemplateApp, width: f32| {
            let mut cards = Vec::new();
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 600.0))),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        week_scroll_area(ui, |ui| {
                            cards = render_day_cards(ui, &mut app.state);
                        });
                    });
                },
            );
            assert!(output.platform_output.num_completed_passes <= 2);
            for rect in &cards {
                assert!((rect.height() - cards[0].height()).abs() < 0.1, "cards={cards:?}");
            }
            (cards[0].height(), output.platform_output.num_completed_passes)
        };
        let (empty_height, _) = render(&mut app, 1250.0);
        app.state.days_mut()[4].durations = (0..8).map(|_| crate::ui::Duration::default()).collect();
        let (full_height, _) = render(&mut app, 1250.0);
        assert!(full_height > empty_height);
        let (narrow_height, _) = render(&mut app, 190.0);
        assert!(narrow_height > full_height);
        let (wide_height, _) = render(&mut app, 1250.0);
        assert!((wide_height - full_height).abs() < 0.1);
        app.state.days_mut()[4].durations.clear();
        let (cleared_height, _) = render(&mut app, 1250.0);
        assert!((cleared_height - empty_height).abs() < 0.1);
        let (_, passes) = render(&mut app, 1250.0);
        assert_eq!(passes, 1, "unchanged cards should not trigger another sizing pass");
    }

    #[test]
    fn rendered_cards_fit_and_focus_survives_resizing() {
        let ctx = egui::Context::default();
        let mut app = TemplateApp::default();
        for (index, day) in app.state.days_mut().iter_mut().enumerate() {
            day.enabled = index != 4;
            day.durations = (0..index + 1)
                .map(|_| {
                    let mut duration = crate::ui::Duration::default();
                    duration.set_note("PROJ-123 — a long note that must scroll inside the editor rather than widen the card".to_owned());
                    duration
                })
                .collect();
        }
        let row_id = app.state.days()[4].durations[0].row_id();
        let hour_id = egui::Id::new((row_id, "start")).with("hour");
        request_digitwise_editor_focus(&ctx, hour_id, 0);
        for dark in [true, false] {
            if !dark {
                ctx.memory_mut(|memory| memory.request_focus(app.state.days()[4].durations[0].note_id()));
            }
            ctx.set_visuals(if dark { egui::Visuals::dark() } else { egui::Visuals::light() });
            for width in [1250.0, 1000.0, 967.5, 968.0, 968.5, 760.0, 520.0, 260.0, 190.0, 140.0] {
                // Reuse the context and render several frames to include cached
                // widget sizes and scrollbar animation during continuous resize.
                for frame in 0..3 {
                    let input = egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 220.0))),
                        time: Some(width as f64 + frame as f64),
                        ..Default::default()
                    };
                    let _ = ctx.run(input, |ctx| {
                        egui::CentralPanel::default().show(ctx, |ui| {
                            let output = week_scroll_area(ui, |ui| {
                                let viewport = ui.max_rect();
                                let columns = day_card_columns(ui.available_width(), app.state.days().len());
                                let cards = render_day_cards(ui, &mut app.state);
                                for (index, rect) in cards.iter().enumerate() {
                                    if !ui.ctx().will_discard() {
                                        assert!((rect.height() - cards[0].height()).abs() < 0.1, "cards={cards:?}");
                                    }
                                    assert!(rect.left() >= viewport.left() - 0.1);
                                    assert!(
                                        rect.right() <= viewport.right() + 0.1,
                                        "width={width}: {rect:?}, viewport={viewport:?}"
                                    );
                                    if index % columns != 0 {
                                        assert_eq!(rect.top(), cards[index - 1].top());
                                        assert!(
                                            rect.left() >= cards[index - 1].right() + DAY_CARD_GAP - 0.1,
                                            "width={width}, cards={cards:?}"
                                        );
                                    } else if index >= columns {
                                        let previous_row_bottom =
                                            cards[index - columns..index].iter().map(|r| r.bottom()).fold(0.0, f32::max);
                                        assert!(rect.top() >= previous_row_bottom + DAY_CARD_GAP - 0.1);
                                    }
                                }
                                let bottom = cards.iter().map(|r| r.bottom()).fold(0.0, f32::max);
                                render_week_summary(&app, ui);
                                assert!(ui.min_rect().bottom() > bottom);
                            });
                            assert!(output.content_size.y > output.inner_rect.height());
                        });
                    });
                    let focused_id = if dark {
                        egui::Id::new(hour_id).with("editor")
                    } else {
                        app.state.days()[4].durations[0].note_id()
                    };
                    assert!(ctx.memory(|m| m.has_focus(focused_id)));
                }
            }
        }
    }
}
