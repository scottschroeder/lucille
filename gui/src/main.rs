#![warn(clippy::all, rust_2018_idioms)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

// When compiling natively:
#[cfg(not(target_arch = "wasm32"))]
fn main() {
    // Log to stdout (if you run with `RUST_LOG=debug`).
    // tracing_subscriber::fmt::init();
    gui::egui_logger::init().unwrap();

    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "Lucile",
        native_options,
        Box::new(|cc| Box::new(gui::ShellApp::new(cc))),
    );
}
