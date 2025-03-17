use egui;
use std::ops;

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, PartialEq)]
pub struct SingleDigit {
    pub value: u8,
    range: ops::RangeInclusive<u8>,
    surrender_focus: bool,
    delta: f32,
    name: String,
}

fn select_all_text(ui: &mut egui::Ui, text_edit_output: &mut egui::widgets::text_edit::TextEditOutput, val_str: &str) {
    // dbg!("select_all_text");
    text_edit_output.state.cursor.set_char_range(Some(egui::text::CCursorRange::two(
        egui::text::CCursor::new(0),
        egui::text::CCursor::new(val_str.len()),
    )));
    // don't forget to do apply changes
    text_edit_output.state.clone().store(ui.ctx(), text_edit_output.response.id);
}

impl SingleDigit {
    pub fn new(value: u8, name: &str) -> SingleDigit {
        SingleDigit {
            value,
            range: 0..=9,
            surrender_focus: true,
            delta: 0.0,
            name: name.into(),
        }
    }
    /// Valid range for value
    pub fn range(mut self, range: ops::RangeInclusive<u8>) -> Self {
        self.range = range;
        self.value = self.value.clamp(*self.range.start(), *self.range.end());
        self
    }
    /// When a valid number is entered, surrender focus (most likely to the next singe digit),
    pub fn surrender_focus(mut self, surrender_focus: bool) -> Self {
        self.surrender_focus = surrender_focus;
        self
    }

    pub fn clamp_value(&mut self)
    {
        self.value = self.value.clamp(*self.range.start(), *self.range.end())
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) -> egui::Response {
        let mut val_str = self.value.to_string();
        let mut output = egui::TextEdit::singleline(&mut val_str).desired_width(6.0).show(ui);
        // output.&output.text_clip_rect

        // let canvas_rect = ui.max_rect();
        // let painter = ui.painter_at(canvas_rect);
        // painter.rect(output.response.interact_rect, egui::CornerRadius::from(0), egui::Color32::TRANSPARENT, egui::Stroke::new(1.0, egui::Color32::GREEN), egui::StrokeKind::Outside);

        // if ui.rect_contains_pointer(ui.min_rect()) {
        if ui.rect_contains_pointer(output.response.interact_rect) {
            println!("contains pointer: {}", self.name);
            ui.ctx().input(|i| {
                for event in &i.events {
                    match event {
                        egui::Event::MouseWheel { unit: _, delta, modifiers: _ } => {
                            println!("{}: {}: {}", self.name, "delta", delta);
                            // dbg!(unit);
                            // dbg!(delta);
                            // dbg!(modifiers);
                            self.delta += delta.y;
                            // println!("{}: {}", "self.delta", self.delta);
                            if self.delta.abs() >= 1.0 {
                                let mut value: i16 = self.value as i16;
                                value += self.delta as i16;
                                value = value.clamp(*self.range.start() as i16, *self.range.end() as i16);
                                self.value = value as u8;
                                // println!("{}: {}", "self.value", self.value);
                                // println!("{}: {}", "self.value after clamp", self.value);
                                self.delta -=  self.delta.signum();
                            }
                        }
                        // egui::Event::MouseWheel(v) => {
                        //     println!("{:?}: {:?}", "v", v);
                        // }
                        egui::Event::Zoom(v) => {
                            println!("{:?}: {:?}", "v", v);
                        }
                        _ => {
                        },
                    }
                    // if let egui::Event::Scroll(v) = event {
                    //     println!("{:?}: {:?}", "v", v);
                    //     // println!("Scroll: {}", v);
                    // }
                }
            });
        }

        // if output.response.

        // NOTE: gained_focus() didn't always work here, while now we reselect the whole time.
        // Maybe check and only reselect when necessary.. the store uses a mutex I think
        if output.response.has_focus() {
            select_all_text(ui, &mut output, &val_str);
        }
        if output.response.changed() {
            if let Ok(val) = val_str.parse::<u8>() {
                if self.range.contains(&val) {
                    self.value = val;
                    if self.surrender_focus {
                        output.response.surrender_focus();
                    }
                } else {
                    // valid u8, but not in range, don't change and reselect text
                    select_all_text(ui, &mut output, &val_str);
                }
            } else {
                // invalid u8, don't change and reselect text
                select_all_text(ui, &mut output, &val_str);
            }
        }

        output.response
    }
}
