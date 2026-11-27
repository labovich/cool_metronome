use cm_core::{BeatLevel, Meter, Subdivision};

/// What a scheduled event plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClickKind {
    /// Accented beat.
    Accent,
    /// Normal beat.
    Normal,
    /// Subdivision click between beats.
    Sub,
    /// Bell marking the bar start in compound meters.
    Bell,
    /// Muted cell: no sound, but the beat indicator still moves.
    Silent,
}

impl ClickKind {
    /// Number of kinds that have a sound (all but [`ClickKind::Silent`]).
    pub const SOUNDING: usize = 4;

    /// Index of the sound slot, or `None` for [`ClickKind::Silent`].
    #[must_use]
    pub fn sound_index(self) -> Option<usize> {
        match self {
            Self::Accent => Some(0),
            Self::Normal => Some(1),
            Self::Sub => Some(2),
            Self::Bell => Some(3),
            Self::Silent => None,
        }
    }

    /// True for events that mark a meter cell (the beat indicator follows these).
    #[must_use]
    pub fn is_cell(self) -> bool {
        matches!(self, Self::Accent | Self::Normal | Self::Silent)
    }
}

/// One event inside a bar.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PatternEvent {
    /// Position from the bar start, in beats.
    pub beat: f64,
    /// What sounds.
    pub kind: ClickKind,
    /// Meter cell the event belongs to (for subdivisions: the cell it falls in).
    pub cell: u16,
}

/// All events of one bar, sorted by position. Built off the audio thread.
#[derive(Debug, Clone, PartialEq)]
pub struct BarPattern {
    events: Vec<PatternEvent>,
    length: f64,
    cells: u16,
}

/// Two positions closer than this (in beats) are treated as the same instant.
const SAME_TIME: f64 = 1e-9;

impl BarPattern {
    /// Builds the bar from a meter and a subdivision.
    ///
    /// `bell` adds the bell at the bar start when the meter is compound (3+2, 2+2+3…).
    /// A subdivision click that coincides with a sounding cell is left out.
    #[must_use]
    pub fn new(meter: &Meter, subdivision: Subdivision, bell: bool) -> Self {
        let subunits = f64::from(meter.subunits());
        let length = meter.bar_beats();
        let mut events = Vec::new();

        if bell && meter.is_compound() {
            events.push(PatternEvent {
                beat: 0.0,
                kind: ClickKind::Bell,
                cell: 0,
            });
        }

        for (i, level) in meter.cells().iter().enumerate() {
            let kind = match level {
                BeatLevel::Accent => ClickKind::Accent,
                BeatLevel::Normal => ClickKind::Normal,
                BeatLevel::Mute => ClickKind::Silent,
            };
            events.push(PatternEvent {
                beat: index_f64(i) / subunits,
                kind,
                cell: cell_u16(i),
            });
        }

        let parts = subdivision.parts();
        for beat in 0..meter.beat_count() {
            for k in 1..parts {
                if !subdivision.plays(k) {
                    continue;
                }
                let pos = index_f64(beat) + f64::from(k) / f64::from(parts);
                if pos >= length - SAME_TIME {
                    continue; // falls into the cut part of the bar
                }
                let sounding_cell_here = events.iter().any(|e| {
                    e.kind != ClickKind::Silent
                        && e.kind != ClickKind::Bell
                        && (e.beat - pos).abs() < SAME_TIME
                });
                if sounding_cell_here {
                    continue;
                }
                // floor(pos * subunits) is a valid cell index: pos < length.
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let cell = (pos * subunits + SAME_TIME).floor() as usize;
                events.push(PatternEvent {
                    beat: pos,
                    kind: ClickKind::Sub,
                    cell: cell_u16(cell.min(meter.len() - 1)),
                });
            }
        }

        events.sort_by(|a, b| a.beat.total_cmp(&b.beat));
        Self {
            events,
            length,
            cells: cell_u16(meter.len()),
        }
    }

    /// Events in time order.
    #[must_use]
    pub fn events(&self) -> &[PatternEvent] {
        &self.events
    }

    /// Bar length in beats.
    #[must_use]
    pub fn length(&self) -> f64 {
        self.length
    }

    /// Number of meter cells.
    #[must_use]
    pub fn cells(&self) -> u16 {
        self.cells
    }
}

fn index_f64(i: usize) -> f64 {
    // Indexes are bounded by Meter::MAX_CELLS.
    #[allow(clippy::cast_precision_loss)]
    let v = i as f64;
    v
}

fn cell_u16(i: usize) -> u16 {
    u16::try_from(i).unwrap_or(u16::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ClickKind::{Accent as A, Bell as B, Normal as N, Silent as S, Sub as U};

    fn kinds(p: &BarPattern) -> Vec<(f64, ClickKind)> {
        p.events().iter().map(|e| (e.beat, e.kind)).collect()
    }

    #[test]
    fn four_four_with_eighths() {
        let p = BarPattern::new(&Meter::default(), Subdivision::Duple, true);
        assert_eq!(
            kinds(&p),
            vec![
                (0.0, A),
                (0.5, U),
                (1.0, N),
                (1.5, U),
                (2.0, N),
                (2.5, U),
                (3.0, N),
                (3.5, U)
            ]
        );
    }

    #[test]
    fn bell_only_in_compound_meters() {
        let p = BarPattern::new(
            &Meter::from_formula("3+2").unwrap(),
            Subdivision::None,
            true,
        );
        assert_eq!(kinds(&p)[0], (0.0, B));
        assert_eq!(kinds(&p)[1], (0.0, A));
        let p = BarPattern::new(
            &Meter::from_formula("3+2").unwrap(),
            Subdivision::None,
            false,
        );
        assert_eq!(kinds(&p)[0], (0.0, A));
    }

    #[test]
    fn mute_keeps_indicator_event() {
        let mut m = Meter::default();
        m.set_level(2, BeatLevel::Mute);
        let p = BarPattern::new(&m, Subdivision::None, true);
        assert_eq!(kinds(&p)[2], (2.0, S));
    }

    #[test]
    fn cut_bar_drops_subdivisions_past_the_end() {
        // 7/8: 3.5 beats. Duple subdivisions at 0.5..2.5 only; 3.5 is past the end.
        let m = Meter::new(2, vec![BeatLevel::Accent; 7]).unwrap();
        let p = BarPattern::new(&m, Subdivision::Duple, false);
        assert!(p.events().iter().all(|e| e.kind != U));
        assert_eq!(p.events().len(), 7);
        assert!((p.length() - 3.5).abs() < 1e-12);
    }

    #[test]
    fn subdivisions_independent_of_subunits() {
        // Two subunits per beat, triplet subdivision: "two against three".
        let m = Meter::new(2, vec![BeatLevel::Accent, BeatLevel::Normal]).unwrap();
        let p = BarPattern::new(&m, Subdivision::Triple, false);
        let subs: Vec<f64> = p
            .events()
            .iter()
            .filter(|e| e.kind == U)
            .map(|e| e.beat)
            .collect();
        assert_eq!(subs.len(), 2);
        assert!((subs[0] - 1.0 / 3.0).abs() < 1e-12);
        assert!((subs[1] - 2.0 / 3.0).abs() < 1e-12);
    }
}
