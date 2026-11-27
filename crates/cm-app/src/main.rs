//! Cool Metronome desktop application.

// No console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod i18n;
mod icons;
mod keymap;
mod options;
mod player;
mod settings;
mod theme;
mod view;

fn main() -> eframe::Result {
    let title = i18n::I18n::english().tr("app-title");
    let (store, settings) = settings::Store::open();
    let options = eframe::NativeOptions {
        viewport: viewport(&title, &settings.window),
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
        Box::new(move |cc| Ok(Box::new(app::MetronomeApp::new(cc, store, &settings)))),
    )
}

/// The main window as it was left: size, position, compact mode, on top.
fn viewport(title: &str, w: &settings::WindowState) -> egui::ViewportBuilder {
    let full = w
        .size(app::MIN_SIZE.into())
        .map_or(app::DEFAULT_SIZE, Into::into);
    let (size, min) = if w.compact {
        (app::COMPACT_SIZE, app::COMPACT_SIZE)
    } else {
        (full, app::MIN_SIZE)
    };
    let mut vp = egui::ViewportBuilder::default()
        .with_title(title)
        .with_inner_size(size)
        .with_min_inner_size(min)
        // Own title bar and rounded frame (the mockup); transparent corners.
        .with_decorations(false)
        .with_transparent(true);
    if let Some(pos) = w.position() {
        vp = vp.with_position(pos);
    }
    if w.pinned {
        vp = vp.with_window_level(egui::WindowLevel::AlwaysOnTop);
    }
    vp
}
