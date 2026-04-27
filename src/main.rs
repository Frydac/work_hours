#![warn(clippy::all, rust_2018_idioms)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

// Binary entry point for native and WASM targets. This file is intentionally
// thin: platform-specific startup lives here, while application behavior lives
// in `TemplateApp`.

// When compiling natively:
#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    use tracing::{info, warn};
    use work_hours_calculator::{config::AppConfig, logging::init_tracing, TemplateApp};

    init_tracing();
    info!(target = "startup", platform = "native", "starting app");
    if let Err(err) = AppConfig::load_public() {
        warn!(target = "startup", error = %err, "Supabase public config not available yet");
    }

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([400.0, 300.0])
            .with_min_inner_size([300.0, 220.0])
            .with_icon(
                // NOTE: Adding an icon is optional
                eframe::icon_data::from_png_bytes(&include_bytes!("../assets/icon-256.png")[..]).expect("Failed to load icon"),
            ),
        ..Default::default()
    };
    eframe::run_native(
        "Work Hours Calculator",
        native_options,
        Box::new(|cc| Ok(Box::new(TemplateApp::new(cc)))),
    )
}

// When compiling to web using trunk:
#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;
    use tracing::{info, warn};
    use work_hours_calculator::{config::AppConfig, logging::init_tracing, TemplateApp};

    init_tracing();
    info!(target = "startup", platform = "wasm", "starting app");

    if let Err(err) = AppConfig::load_public() {
        warn!(target = "startup", error = %err, "Supabase public config not available yet");
    }

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window().expect("No window").document().expect("No document");

        let canvas = document
            .get_element_by_id("the_canvas_id")
            .expect("Failed to find the_canvas_id")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("the_canvas_id was not a HtmlCanvasElement");

        let start_result = eframe::WebRunner::new()
            .start(canvas, web_options, Box::new(|cc| Ok(Box::new(TemplateApp::new(cc)))))
            .await;

        // Remove the loading text once egui has taken over the page, or swap it
        // out for a crash message if startup failed.
        if let Some(loading_text) = document.get_element_by_id("loading_text") {
            match start_result {
                Ok(_) => {
                    loading_text.remove();
                }
                Err(e) => {
                    loading_text.set_inner_html("<p> The app has crashed. See the developer console for details. </p>");
                    panic!("Failed to start eframe: {e:?}");
                }
            }
        }
    });
}
