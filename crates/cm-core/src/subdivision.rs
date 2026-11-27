/// Extra, quieter clicks inside each beat.
///
/// A subdivision splits the beat (not the meter's subunit cell) into equal
/// parts. Part 0 coincides with the beat itself and is never played as a
/// subdivision click; [`Subdivision::plays`] tells which other parts sound.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Subdivision {
    /// No subdivision clicks.
    #[default]
    None,
    /// One part per beat: nothing between beats.
    Single,
    /// Eighths: "one-and".
    Duple,
    /// Triplets.
    Triple,
    /// Sixteenths.
    Quadruple,
    /// Quintuplets.
    Quintuple,
    /// Sextuplets.
    Sextuple,
    /// Septuplets.
    Septuple,
    /// Triplet with the middle note left out (shuffle / swing).
    Shuffle,
    /// Four parts, only the last one played ("one … a").
    QuadrupleFourth,
}

impl Subdivision {
    /// Every variant, in menu order.
    pub const ALL: [Subdivision; 10] = [
        Self::None,
        Self::Single,
        Self::Duple,
        Self::Triple,
        Self::Quadruple,
        Self::Quintuple,
        Self::Sextuple,
        Self::Septuple,
        Self::Shuffle,
        Self::QuadrupleFourth,
    ];

    /// Number of equal parts the beat is split into.
    #[must_use]
    pub fn parts(self) -> u8 {
        match self {
            Self::None | Self::Single => 1,
            Self::Duple => 2,
            Self::Triple | Self::Shuffle => 3,
            Self::Quadruple | Self::QuadrupleFourth => 4,
            Self::Quintuple => 5,
            Self::Sextuple => 6,
            Self::Septuple => 7,
        }
    }

    /// Whether part `k` (1-based within the beat, `1..parts`) gets a subdivision click.
    #[must_use]
    pub fn plays(self, k: u8) -> bool {
        if k == 0 || k >= self.parts() {
            return false;
        }
        match self {
            Self::Shuffle => k == 2,
            Self::QuadrupleFourth => k == 3,
            _ => true,
        }
    }

    /// English menu label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Single => "Single",
            Self::Duple => "Duple",
            Self::Triple => "Triple",
            Self::Quadruple => "Quadruple",
            Self::Quintuple => "Quintuple",
            Self::Sextuple => "Sextuple",
            Self::Septuple => "Septuple",
            Self::Shuffle => "Shuffle (triplet 1st and 3rd)",
            Self::QuadrupleFourth => "Quadruple 1st and 4th",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn played(s: Subdivision) -> Vec<u8> {
        (0..8).filter(|k| s.plays(*k)).collect()
    }

    #[test]
    fn patterns() {
        assert_eq!(played(Subdivision::None), Vec::<u8>::new());
        assert_eq!(played(Subdivision::Single), Vec::<u8>::new());
        assert_eq!(played(Subdivision::Duple), vec![1]);
        assert_eq!(played(Subdivision::Triple), vec![1, 2]);
        assert_eq!(played(Subdivision::Septuple), vec![1, 2, 3, 4, 5, 6]);
        assert_eq!(played(Subdivision::Shuffle), vec![2]);
        assert_eq!(played(Subdivision::QuadrupleFourth), vec![3]);
    }
}
