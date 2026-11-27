//! Cool Metronome desktop application.

// No console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod i18n;
mod icons;
mod player;
mod theme;
mod view;

/// Default window size in logical points (scaled by the OS display scale).
const DEFAULT_SIZE: [f32; 2] = [440.0, 320.0];
/// Smallest allowed window size in logical points.
const MIN_SIZE: [f32; 2] = [380.0, 280.0];

fn main() -> eframe::Result {
    let title = i18n::I18n::english().tr("app-title");
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(&title)
            .with_inner_size(DEFAULT_SIZE)
            .with_min_inner_size(MIN_SIZE),
        // The UI repaints only on events and beat changes, so there is nothing
        // to tear; waiting for vsync costs CPU in some OpenGL drivers.
        glow_options: eframe::egui_glow::GlowConfiguration {
            vsync: false,
            ..Default::default()
        },
        ..Default::default()
    };
    eframe::run_native(
        &title,
        options,
        Box::new(|cc| Ok(Box::new(app::MetronomeApp::new(cc)))),
    )
}
