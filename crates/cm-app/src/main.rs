//! Cool Metronome desktop application.

// No console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use cm_core::Tempo;

/// Default window size in logical points (scaled by the OS display scale).
const DEFAULT_SIZE: [f32; 2] = [440.0, 320.0];
/// Smallest allowed window size in logical points.
const MIN_SIZE: [f32; 2] = [380.0, 280.0];

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Cool Metronome")
            .with_inner_size(DEFAULT_SIZE)
            .with_min_inner_size(MIN_SIZE),
        ..Default::default()
    };

    eframe::run_native(
        "Cool Metronome",
        options,
        Box::new(|_cc| Ok(Box::new(CoolMetronome::default()))),
    )
}

/// Application state. Stage 1 skeleton: shows the default tempo only.
#[derive(Debug, Default)]
struct CoolMetronome {
    tempo: Tempo,
}

impl eframe::App for CoolMetronome {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(80.0);
                ui.heading(format!("{} BPM", self.tempo));
                ui.label("Cool Metronome — project skeleton");
            });
        });
    }
}
