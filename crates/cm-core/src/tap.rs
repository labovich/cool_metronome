use crate::Tempo;

/// Tap tempo: the average interval of the recent taps.
///
/// Times are seconds on any monotonic clock, so the logic is testable
/// without real time. A pause longer than [`TapTempo::RESET_SECONDS`]
/// starts a new measurement.
#[derive(Debug, Clone, Default)]
pub struct TapTempo {
    taps: [f64; Self::MAX_TAPS],
    len: usize,
    fractional: bool,
}

impl TapTempo {
    /// Most recent taps used for the average.
    pub const MAX_TAPS: usize = 6;
    /// A longer pause between taps starts over (20+ BPM can be tapped).
    pub const RESET_SECONDS: f64 = 3.0;

    /// `fractional`: round the result to 0.1 BPM instead of whole BPM.
    #[must_use]
    pub fn new(fractional: bool) -> Self {
        Self {
            fractional,
            ..Self::default()
        }
    }

    /// Registers a tap at `at` seconds. Returns the tempo once there are two
    /// or more taps in the current measurement.
    pub fn tap(&mut self, at: f64) -> Option<Tempo> {
        if !at.is_finite() {
            return None;
        }
        let restart = self.len == 0 || {
            let gap = at - self.taps[self.len - 1];
            !(0.0..=Self::RESET_SECONDS).contains(&gap) || gap == 0.0
        };
        if restart {
            self.taps[0] = at;
            self.len = 1;
            return None;
        }
        if self.len == Self::MAX_TAPS {
            self.taps.copy_within(1.., 0);
            self.len -= 1;
        }
        self.taps[self.len] = at;
        self.len += 1;

        let first = self.taps[0];
        let last = self.taps[self.len - 1];
        #[allow(clippy::cast_precision_loss)]
        let intervals = (self.len - 1) as f64;
        let bpm = 60.0 * intervals / (last - first);
        let bpm = if self.fractional { bpm } else { bpm.round() };
        Some(Tempo::from_bpm(bpm))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn needs_two_taps() {
        let mut t = TapTempo::new(false);
        assert_eq!(t.tap(10.0), None);
        assert_eq!(t.tap(10.5), Some(Tempo::from_bpm(120.0)));
    }

    #[test]
    fn averages_recent_taps() {
        let mut t = TapTempo::new(true);
        // Uneven taps around 100 BPM (0.6 s): the average evens them out.
        let mut last = None;
        for at in [0.0, 0.62, 1.18, 1.81, 2.40] {
            last = t.tap(at);
        }
        assert_eq!(last, Some(Tempo::from_bpm(100.0)));
    }

    #[test]
    fn keeps_only_the_last_taps() {
        let mut t = TapTempo::new(false);
        // Slow taps first, then fast ones: after enough fast taps only they count.
        for at in [0.0, 1.0, 2.0] {
            let _ = t.tap(at);
        }
        let mut last = None;
        for k in 1..=TapTempo::MAX_TAPS {
            last = t.tap(2.0 + 0.25 * k as f64);
        }
        assert_eq!(last, Some(Tempo::from_bpm(240.0)));
    }

    #[test]
    fn long_pause_starts_over() {
        let mut t = TapTempo::new(false);
        let _ = t.tap(0.0);
        let _ = t.tap(0.5);
        assert_eq!(t.tap(4.0), None);
        assert_eq!(t.tap(5.0), Some(Tempo::from_bpm(60.0)));
    }

    #[test]
    fn integer_or_fractional() {
        let mut whole = TapTempo::new(false);
        let mut frac = TapTempo::new(true);
        for t in [&mut whole, &mut frac] {
            let _ = t.tap(0.0);
        }
        // 0.47 s between taps = 127.66 BPM.
        assert_eq!(whole.tap(0.47), Some(Tempo::from_bpm(128.0)));
        assert_eq!(frac.tap(0.47), Some(Tempo::from_bpm(127.7)));
    }

    #[test]
    fn ignores_bad_times() {
        let mut t = TapTempo::new(false);
        let _ = t.tap(5.0);
        assert_eq!(t.tap(f64::NAN), None);
        assert_eq!(t.tap(4.0), None, "time going back restarts");
        assert_eq!(t.tap(4.5), Some(Tempo::from_bpm(120.0)));
    }
}
