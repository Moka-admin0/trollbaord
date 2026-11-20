#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(not(windows))]
compile_error!("trollboard only runs on Windows");

mod app_audio;
mod audio;
mod clips;
mod engine;
mod hotkeys;
mod settings;
mod ui;
mod win32;

use eframe::egui::ViewportBuilder;

fn main() -> eframe::Result {
    let open_setup = std::env::args().any(|arg| arg == "--setup");

    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_title("trollboard")
            .with_app_id("trollboard")
            .with_inner_size([540.0, 680.0])
            .with_min_inner_size([510.0, 520.0])
            .with_decorations(false)
            .with_drag_and_drop(true),
        ..Default::default()
    };

    eframe::run_native("trollboard", options, Box::new(move |cc| Ok(Box::new(ui::TrollApp::new(cc, open_setup)))))
}
