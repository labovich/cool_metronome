//! The eframe application: state, window chrome, keyboard and wheel,
//! housekeeping, repaint scheduling, zoom.

use crate::i18n::I18n;
use crate::keymap::{self, Action, Wheel};
use crate::player::Player;
use crate::theme::{self, Palette};
use crate::view::{TITLE_H, TempoEdit, View, WindowAction};
use cm_core::{Meter, Rhythm};
use egui::{
    CursorIcon, Event, Id, Key, Modifiers, Rect, ResizeDirection, Sense, Stroke, StrokeKind, Vec2,
    ViewportCommand, WindowLevel, pos2, vec2,
};
use std::time::Instant;

/// Default window size in logical points (scaled by the OS display scale).
pub const DEFAULT_SIZE: Vec2 = vec2(440.0, 320.0);
/// Smallest full window.
pub const MIN_SIZE: Vec2 = vec2(380.0, 280.0);
/// The compact strip.
const COMPACT_SIZE: Vec2 = vec2(440.0, 96.0);
/// Window corner radius and resize grip width.
const RADIUS: f32 = 10.0;
const GRIP: f32 = 5.0;

/// UI zoom range on top of the OS display scale.
const ZOOM_MIN: f32 = 0.8;
const ZOOM_MAX: f32 = 2.0;
const ZOOM_STEP: f32 = 0.1;
/// Quick jump step of PgUp/PgDn and Ctrl+←/→ (configurable in stage 5).
const QUICK_JUMP: f64 = 6.0;

#[derive(Debug)]
pub struct MetronomeApp {
    player: Player,
    i18n: I18n,
    rhythms: Vec<Rhythm>,
    tempo_edit: Option<TempoEdit>,
    wheel: Wheel,
    compact: bool,
    pinned: bool,
    /// Full window size to restore when leaving compact mode.
    full_size: Option<Vec2>,
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
            wheel: Wheel::default(),
            compact: false,
            pinned: false,
            full_size: None,
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

    /// Takes the shortcut and wheel events of this frame and applies them.
    /// Nothing is taken while a text field (the typed tempo) has the keyboard.
    fn handle_input(&mut self, ctx: &egui::Context) {
        let keys = !ctx.egui_wants_keyboard_input();
        // Menus and the volume popup keep the wheel for themselves.
        let wheel = !egui::Popup::is_any_open(ctx) && ctx.is_pointer_over_egui();
        let mut actions = Vec::new();
        ctx.input_mut(|i| {
            i.events.retain(|e| match e {
                Event::Key { .. } if keys => match keymap::key_action(keymap::DEFAULT_KEYMAP, e) {
                    Some(a) => {
                        actions.push(a);
                        false
                    }
                    None => true,
                },
                Event::MouseWheel {
                    unit,
                    delta,
                    modifiers,
                    ..
                } if wheel => {
                    self.wheel.actions(*unit, *delta, *modifiers, &mut actions);
                    false
                }
                _ => true,
            });
        });
        let now = Instant::now();
        for action in actions {
            self.apply(action, now);
        }
    }

    fn apply(&mut self, action: Action, now: Instant) {
        let p = &mut self.player;
        match action {
            Action::TogglePlay => p.toggle(),
            Action::Tempo(d) => p.set_tempo(p.tempo().offset(d)),
            Action::TempoJump(n) => p.set_tempo(p.tempo().offset(n * QUICK_JUMP)),
            Action::TempoScale(f) => p.set_tempo(p.tempo().scaled(f)),
            Action::Volume(d) => p.set_volume(p.volume() + d),
            Action::Tap => p.tap(now),
            Action::Meter(n) => p.set_meter(meter_for_key(n)),
            Action::Subdivision(s) => p.set_subdivision(s),
        }
    }

    fn window_action(&mut self, ctx: &egui::Context, action: WindowAction) {
        match action {
            WindowAction::Drag => ctx.send_viewport_cmd(ViewportCommand::StartDrag),
            WindowAction::Minimize => ctx.send_viewport_cmd(ViewportCommand::Minimized(true)),
            WindowAction::Close => ctx.send_viewport_cmd(ViewportCommand::Close),
            WindowAction::TogglePin => {
                self.pinned = !self.pinned;
                let level = if self.pinned {
                    WindowLevel::AlwaysOnTop
                } else {
                    WindowLevel::Normal
                };
                ctx.send_viewport_cmd(ViewportCommand::WindowLevel(level));
            }
            WindowAction::ToggleCompact => {
                self.compact = !self.compact;
                if self.compact {
                    self.full_size = ctx.input(|i| i.viewport().inner_rect.map(|r| r.size()));
                    self.tempo_edit = None;
                    ctx.send_viewport_cmd(ViewportCommand::MinInnerSize(COMPACT_SIZE));
                    ctx.send_viewport_cmd(ViewportCommand::InnerSize(COMPACT_SIZE));
                } else {
                    let size = self.full_size.unwrap_or(DEFAULT_SIZE).max(MIN_SIZE);
                    ctx.send_viewport_cmd(ViewportCommand::InnerSize(size));
                    ctx.send_viewport_cmd(ViewportCommand::MinInnerSize(MIN_SIZE));
                }
            }
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

    /// The window is transparent outside its rounded frame.
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0; 4]
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        handle_zoom(&ctx);
        self.handle_input(&ctx);
        let p = Palette::of(ctx.theme());
        let mut action = None;
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                let rect = ui.max_rect();
                ui.painter().rect(
                    rect,
                    RADIUS,
                    p.bg,
                    Stroke::new(1.0, p.border),
                    StrokeKind::Inside,
                );
                let mut view = View {
                    player: &mut self.player,
                    i18n: &self.i18n,
                    rhythms: &self.rhythms,
                    tempo_edit: &mut self.tempo_edit,
                    p,
                    now: Instant::now(),
                };
                if self.compact {
                    action = view.show_compact(ui, rect);
                } else {
                    let (title, content) = rect.split_top_bottom_at_y(rect.top() + TITLE_H);
                    action = view.title_bar(ui, title, self.pinned);
                    view.show(ui, content);
                    resize_grips(ui, rect);
                }
            });
        if let Some(action) = action {
            self.window_action(&ctx, action);
        }
        // Commands sent above may need a follow-up tick.
        self.schedule_repaint(&ctx);
    }
}

/// Meter preset keys 1–8: N beats, 5 is 3+2.
fn meter_for_key(n: u8) -> Meter {
    if n == 5 {
        Meter::from_formula("3+2").expect("valid preset")
    } else {
        Meter::simple(usize::from(n.max(1))).expect("1..=8 beats")
    }
}

/// Invisible grips along the window edges that start a native resize.
/// Added last, so they win over the widgets near the edges.
fn resize_grips(ui: &egui::Ui, rect: Rect) {
    use ResizeDirection as D;
    let (l, r, t, b) = (rect.left(), rect.right(), rect.top(), rect.bottom());
    let c = RADIUS;
    let grips = [
        (
            D::North,
            Rect::from_min_max(pos2(l + c, t), pos2(r - c, t + GRIP)),
        ),
        (
            D::South,
            Rect::from_min_max(pos2(l + c, b - GRIP), pos2(r - c, b)),
        ),
        (
            D::West,
            Rect::from_min_max(pos2(l, t + c), pos2(l + GRIP, b - c)),
        ),
        (
            D::East,
            Rect::from_min_max(pos2(r - GRIP, t + c), pos2(r, b - c)),
        ),
        (
            D::NorthWest,
            Rect::from_min_max(pos2(l, t), pos2(l + c, t + c)),
        ),
        (
            D::NorthEast,
            Rect::from_min_max(pos2(r - c, t), pos2(r, t + c)),
        ),
        (
            D::SouthWest,
            Rect::from_min_max(pos2(l, b - c), pos2(l + c, b)),
        ),
        (
            D::SouthEast,
            Rect::from_min_max(pos2(r - c, b - c), pos2(r, b)),
        ),
    ];
    for (dir, grip) in grips {
        let cursor = match dir {
            D::North | D::South => CursorIcon::ResizeVertical,
            D::East | D::West => CursorIcon::ResizeHorizontal,
            D::NorthWest | D::SouthEast => CursorIcon::ResizeNwSe,
            D::NorthEast | D::SouthWest => CursorIcon::ResizeNeSw,
        };
        let resp = ui
            .interact(
                grip,
                Id::unique(("grip", format!("{dir:?}"))),
                Sense::drag(),
            )
            .on_hover_cursor(cursor);
        if resp.drag_started() {
            ui.ctx()
                .send_viewport_cmd(ViewportCommand::BeginResize(dir));
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meter_keys() {
        assert_eq!(meter_for_key(1).label(), "1");
        assert_eq!(meter_for_key(4).label(), "4");
        assert_eq!(meter_for_key(5).label(), "3+2");
        assert_eq!(meter_for_key(8).len(), 8);
    }
}
