use std::fmt;

/// How a cell of the meter grid sounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum BeatLevel {
    /// Accented (strong) click.
    Accent,
    /// Normal click.
    #[default]
    Normal,
    /// Silent cell.
    Mute,
}

impl BeatLevel {
    /// Next level in the click cycle used by the UI: normal → accent → mute → normal.
    #[must_use]
    pub fn cycled(self) -> Self {
        match self {
            Self::Normal => Self::Accent,
            Self::Accent => Self::Mute,
            Self::Mute => Self::Normal,
        }
    }
}

/// Error when building a [`Meter`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MeterError {
    /// The grouping formula could not be parsed.
    BadFormula(String),
    /// A cell grid has a character other than `X`, `x`, `.`, space or `|`.
    BadGrid(String),
    /// No cells, or more than [`Meter::MAX_CELLS`].
    BadLength(usize),
    /// Subunits per beat outside 1..=[`Meter::MAX_SUBUNITS`].
    BadSubunits(u8),
}

impl fmt::Display for MeterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadFormula(s) => write!(f, "cannot parse meter formula \"{s}\""),
            Self::BadGrid(s) => write!(
                f,
                "cannot parse cell grid \"{s}\" (use X accent, x normal, . mute)"
            ),
            Self::BadLength(n) => {
                write!(f, "a bar must have 1..={} cells, got {n}", Meter::MAX_CELLS)
            }
            Self::BadSubunits(n) => {
                write!(
                    f,
                    "subunits per beat must be 1..={}, got {n}",
                    Meter::MAX_SUBUNITS
                )
            }
        }
    }
}

impl std::error::Error for MeterError {}

/// One bar of the metronome pattern.
///
/// The bar is a grid of cells. Each beat is split into `subunits` cells
/// (1 for ordinary meters, 4 for clave patterns written in sixteenths).
/// The tempo counts beats, so a cell lasts `1 / subunits` of a beat.
/// The number of cells need not be a multiple of `subunits`: a "cut" last
/// beat gives 5/8 or 7/8 at the quarter-note tempo.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Meter {
    subunits: u8,
    cells: Vec<BeatLevel>,
}

impl Meter {
    /// Most cells a bar may have.
    pub const MAX_CELLS: usize = 128;
    /// Most cells per beat.
    pub const MAX_SUBUNITS: u8 = 8;

    /// Creates a meter from its cells.
    ///
    /// # Errors
    /// If the cell count or `subunits` is out of range.
    pub fn new(subunits: u8, cells: Vec<BeatLevel>) -> Result<Self, MeterError> {
        if !(1..=Self::MAX_SUBUNITS).contains(&subunits) {
            return Err(MeterError::BadSubunits(subunits));
        }
        if cells.is_empty() || cells.len() > Self::MAX_CELLS {
            return Err(MeterError::BadLength(cells.len()));
        }
        Ok(Self { subunits, cells })
    }

    /// `beats` beats per bar, first accented.
    ///
    /// # Errors
    /// If `beats` is 0 or above [`Meter::MAX_CELLS`].
    pub fn simple(beats: usize) -> Result<Self, MeterError> {
        if beats == 0 || beats > Self::MAX_CELLS {
            return Err(MeterError::BadLength(beats));
        }
        let mut cells = vec![BeatLevel::Normal; beats];
        cells[0] = BeatLevel::Accent;
        Self::new(1, cells)
    }

    /// Parses a grouping formula such as `"4"`, `"3+2"` or `"2+2+3"`:
    /// one beat per unit, each group starts with an accent.
    ///
    /// # Errors
    /// If the formula is empty, has a zero or non-numeric group, or is too long.
    pub fn from_formula(formula: &str) -> Result<Self, MeterError> {
        let bad = || MeterError::BadFormula(formula.to_owned());
        let mut cells = Vec::new();
        for group in formula.split('+') {
            let n: usize = group.trim().parse().map_err(|_| bad())?;
            if n == 0 {
                return Err(bad());
            }
            if cells.len() + n > Self::MAX_CELLS {
                return Err(MeterError::BadLength(cells.len() + n));
            }
            cells.push(BeatLevel::Accent);
            cells.extend(std::iter::repeat_n(BeatLevel::Normal, n - 1));
        }
        Self::new(1, cells)
    }

    /// Parses a cell grid: `X` accent, `x` normal, `.` mute, one character
    /// per cell. Spaces and `|` are ignored, so beats can be set apart:
    /// `"X..x ..x. ..x. x..."`.
    ///
    /// # Errors
    /// On any other character, or if the cell count or `subunits` is out of range.
    pub fn from_grid(subunits: u8, grid: &str) -> Result<Self, MeterError> {
        let mut cells = Vec::new();
        for ch in grid.chars() {
            let level = match ch {
                'X' => BeatLevel::Accent,
                'x' => BeatLevel::Normal,
                '.' => BeatLevel::Mute,
                c if c.is_whitespace() || c == '|' => continue,
                _ => return Err(MeterError::BadGrid(grid.to_owned())),
            };
            if cells.len() == Self::MAX_CELLS {
                return Err(MeterError::BadLength(Self::MAX_CELLS + 1));
            }
            cells.push(level);
        }
        Self::new(subunits, cells)
    }

    /// The cells as a grid string (see [`Meter::from_grid`]), one space between beats.
    #[must_use]
    pub fn grid(&self) -> String {
        let mut out = String::with_capacity(self.cells.len() * 2);
        for (i, cell) in self.cells.iter().enumerate() {
            if i > 0 && i % usize::from(self.subunits) == 0 {
                out.push(' ');
            }
            out.push(match cell {
                BeatLevel::Accent => 'X',
                BeatLevel::Normal => 'x',
                BeatLevel::Mute => '.',
            });
        }
        out
    }

    /// Cells per beat.
    #[must_use]
    pub fn subunits(&self) -> u8 {
        self.subunits
    }

    /// All cells of the bar.
    #[must_use]
    pub fn cells(&self) -> &[BeatLevel] {
        &self.cells
    }

    /// Number of cells in the bar.
    #[must_use]
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    /// Always false: a meter has at least one cell.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// Sets the level of cell `index`. Out-of-range indexes are ignored.
    pub fn set_level(&mut self, index: usize, level: BeatLevel) {
        if let Some(cell) = self.cells.get_mut(index) {
            *cell = level;
        }
    }

    /// Bar length in beats (may be fractional for a cut last beat).
    #[must_use]
    pub fn bar_beats(&self) -> f64 {
        // len <= MAX_CELLS, exactly representable.
        #[allow(clippy::cast_precision_loss)]
        let len = self.cells.len() as f64;
        len / f64::from(self.subunits)
    }

    /// Number of beats that start inside the bar (a cut beat counts).
    #[must_use]
    pub fn beat_count(&self) -> usize {
        self.cells.len().div_ceil(usize::from(self.subunits))
    }

    /// True when more than one beat start carries an accent (3+2, 2+2+3…):
    /// the bell then marks the bar start.
    #[must_use]
    pub fn is_compound(&self) -> bool {
        self.cells
            .iter()
            .step_by(usize::from(self.subunits))
            .filter(|c| **c == BeatLevel::Accent)
            .count()
            > 1
    }

    /// Short label: the grouping formula when the meter is one, else `"N/M"`-like cell count.
    #[must_use]
    pub fn label(&self) -> String {
        if self.subunits == 1 && self.cells.first() == Some(&BeatLevel::Accent) {
            let mut groups = Vec::new();
            let mut run = 0usize;
            for cell in &self.cells {
                if *cell == BeatLevel::Accent && run > 0 {
                    groups.push(run.to_string());
                    run = 0;
                }
                run += 1;
            }
            groups.push(run.to_string());
            groups.join("+")
        } else {
            format!("{} × 1/{}", self.cells.len(), self.subunits)
        }
    }
}

impl Default for Meter {
    fn default() -> Self {
        Self::from_formula("4").expect("valid default meter")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use BeatLevel::{Accent as A, Mute as M, Normal as N};

    #[test]
    fn formula_parsing() {
        assert_eq!(Meter::from_formula("4").unwrap().cells(), &[A, N, N, N]);
        assert_eq!(
            Meter::from_formula(" 3 + 2 ").unwrap().cells(),
            &[A, N, N, A, N]
        );
        assert!(Meter::from_formula("").is_err());
        assert!(Meter::from_formula("3+0").is_err());
        assert!(Meter::from_formula("x").is_err());
        assert!(Meter::from_formula("100+100").is_err());
    }

    #[test]
    fn labels_round_trip() {
        for f in ["4", "3+2", "2+2+3", "1"] {
            assert_eq!(Meter::from_formula(f).unwrap().label(), f);
        }
    }

    #[test]
    fn cut_bar_length() {
        // 7/8 written as 2 subunits per quarter-note beat: 3.5 beats, 4 beat starts.
        let m = Meter::new(2, vec![A, N, N, N, A, N, N]).unwrap();
        assert!((m.bar_beats() - 3.5).abs() < 1e-12);
        assert_eq!(m.beat_count(), 4);
    }

    #[test]
    fn compound_detection() {
        assert!(!Meter::from_formula("4").unwrap().is_compound());
        assert!(Meter::from_formula("3+2").unwrap().is_compound());
        // Accents off the beat start do not make it compound.
        let clave = Meter::new(2, vec![A, N, N, A, M, N]).unwrap();
        assert!(!clave.is_compound());
    }

    #[test]
    fn level_cycle() {
        assert_eq!(N.cycled(), A);
        assert_eq!(A.cycled(), M);
        assert_eq!(M.cycled(), N);
    }

    #[test]
    fn grid_parsing() {
        let m = Meter::from_grid(4, "X..x | ..x. ..x. x...").unwrap();
        assert_eq!(m.subunits(), 4);
        assert_eq!(m.len(), 16);
        assert_eq!(&m.cells()[..4], &[A, M, M, N]);
        assert_eq!(m.grid(), "X..x ..x. ..x. x...");
        assert_eq!(Meter::from_grid(4, &m.grid()).unwrap(), m);
        assert!(matches!(
            Meter::from_grid(1, "Xo"),
            Err(MeterError::BadGrid(_))
        ));
        assert!(Meter::from_grid(1, " | ").is_err());
        assert!(Meter::from_grid(1, &"x".repeat(Meter::MAX_CELLS + 1)).is_err());
    }

    #[test]
    fn rejects_bad_subunits() {
        assert!(Meter::new(0, vec![A]).is_err());
        assert!(Meter::new(9, vec![A]).is_err());
    }
}
