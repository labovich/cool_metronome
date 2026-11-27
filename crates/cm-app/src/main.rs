//! Cool Metronome desktop application.
//!
//! Stage 2: a temporary test screen to hear the engine. The real interface
//! from the approved mockup arrives in stage 4.

// No console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod player;

use cm_core::{BeatLevel, Meter, Rhythm, Subdivision, Tempo};
use player::Player;
use std::time::Duration;

/// Default window size in logical points (scaled by the OS display scale).
const DEFAULT_SIZE: [f32; 2] = [440.0, 320.0];
/// Smallest allowed window size in logical points.
const MIN_SIZE: [f32; 2] = [380.0, 280.0];

const ACCENT: egui::Color32 = egui::Color32::from_rgb(255, 138, 61);
const NORMAL: egui::Color32 = egui::Color32::from_rgb(201, 203, 210);
const MUTED: egui::Color32 = egui::Color32::from_rgb(110, 114, 124);

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
        Box::new(|cc| {
            let ctx = cc.egui_ctx.clone();
            let waker = cm_audio::Waker::new(move || ctx.request_repaint());
            Ok(Box::new(TestScreen::new(waker)))
        }),
    )
}

#[derive(Debug)]
struct TestScreen {
    player: Player,
    rhythms: Vec<Rhythm>,
    formula: String,
    formula_ok: bool,
}

impl TestScreen {
    fn new(waker: cm_audio::Waker) -> Self {
        Self {
            player: Player::new(waker),
            rhythms: Rhythm::builtin(),
            formula: "4".to_owned(),
            formula_ok: true,
        }
    }

    fn rhythm_picker(&mut self, ui: &mut egui::Ui) {
        let meter = self.player.meter();
        let subdivision = self.player.subdivision();
        let current = self
            .rhythms
            .iter()
            .position(|r| &r.meter == meter && r.subdivision == subdivision);
        let mut picked = None;
        egui::ComboBox::from_id_salt("rhythm")
            .selected_text(current.map_or("Custom", |i| self.rhythms[i].name.as_str()))
            .show_ui(ui, |ui| {
                let mut category = None;
                for (i, r) in self.rhythms.iter().enumerate() {
                    if category != Some(r.category) {
                        category = Some(r.category);
                        ui.weak(r.category.label());
                    }
                    if ui.selectable_label(current == Some(i), &r.name).clicked() {
                        picked = Some(i);
                    }
                }
            });
        if let Some(i) = picked {
            let r = &self.rhythms[i];
            self.player.set_meter(r.meter.clone());
            self.player.set_subdivision(r.subdivision);
            self.formula = self.player.meter().label();
            self.formula_ok = true;
        }
    }

    fn beat_indicator(&mut self, ui: &mut egui::Ui) {
        let cells: Vec<BeatLevel> = self.player.meter().cells().to_vec();
        // Shrink the circles so long bars (16-cell clave) fit the window.
        let n = cells.len().max(1) as f32;
        let step = (ui.available_width() / n).clamp(8.0, 28.0);
        let radius = (step - 8.0).max(step * 0.6) / 2.0;
        let width = n * step;
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(width, 2.0 * radius + 8.0), egui::Sense::click());
        let current = self.player.current_cell();
        let painter = ui.painter_at(rect);
        for (i, level) in cells.iter().enumerate() {
            let center = egui::pos2(rect.left() + radius + i as f32 * step, rect.center().y);
            let lit = current == Some(i);
            let r = if lit { radius } else { radius - 2.0 };
            match level {
                BeatLevel::Accent => {
                    painter.circle_filled(center, r, ACCENT);
                }
                BeatLevel::Normal => {
                    painter.circle_filled(center, r, NORMAL);
                }
                BeatLevel::Mute => {
                    painter.circle_stroke(center, r, egui::Stroke::new(1.5, MUTED));
                }
            }
            if lit {
                painter.circle_stroke(center, radius + 3.0, egui::Stroke::new(2.0, NORMAL));
            }
        }
        if response.clicked()
            && let Some(pos) = response.interact_pointer_pos()
        {
            let x = pos.x - rect.left();
            if x >= 0.0 {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let index = (x / step) as usize;
                self.player.cycle_cell(index);
                self.formula = self.player.meter().label();
                self.formula_ok = true;
            }
        }
    }
}

impl TestScreen {
    /// Beat indicator while playing; otherwise only what the player needs.
    fn schedule_repaint(&self, ctx: &egui::Context) {
        let frame = self
            .player
            .is_playing()
            .then_some(Duration::from_millis(16));
        if let Some(after) = frame.into_iter().chain(self.player.next_tick()).min() {
            ctx.request_repaint_after(after);
        }
    }
}

impl eframe::App for TestScreen {
    /// Runs on every pass, also while the window is minimized or hidden
    /// (then `ui` is skipped), so the audio housekeeping never stalls.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.player.tick();
        self.schedule_repaint(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("Cool Metronome — engine test");
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                let label = if self.player.is_playing() {
                    "■ Stop"
                } else {
                    "▶ Play"
                };
                if ui
                    .add_sized([90.0, 32.0], egui::Button::new(label))
                    .clicked()
                {
                    self.player.toggle();
                }
                let mut bpm = self.player.tempo().bpm();
                let drag = egui::DragValue::new(&mut bpm)
                    .range(1.0..=999.0)
                    .speed(0.1)
                    .fixed_decimals(1)
                    .suffix(" BPM");
                if ui.add(drag).changed() {
                    self.player.set_tempo(Tempo::from_bpm(bpm));
                }
            });

            let mut bpm = self.player.tempo().bpm();
            if ui
                .add(egui::Slider::new(&mut bpm, 40.0..=208.0).show_value(false))
                .changed()
            {
                self.player.set_tempo(Tempo::from_bpm(bpm.round()));
            }

            ui.horizontal(|ui| {
                ui.label("Meter");
                let edit =
                    ui.add(egui::TextEdit::singleline(&mut self.formula).desired_width(90.0));
                if edit.changed() {
                    match Meter::from_formula(&self.formula) {
                        Ok(m) => {
                            self.player.set_meter(m);
                            self.formula_ok = true;
                        }
                        Err(_) => self.formula_ok = false,
                    }
                }
                if !self.formula_ok {
                    ui.colored_label(ACCENT, "e.g. 4 or 3+2+2");
                }
            });

            ui.horizontal(|ui| {
                ui.label("Subdivision");
                let mut sub = self.player.subdivision();
                egui::ComboBox::from_id_salt("subdivision")
                    .selected_text(sub.label())
                    .show_ui(ui, |ui| {
                        for s in Subdivision::ALL {
                            ui.selectable_value(&mut sub, s, s.label());
                        }
                    });
                if sub != self.player.subdivision() {
                    self.player.set_subdivision(sub);
                }
            });

            ui.horizontal(|ui| {
                ui.label("Rhythm");
                self.rhythm_picker(ui);
            });

            let mut bell = self.player.bell();
            if ui
                .checkbox(&mut bell, "Bell on the bar start in compound meters")
                .changed()
            {
                self.player.set_bell(bell);
            }

            ui.add_space(6.0);
            self.beat_indicator(ui);
            ui.label("Click a beat to cycle accent / mute / normal.");

            ui.separator();
            match (self.player.device_info(), self.player.error()) {
                (_, Some(err)) => {
                    ui.colored_label(ACCENT, err);
                }
                (Some(info), None) => {
                    ui.weak(info);
                }
                (None, None) => {}
            }
        });
        // Commands sent above may need a follow-up tick.
        self.schedule_repaint(ui.ctx());
    }
}
