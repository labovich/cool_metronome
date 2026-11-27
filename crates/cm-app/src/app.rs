//! The eframe application: state, housekeeping, repaint scheduling, zoom.

use crate::i18n::I18n;
use crate::player::Player;
use crate::theme::{self, Palette};
use crate::view::{TempoEdit, View};
use cm_core::Rhythm;
use egui::{Event, Key, Modifiers};
use std::time::Instant;

/// UI zoom range on top of the OS display scale.
const ZOOM_MIN: f32 = 0.8;
const ZOOM_MAX: f32 = 2.0;
const ZOOM_STEP: f32 = 0.1;

#[derive(Debug)]
pub struct MetronomeApp {
    player: Player,
    i18n: I18n,
    rhythms: Vec<Rhythm>,
    tempo_edit: Option<TempoEdit>,
}

impl MetronomeApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let ctx = cc.egui_ctx.clone();
        theme::install_fonts(&ctx);
        theme::install_visuals(&ctx);
        // Own zoom shortcuts (limited range; Ctrl+0 is reserved for subdivisions).
        ctx.options_mut(|o| o.zoom_with_keyboard = false);
        let waker = cm_audio::Waker::new(move || ctx.request_repaint());
        Self {
            player: Player::new(waker),
            i18n: I18n::english(),
            rhythms: Rhythm::builtin(),
            tempo_edit: None,
        }
    }

    /// Asks for the next frame only when something will change by itself.
    fn schedule_repaint(&self, ctx: &egui::Context) {
        let now = Instant::now();
        let next = [self.player.next_tick(), self.player.next_visual_change(now)]
            .into_iter()
            .flatten()
            .min();
        if let Some(after) = next {
            ctx.request_repaint_after(after);
        }
    }
}

impl eframe::App for MetronomeApp {
    /// Runs on every pass, also while the window is minimized or hidden
    /// (then `ui` is skipped), so the audio housekeeping never stalls.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.player.tick();
        self.schedule_repaint(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        handle_zoom(&ctx);
        let p = Palette::of(ctx.theme());
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(p.bg))
            .show(ui, |ui| {
                let rect = ui.max_rect();
                View {
                    player: &mut self.player,
                    i18n: &self.i18n,
                    rhythms: &self.rhythms,
                    tempo_edit: &mut self.tempo_edit,
                    p,
                    now: Instant::now(),
                }
                .show(ui, rect);
            });
        // Commands sent above may need a follow-up tick.
        self.schedule_repaint(&ctx);
    }
}

/// Ctrl+= / Ctrl+− zoom by 10 % within 80–200 %, Ctrl+Shift+0 resets.
fn handle_zoom(ctx: &egui::Context) {
    let zoom = ctx.zoom_factor();
    let new = if take_key(ctx, Key::Num0, Modifiers::COMMAND | Modifiers::SHIFT) {
        1.0
    } else if take_key(ctx, Key::Equals, Modifiers::COMMAND)
        || take_key(ctx, Key::Plus, Modifiers::COMMAND)
    {
        zoom + ZOOM_STEP
    } else if take_key(ctx, Key::Minus, Modifiers::COMMAND) {
        zoom - ZOOM_STEP
    } else {
        return;
    };
    let new = ((new * 10.0).round() / 10.0).clamp(ZOOM_MIN, ZOOM_MAX);
    ctx.set_zoom_factor(new);
}

/// Consumes a press of `key` with exactly `mods` (Ctrl/Shift/Alt; Cmd on
/// macOS). Matches the physical key too, so it works with any keyboard
/// layout and with Shift changing the symbol (Shift+0 is ")").
fn take_key(ctx: &egui::Context, key: Key, mods: Modifiers) -> bool {
    ctx.input_mut(|i| {
        let before = i.events.len();
        i.events.retain(|e| {
            !matches!(
                e,
                Event::Key { key: k, physical_key, pressed: true, modifiers, .. }
                    if (*k == key || *physical_key == Some(key))
                        && modifiers.command == mods.command
                        && modifiers.shift == mods.shift
                        && modifiers.alt == mods.alt
            )
        });
        i.events.len() != before
    })
}
