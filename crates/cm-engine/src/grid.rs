use cm_core::Tempo;

/// A regular grid of events at a fixed tempo, anchored at a start sample.
///
/// The position of event `n` is computed directly as
/// `start + round(n * period)` instead of adding the period again and again.
/// Rounding error therefore never accumulates: every event is within half a
/// sample of its ideal time, however long the metronome runs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BeatGrid {
    start: u64,
    period: f64,
}

impl BeatGrid {
    /// Grid of `events_per_beat` events per beat at `tempo`, starting at sample `start`.
    ///
    /// # Panics
    /// If `sample_rate` or `events_per_beat` is zero.
    #[must_use]
    pub fn new(tempo: Tempo, sample_rate: u32, events_per_beat: u32, start: u64) -> Self {
        assert!(sample_rate > 0, "sample rate must be positive");
        assert!(events_per_beat > 0, "events per beat must be positive");
        let period = tempo.beat_seconds() * f64::from(sample_rate) / f64::from(events_per_beat);
        Self { start, period }
    }

    /// Exact (unrounded) distance between two events, in samples.
    #[must_use]
    pub fn period(&self) -> f64 {
        self.period
    }

    /// Sample at which event `n` starts.
    #[must_use]
    pub fn event_sample(&self, n: u64) -> u64 {
        // n * period stays far below 2^53 for any realistic session
        // (2^53 samples at 192 kHz is ~1.5 million years).
        #[allow(clippy::cast_precision_loss)]
        let offset = (n as f64 * self.period).round();
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let offset = offset as u64;
        self.start + offset
    }

    /// Index of the first event that starts at or after `sample`.
    #[must_use]
    pub fn first_event_at_or_after(&self, sample: u64) -> u64 {
        if sample <= self.start {
            return 0;
        }
        #[allow(clippy::cast_precision_loss)]
        let approx = ((sample - self.start) as f64 / self.period).floor();
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let mut n = approx as u64;
        // Correct for rounding at the boundary.
        while n > 0 && self.event_sample(n - 1) >= sample {
            n -= 1;
        }
        while self.event_sample(n) < sample {
            n += 1;
        }
        n
    }
}

#[cfg(test)]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
mod tests {
    use super::*;

    /// One hour at an awkward tempo: every event stays within half a sample
    /// of its ideal position and the last event is exactly where it should be.
    #[test]
    fn no_drift_over_one_hour() {
        let rate = 44_100;
        let tempo = Tempo::from_bpm(133.7);
        let grid = BeatGrid::new(tempo, rate, 1, 0);
        let ideal_period = 60.0 / 133.7 * f64::from(rate);
        let events = (3600.0 / tempo.beat_seconds()) as u64;
        let mut worst: f64 = 0.0;
        for n in 0..=events {
            let ideal = n as f64 * ideal_period;
            let err = (grid.event_sample(n) as f64 - ideal).abs();
            worst = worst.max(err);
        }
        assert!(worst <= 0.5 + 1e-6, "worst error {worst} samples");
    }

    #[test]
    fn intervals_differ_by_at_most_one_sample() {
        let grid = BeatGrid::new(Tempo::from_bpm(97.3), 48_000, 3, 1_000);
        let mut prev = grid.event_sample(0);
        for n in 1..10_000 {
            let s = grid.event_sample(n);
            let gap = (s - prev) as f64;
            assert!((gap - grid.period()).abs() <= 1.0);
            prev = s;
        }
    }

    #[test]
    fn first_event_lookup_matches_positions() {
        let grid = BeatGrid::new(Tempo::from_bpm(208.0), 44_100, 4, 500);
        for sample in [0, 500, 501, 3_180, 3_181, 1_000_000] {
            let n = grid.first_event_at_or_after(sample);
            assert!(grid.event_sample(n) >= sample);
            if n > 0 {
                assert!(grid.event_sample(n - 1) < sample);
            }
        }
    }
}
