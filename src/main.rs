#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! DiskCraft — free, open-source disk and partition manager.

#[macro_use]
mod i18n;
mod fat32;
mod gpt;
mod logo;
mod model;
mod ops;
mod plan;
mod probe;
mod rawio;
mod ui;
mod winpe;
mod worker;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    i18n::init();

    // Elevated helper mode: carry out a plan written by the window.
    if let Some(i) = args.iter().position(|a| a == "--worker") {
        let plan = args.get(i + 1).cloned().unwrap_or_default();
        let log = args.get(i + 2).cloned().unwrap_or_default();
        let hash = args.get(i + 3).cloned().unwrap_or_default();
        std::process::exit(worker::run(&plan, &log, &hash));
    }
    // Write the logo files (PNG sizes, .ico, .svg) into a folder.
    if let Some(i) = args.iter().position(|a| a == "--export-logo") {
        let dir = args.get(i + 1).cloned().unwrap_or_else(|| "logo".into());
        std::process::exit(match logo::export(&dir) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("{e}");
                1
            }
        });
    }

    let screenshot = args.iter().position(|a| a == "--screenshot").and_then(|i| args.get(i + 1).cloned());
    let demo = args.iter().any(|a| a == "--demo");

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("DiskCraft")
        .with_app_id("diskcraft")
        .with_icon(std::sync::Arc::new(logo::icon()))
        .with_inner_size([1380.0, 880.0])
        .with_min_inner_size([1040.0, 660.0]);
    if screenshot.is_some() {
        // Checks must never steal focus or cover what the person is doing.
        viewport = viewport.with_active(false);
        #[cfg(windows)]
        {
            viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
    }
    let options = eframe::NativeOptions { viewport, run_and_return: true, ..Default::default() };
    let _ = eframe::run_native("DiskCraft", options, Box::new(move |cc| Ok(Box::new(ui::App::new(cc, screenshot, demo)))));
}
