use std::fmt;

/// Tempo in beats per minute.
///
/// Always within [`Tempo::MIN`]..=[`Tempo::MAX`] and stored with a 0.1 BPM step,
/// exactly as the user can enter it. The value is kept as tenths of a BPM in an
/// integer, so equality and hashing are exact and no float noise accumulates
/// when the tempo is nudged up and down.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tempo {
    tenths: u16,
}

impl Tempo {
    /// Lowest tempo that can be entered.
    pub const MIN: Tempo = Tempo { tenths: 10 };
    /// Highest tempo that can be entered.
    pub const MAX: Tempo = Tempo { tenths: 9990 };
    /// Default lower bound of the tempo slider (classic mechanical metronome range).
    pub const SLIDER_DEFAULT_MIN: Tempo = Tempo { tenths: 400 };
    /// Default upper bound of the tempo slider.
    pub const SLIDER_DEFAULT_MAX: Tempo = Tempo { tenths: 2080 };
    /// Tempo used on first start.
    pub const DEFAULT: Tempo = Tempo { tenths: 1200 };

    /// Creates a tempo from BPM, rounding to the nearest 0.1 and clamping to the valid range.
    /// Non-finite input yields [`Tempo::DEFAULT`].
    #[must_use]
    pub fn from_bpm(bpm: f64) -> Self {
        if !bpm.is_finite() {
            return Self::DEFAULT;
        }
        let tenths = (bpm * 10.0)
            .round()
            .clamp(f64::from(Self::MIN.tenths), f64::from(Self::MAX.tenths));
        // In range 10..=9990 after the clamp, so the cast is lossless.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        Self {
            tenths: tenths as u16,
        }
    }

    /// Tempo as BPM.
    #[must_use]
    pub fn bpm(self) -> f64 {
        f64::from(self.tenths) / 10.0
    }

    /// Returns this tempo changed by `delta` BPM, clamped to the valid range.
    #[must_use]
    pub fn offset(self, delta: f64) -> Self {
        Self::from_bpm(self.bpm() + delta)
    }

    /// Length of one beat in seconds.
    #[must_use]
    pub fn beat_seconds(self) -> f64 {
        60.0 / self.bpm()
    }
}

impl Default for Tempo {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl fmt::Display for Tempo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.tenths.is_multiple_of(10) {
            write!(f, "{}", self.tenths / 10)
        } else {
            write!(f, "{}.{}", self.tenths / 10, self.tenths % 10)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounds_to_tenths() {
        assert_eq!(Tempo::from_bpm(120.04), Tempo::from_bpm(120.0));
        assert_eq!(Tempo::from_bpm(160.46).to_string(), "160.5");
    }

    #[test]
    fn clamps_to_range() {
        assert_eq!(Tempo::from_bpm(0.0), Tempo::MIN);
        assert_eq!(Tempo::from_bpm(5000.0), Tempo::MAX);
        assert_eq!(Tempo::from_bpm(f64::NAN), Tempo::DEFAULT);
    }

    #[test]
    fn offset_has_no_float_drift() {
        let mut t = Tempo::DEFAULT;
        for _ in 0..1000 {
            t = t.offset(0.1);
        }
        for _ in 0..1000 {
            t = t.offset(-0.1);
        }
        assert_eq!(t, Tempo::DEFAULT);
    }

    #[test]
    fn display() {
        assert_eq!(Tempo::from_bpm(120.0).to_string(), "120");
        assert_eq!(Tempo::from_bpm(1.0).to_string(), "1");
    }

    #[test]
    fn beat_length() {
        assert!((Tempo::from_bpm(120.0).beat_seconds() - 0.5).abs() < 1e-12);
    }
}
