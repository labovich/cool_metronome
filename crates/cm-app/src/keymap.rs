//! Local keyboard shortcuts and the mouse wheel.
//!
//! The default keymap follows the original program. It is a table, so the
//! hotkey editor (stage 6) can replace it. Keys match by physical position
//! first, so shortcuts work with any keyboard layout (and Shift+= is still
//! "=" even though it types "+").

use cm_core::Subdivision;
use egui::{Event, Key, Modifiers, MouseWheelUnit, Vec2};

/// What a shortcut does.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Action {
    TogglePlay,
    /// Change the tempo by this many BPM.
    Tempo(f64),
    /// Quick jump: this many quick-jump steps (±1).
    TempoJump(f64),
    /// Multiply the tempo (×2, ÷2).
    TempoScale(f64),
    /// Change the master volume slider position by this much.
    Volume(f32),
    Tap,
    /// Meter preset key 1–8 (5 = 3+2).
    Meter(u8),
    Subdivision(Subdivision),
}

/// A key with exact modifiers (`ctrl` is Cmd on macOS).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Combo {
    pub key: Key,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

impl Combo {
    const fn new(key: Key, ctrl: bool, shift: bool, alt: bool) -> Self {
        Self {
            key,
            ctrl,
            shift,
            alt,
        }
    }

    fn matches(&self, key: Key, physical: Option<Key>, m: Modifiers) -> bool {
        (physical == Some(self.key) || key == self.key)
            && m.command == self.ctrl
            && m.shift == self.shift
            && m.alt == self.alt
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Binding {
    pub combo: Combo,
    pub action: Action,
    /// Whether holding the key repeats the action.
    pub repeat: bool,
}

const fn bind(
    key: Key,
    ctrl: bool,
    shift: bool,
    alt: bool,
    action: Action,
    repeat: bool,
) -> Binding {
    Binding {
        combo: Combo::new(key, ctrl, shift, alt),
        action,
        repeat,
    }
}

/// Volume step of ↑/↓ and the wheel; Ctrl makes it coarse.
const VOLUME_STEP: f32 = 0.05;
const VOLUME_COARSE: f32 = 0.2;

/// The default local keymap (as in the original program).
pub const DEFAULT_KEYMAP: &[Binding] = &[
    bind(Key::Space, false, false, false, Action::TogglePlay, false),
    bind(
        Key::ArrowRight,
        false,
        false,
        false,
        Action::Tempo(1.0),
        true,
    ),
    bind(
        Key::ArrowLeft,
        false,
        false,
        false,
        Action::Tempo(-1.0),
        true,
    ),
    bind(
        Key::ArrowRight,
        false,
        true,
        false,
        Action::Tempo(0.1),
        true,
    ),
    bind(
        Key::ArrowLeft,
        false,
        true,
        false,
        Action::Tempo(-0.1),
        true,
    ),
    bind(
        Key::PageUp,
        false,
        false,
        false,
        Action::TempoJump(1.0),
        true,
    ),
    bind(
        Key::PageDown,
        false,
        false,
        false,
        Action::TempoJump(-1.0),
        true,
    ),
    bind(
        Key::ArrowRight,
        true,
        false,
        false,
        Action::TempoJump(1.0),
        true,
    ),
    bind(
        Key::ArrowLeft,
        true,
        false,
        false,
        Action::TempoJump(-1.0),
        true,
    ),
    bind(
        Key::Equals,
        false,
        true,
        false,
        Action::TempoScale(2.0),
        false,
    ),
    bind(
        Key::Minus,
        false,
        true,
        false,
        Action::TempoScale(0.5),
        false,
    ),
    bind(
        Key::ArrowUp,
        false,
        false,
        false,
        Action::Volume(VOLUME_STEP),
        true,
    ),
    bind(
        Key::ArrowDown,
        false,
        false,
        false,
        Action::Volume(-VOLUME_STEP),
        true,
    ),
    bind(
        Key::ArrowUp,
        true,
        false,
        false,
        Action::Volume(VOLUME_COARSE),
        true,
    ),
    bind(
        Key::ArrowDown,
        true,
        false,
        false,
        Action::Volume(-VOLUME_COARSE),
        true,
    ),
    bind(Key::Enter, false, false, false, Action::Tap, false),
    bind(Key::T, false, false, false, Action::Tap, false),
    bind(Key::Num1, false, false, false, Action::Meter(1), false),
    bind(Key::Num2, false, false, false, Action::Meter(2), false),
    bind(Key::Num3, false, false, false, Action::Meter(3), false),
    bind(Key::Num4, false, false, false, Action::Meter(4), false),
    bind(Key::Num5, false, false, false, Action::Meter(5), false),
    bind(Key::Num6, false, false, false, Action::Meter(6), false),
    bind(Key::Num7, false, false, false, Action::Meter(7), false),
    bind(Key::Num8, false, false, false, Action::Meter(8), false),
    bind(
        Key::Num0,
        true,
        false,
        false,
        Action::Subdivision(Subdivision::None),
        false,
    ),
    bind(
        Key::Num1,
        true,
        false,
        false,
        Action::Subdivision(Subdivision::Single),
        false,
    ),
    bind(
        Key::Num2,
        true,
        false,
        false,
        Action::Subdivision(Subdivision::Duple),
        false,
    ),
    bind(
        Key::Num3,
        true,
        false,
        false,
        Action::Subdivision(Subdivision::Triple),
        false,
    ),
    bind(
        Key::Num4,
        true,
        false,
        false,
        Action::Subdivision(Subdivision::Quadruple),
        false,
    ),
    bind(
        Key::Num5,
        true,
        false,
        false,
        Action::Subdivision(Subdivision::Quintuple),
        false,
    ),
    bind(
        Key::Num6,
        true,
        false,
        false,
        Action::Subdivision(Subdivision::Sextuple),
        false,
    ),
    bind(
        Key::Num7,
        true,
        false,
        false,
        Action::Subdivision(Subdivision::Septuple),
        false,
    ),
    bind(
        Key::Num3,
        false,
        false,
        true,
        Action::Subdivision(Subdivision::Shuffle),
        false,
    ),
    bind(
        Key::Num4,
        false,
        false,
        true,
        Action::Subdivision(Subdivision::QuadrupleFourth),
        false,
    ),
];

/// The action bound to a key event, if any.
pub fn key_action(keymap: &[Binding], event: &Event) -> Option<Action> {
    let Event::Key {
        key,
        physical_key,
        pressed: true,
        repeat,
        modifiers,
    } = event
    else {
        return None;
    };
    keymap
        .iter()
        .find(|b| b.combo.matches(*key, *physical_key, *modifiers) && (b.repeat || !repeat))
        .map(|b| b.action)
}

/// Mouse wheel: ±1 BPM per notch (Shift ±0.1), Ctrl+wheel changes the volume.
/// `acc` collects partial notches from smooth wheels and touchpads.
#[derive(Debug, Default)]
pub struct Wheel {
    acc: f32,
}

/// Points of touchpad scrolling per wheel notch.
const POINTS_PER_NOTCH: f32 = 40.0;

impl Wheel {
    /// Turns one wheel event into actions (one per whole notch).
    pub fn actions(
        &mut self,
        unit: MouseWheelUnit,
        delta: Vec2,
        modifiers: Modifiers,
        out: &mut Vec<Action>,
    ) {
        // Shift+wheel may arrive as horizontal scrolling.
        let d = if delta.y == 0.0 { delta.x } else { delta.y };
        // Turning the other way drops a partial notch left from before.
        if d != 0.0 && self.acc != 0.0 && d.signum() != self.acc.signum() {
            self.acc = 0.0;
        }
        self.acc += match unit {
            MouseWheelUnit::Line => d,
            MouseWheelUnit::Point => d / POINTS_PER_NOTCH,
            MouseWheelUnit::Page => d * 3.0,
        };
        while self.acc.abs() >= 1.0 {
            let sign = self.acc.signum();
            self.acc -= sign;
            out.push(if modifiers.command {
                Action::Volume(VOLUME_STEP * sign)
            } else if modifiers.shift {
                Action::Tempo(0.1 * f64::from(sign))
            } else {
                Action::Tempo(f64::from(sign))
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(k: Key, physical: Option<Key>, m: Modifiers, repeat: bool) -> Event {
        Event::Key {
            key: k,
            physical_key: physical,
            pressed: true,
            repeat,
            modifiers: m,
        }
    }

    fn act(e: &Event) -> Option<Action> {
        key_action(DEFAULT_KEYMAP, e)
    }

    #[test]
    fn modifiers_must_match_exactly() {
        let none = Modifiers::NONE;
        let ctrl = Modifiers::COMMAND;
        let shift = Modifiers::SHIFT;
        let k = |k, m| act(&key(k, Some(k), m, false));
        assert_eq!(k(Key::Num3, none), Some(Action::Meter(3)));
        assert_eq!(
            k(Key::Num3, ctrl),
            Some(Action::Subdivision(Subdivision::Triple))
        );
        assert_eq!(
            k(Key::Num3, Modifiers::ALT),
            Some(Action::Subdivision(Subdivision::Shuffle))
        );
        // Ctrl+Shift+0 is the zoom reset, not "no subdivision".
        assert_eq!(k(Key::Num0, ctrl | shift), None);
        assert_eq!(k(Key::ArrowRight, shift), Some(Action::Tempo(0.1)));
        assert_eq!(k(Key::ArrowRight, ctrl), Some(Action::TempoJump(1.0)));
    }

    #[test]
    fn physical_key_wins_over_the_typed_symbol() {
        // Shift+= types "+", Shift+- types "_"; a Cyrillic layout types "е" on T.
        let shift = Modifiers::SHIFT;
        assert_eq!(
            act(&key(Key::Plus, Some(Key::Equals), shift, false)),
            Some(Action::TempoScale(2.0))
        );
        assert_eq!(
            act(&key(Key::Minus, Some(Key::Minus), shift, false)),
            Some(Action::TempoScale(0.5))
        );
        assert_eq!(
            act(&key(Key::E, Some(Key::T), Modifiers::NONE, false)),
            Some(Action::Tap)
        );
    }

    #[test]
    fn repeats_only_where_allowed() {
        let none = Modifiers::NONE;
        assert_eq!(act(&key(Key::Space, Some(Key::Space), none, true)), None);
        assert_eq!(act(&key(Key::T, Some(Key::T), none, true)), None);
        assert_eq!(
            act(&key(Key::ArrowLeft, Some(Key::ArrowLeft), none, true)),
            Some(Action::Tempo(-1.0))
        );
    }

    #[test]
    fn released_keys_do_nothing() {
        let e = Event::Key {
            key: Key::Space,
            physical_key: Some(Key::Space),
            pressed: false,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        assert_eq!(act(&e), None);
    }

    #[test]
    fn wheel_notches_and_touchpad() {
        let mut w = Wheel::default();
        let mut out = Vec::new();
        w.actions(
            MouseWheelUnit::Line,
            Vec2::new(0.0, 2.0),
            Modifiers::NONE,
            &mut out,
        );
        assert_eq!(out, [Action::Tempo(1.0), Action::Tempo(1.0)]);
        out.clear();
        // Touchpad: 30 points is not a notch yet, 30 more makes one.
        w.actions(
            MouseWheelUnit::Point,
            Vec2::new(0.0, -30.0),
            Modifiers::NONE,
            &mut out,
        );
        assert!(out.is_empty());
        w.actions(
            MouseWheelUnit::Point,
            Vec2::new(0.0, -30.0),
            Modifiers::NONE,
            &mut out,
        );
        assert_eq!(out, [Action::Tempo(-1.0)]);
        out.clear();
        w.actions(
            MouseWheelUnit::Line,
            Vec2::new(1.0, 0.0),
            Modifiers::SHIFT,
            &mut out,
        );
        w.actions(
            MouseWheelUnit::Line,
            Vec2::new(0.0, -1.0),
            Modifiers::COMMAND,
            &mut out,
        );
        assert_eq!(out, [Action::Tempo(0.1), Action::Volume(-VOLUME_STEP)]);
    }
}
