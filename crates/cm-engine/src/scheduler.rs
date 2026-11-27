use crate::pattern::{BarPattern, ClickKind};
use cm_core::Tempo;

/// An event placed on the sample timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScheduledEvent {
    /// Sample at which the click starts.
    pub sample: u64,
    /// What sounds.
    pub kind: ClickKind,
    /// Meter cell of the event.
    pub cell: u16,
    /// Bar number since start (0-based).
    pub bar: u64,
}

/// Turns a bar pattern and a tempo into exact sample positions.
///
/// Every event's position is computed directly from its musical position:
/// `round(anchor_sample + (beats - anchor_beat) * samples_per_beat)`.
/// Nothing is accumulated from event to event, so the error of any event is
/// below half a sample however long it runs. A tempo change moves the anchor
/// to the next pending event, so the beat in progress keeps its length and
/// the new tempo starts exactly there — no click is lost, doubled or shifted.
///
/// Does not allocate. Replaced patterns are handed back to the caller.
#[derive(Debug)]
pub struct Scheduler {
    pattern: Box<BarPattern>,
    pending: Option<Box<BarPattern>>,
    sample_rate: f64,
    samples_per_beat: f64,
    anchor_sample: f64,
    anchor_beat: f64,
    pattern_start_beat: f64,
    bars_in_pattern: u64,
    bar: u64,
    index: usize,
    running: bool,
}

impl Scheduler {
    /// Creates a stopped scheduler.
    #[must_use]
    pub fn new(pattern: Box<BarPattern>, tempo: Tempo, sample_rate: u32) -> Self {
        let sample_rate = f64::from(sample_rate);
        Self {
            pattern,
            pending: None,
            sample_rate,
            samples_per_beat: tempo.beat_seconds() * sample_rate,
            anchor_sample: 0.0,
            anchor_beat: 0.0,
            pattern_start_beat: 0.0,
            bars_in_pattern: 0,
            bar: 0,
            index: 0,
            running: false,
        }
    }

    /// True between [`Scheduler::start`] and [`Scheduler::stop`].
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// The pattern currently playing.
    #[must_use]
    pub fn pattern(&self) -> &BarPattern {
        &self.pattern
    }

    /// Starts the first bar at `sample`. Returns a pattern that is no longer needed, if any.
    pub fn start(&mut self, sample: u64) -> Option<Box<BarPattern>> {
        let retired = self.take_pending();
        #[allow(clippy::cast_precision_loss)]
        let at = sample as f64;
        self.anchor_sample = at;
        self.anchor_beat = 0.0;
        self.pattern_start_beat = 0.0;
        self.bars_in_pattern = 0;
        self.bar = 0;
        self.index = 0;
        self.running = true;
        retired
    }

    /// Stops scheduling. Returns a pattern that is no longer needed, if any.
    pub fn stop(&mut self) -> Option<Box<BarPattern>> {
        self.running = false;
        self.take_pending()
    }

    /// Changes the tempo. While running, the change takes effect at the next
    /// event, which keeps its position; `now` is used only when the bar has no
    /// events at all.
    pub fn set_tempo(&mut self, tempo: Tempo, now: u64) {
        let new_spb = tempo.beat_seconds() * self.sample_rate;
        if self.running {
            let pivot_beat = match self.pattern.events().get(self.index) {
                Some(e) => self.bar_start_beat() + e.beat,
                None => {
                    #[allow(clippy::cast_precision_loss)]
                    let now = now as f64;
                    self.anchor_beat + (now - self.anchor_sample) / self.samples_per_beat
                }
            };
            self.anchor_sample += (pivot_beat - self.anchor_beat) * self.samples_per_beat;
            self.anchor_beat = pivot_beat;
        }
        self.samples_per_beat = new_spb;
    }

    /// Queues a new pattern. While running it replaces the current one at the
    /// next bar start; when stopped (or when the current bar is silent) at once.
    /// Returns a pattern that is no longer needed, if any.
    pub fn set_pattern(&mut self, pattern: Box<BarPattern>) -> Option<Box<BarPattern>> {
        if !self.running {
            return Some(std::mem::replace(&mut self.pattern, pattern));
        }
        if self.pattern.events().is_empty() {
            // Nothing would ever reach the bar end: switch now, on the current bar start.
            self.index = 0;
            return Some(std::mem::replace(&mut self.pattern, pattern));
        }
        self.pending.replace(pattern)
    }

    /// The next event, without consuming it. `None` when stopped or the bar is silent.
    #[must_use]
    pub fn peek(&self) -> Option<ScheduledEvent> {
        if !self.running {
            return None;
        }
        let e = self.pattern.events().get(self.index)?;
        let beats = self.bar_start_beat() + e.beat;
        let exact = self.anchor_sample + (beats - self.anchor_beat) * self.samples_per_beat;
        // Positions are non-negative and far below 2^53.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let sample = exact.round().max(0.0) as u64;
        Some(ScheduledEvent {
            sample,
            kind: e.kind,
            cell: e.cell,
            bar: self.bar,
        })
    }

    /// Consumes the event returned by [`Scheduler::peek`]. Returns a pattern
    /// that is no longer needed when a queued pattern took over at the bar start.
    pub fn advance(&mut self) -> Option<Box<BarPattern>> {
        if !self.running || self.pattern.events().is_empty() {
            return None;
        }
        self.index += 1;
        if self.index < self.pattern.events().len() {
            return None;
        }
        self.index = 0;
        self.bar += 1;
        self.bars_in_pattern += 1;
        if let Some(next) = self.pending.take() {
            self.pattern_start_beat = self.bar_start_beat_with(self.bars_in_pattern);
            self.bars_in_pattern = 0;
            return Some(std::mem::replace(&mut self.pattern, next));
        }
        None
    }

    fn take_pending(&mut self) -> Option<Box<BarPattern>> {
        let next = self.pending.take()?;
        Some(std::mem::replace(&mut self.pattern, next))
    }

    fn bar_start_beat(&self) -> f64 {
        self.bar_start_beat_with(self.bars_in_pattern)
    }

    fn bar_start_beat_with(&self, bars: u64) -> f64 {
        #[allow(clippy::cast_precision_loss)]
        let bars = bars as f64;
        self.pattern_start_beat + bars * self.pattern.length()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cm_core::{Meter, Subdivision};

    fn sched(formula: &str, bpm: f64) -> Scheduler {
        let p = BarPattern::new(
            &Meter::from_formula(formula).unwrap(),
            Subdivision::None,
            false,
        );
        Scheduler::new(Box::new(p), Tempo::from_bpm(bpm), 48_000)
    }

    fn take(s: &mut Scheduler, n: usize) -> Vec<ScheduledEvent> {
        (0..n)
            .map(|_| {
                let e = s.peek().unwrap();
                let _ = s.advance();
                e
            })
            .collect()
    }

    #[test]
    fn stopped_has_no_events() {
        let s = sched("4", 120.0);
        assert!(s.peek().is_none());
    }

    #[test]
    fn plain_beats() {
        let mut s = sched("4", 120.0);
        let _ = s.start(1000);
        let ev = take(&mut s, 6);
        let samples: Vec<u64> = ev.iter().map(|e| e.sample).collect();
        assert_eq!(samples, vec![1000, 25_000, 49_000, 73_000, 97_000, 121_000]);
        assert_eq!(ev[4].bar, 1);
        assert_eq!(ev[4].cell, 0);
        assert_eq!(ev[4].kind, ClickKind::Accent);
    }

    #[test]
    fn tempo_change_takes_effect_at_next_event() {
        let mut s = sched("4", 120.0);
        let _ = s.start(0);
        let first = take(&mut s, 2); // 0, 24000; next pending is 48000
        assert_eq!(first[1].sample, 24_000);
        s.set_tempo(Tempo::from_bpm(60.0), 30_000);
        let rest = take(&mut s, 3);
        assert_eq!(rest[0].sample, 48_000);
        assert_eq!(rest[1].sample, 96_000);
        assert_eq!(rest[2].sample, 144_000);
    }

    #[test]
    fn pattern_switches_at_bar_start() {
        let mut s = sched("4", 120.0);
        let _ = s.start(0);
        let _ = take(&mut s, 1);
        let three = BarPattern::new(&Meter::simple(3).unwrap(), Subdivision::None, false);
        assert!(s.set_pattern(Box::new(three)).is_none());
        let ev = take(&mut s, 7);
        let cells: Vec<u16> = ev.iter().map(|e| e.cell).collect();
        assert_eq!(cells, vec![1, 2, 3, 0, 1, 2, 0]);
        let samples: Vec<u64> = ev.iter().map(|e| e.sample).collect();
        assert_eq!(samples[3], 96_000);
        assert_eq!(samples[6], 168_000);
    }

    #[test]
    fn silent_pattern_switches_immediately() {
        let mut silent = Meter::simple(1).unwrap();
        silent.set_level(0, cm_core::BeatLevel::Mute);
        let p = BarPattern::new(&silent, Subdivision::None, false);
        assert_eq!(p.events().len(), 1); // the silent cell still exists
        let mut s = sched("4", 120.0);
        let _ = s.start(0);
        assert!(s.set_pattern(Box::new(p)).is_none());
    }
}
