#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use eframe::egui;
use env_logger;

mod app;
mod retrieval;

fn main() -> Result<(), eframe::Error> {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([640.0, 480.0]),
        ..Default::default()
    };

    eframe::run_native(
        "ballast",
        options,
        Box::new(|cc| {
            let mut app = app::Ballast::new();
            app.do_home_page();

            Box::new(app)
        }),
    )
}
