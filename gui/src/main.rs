#![warn(clippy::all, rust_2018_idioms)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

// When compiling natively:
#[cfg(not(target_arch = "wasm32"))]
fn main() {
    // Log to stdout (if you run with `RUST_LOG=debug`).
    // tracing_subscriber::fmt::init();
    gui::egui_logger::init(&[
        "async_io",
        "polling",
        "sqlx::query",
        "tantivy::directory",
        "mio::poll",
        "want",
    ])
    .unwrap();

    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "Lucille",
        native_options,
        Box::new(|cc| Box::new(gui::ShellApp::new(cc))),
    )
    .expect("failed to run eframe");
}
