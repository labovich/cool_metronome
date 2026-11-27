use crate::{Meter, Subdivision};
use serde::Deserialize;
use std::fmt;

/// The built-in library, one TOML file per category (format: see `rhythms/latin.toml`).
const BUILTIN: [&str; 2] = [
    include_str!("../rhythms/latin.toml"),
    include_str!("../rhythms/flamenco.toml"),
];

/// Group of the rhythm library.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RhythmCategory {
    /// Clave, bossa nova, samba…
    Latin,
    /// Flamenco compás.
    Flamenco,
}

impl RhythmCategory {
    /// Stable identifier (`"latin"`, `"flamenco"`), as in the data files.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::Latin => "latin",
            Self::Flamenco => "flamenco",
        }
    }
}

/// A named bar pattern from the rhythm library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rhythm {
    /// Stable identifier, unique in the library.
    pub id: String,
    /// English display name from the data file; the UI prefers a translation.
    pub name: String,
    /// Library group.
    pub category: RhythmCategory,
    /// The bar.
    pub meter: Meter,
    /// Subdivision played with it.
    pub subdivision: Subdivision,
}

/// Error in a rhythm data file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RhythmError {
    /// The file is not valid TOML or does not match the format.
    Format(String),
    /// A rhythm has an invalid field.
    Invalid {
        /// Identifier of the rhythm.
        id: String,
        /// What is wrong.
        reason: String,
    },
}

impl fmt::Display for RhythmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Format(e) => write!(f, "invalid rhythm file: {e}"),
            Self::Invalid { id, reason } => write!(f, "rhythm \"{id}\": {reason}"),
        }
    }
}

impl std::error::Error for RhythmError {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFile {
    category: RhythmCategory,
    rhythm: Vec<RawRhythm>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRhythm {
    id: String,
    name: String,
    subunits: u8,
    grid: String,
    subdivision: Option<String>,
}

impl Rhythm {
    /// The built-in rhythm library, in menu order.
    ///
    /// # Panics
    /// Never in practice: the embedded files are validated by the tests.
    #[must_use]
    pub fn builtin() -> Vec<Rhythm> {
        BUILTIN
            .iter()
            .flat_map(|src| parse_rhythms(src).expect("built-in rhythm data is valid"))
            .collect()
    }
}

/// Parses one rhythm data file.
///
/// # Errors
/// If the file is not valid TOML, has unknown fields, or a rhythm has an
/// invalid grid, subunit count or subdivision.
pub fn parse_rhythms(src: &str) -> Result<Vec<Rhythm>, RhythmError> {
    let file: RawFile = toml::from_str(src).map_err(|e| RhythmError::Format(e.to_string()))?;
    file.rhythm
        .into_iter()
        .map(|raw| {
            let invalid = |reason: String| RhythmError::Invalid {
                id: raw.id.clone(),
                reason,
            };
            let meter =
                Meter::from_grid(raw.subunits, &raw.grid).map_err(|e| invalid(e.to_string()))?;
            let subdivision = match raw.subdivision.as_deref() {
                None => Subdivision::None,
                Some(s) => Subdivision::from_id(s)
                    .ok_or_else(|| invalid(format!("unknown subdivision \"{s}\"")))?,
            };
            Ok(Rhythm {
                id: raw.id,
                name: raw.name,
                category: file.category,
                meter,
                subdivision,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Positions of the sounding cells.
    fn strokes(r: &Rhythm) -> Vec<usize> {
        r.meter
            .cells()
            .iter()
            .enumerate()
            .filter(|(_, c)| **c != crate::BeatLevel::Mute)
            .map(|(i, _)| i)
            .collect()
    }

    fn get<'a>(lib: &'a [Rhythm], id: &str) -> &'a Rhythm {
        lib.iter().find(|r| r.id == id).expect(id)
    }

    #[test]
    fn builtin_library_is_valid_and_complete() {
        for src in BUILTIN {
            parse_rhythms(src).unwrap();
        }
        let lib = Rhythm::builtin();
        let ids: HashSet<&str> = lib.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids.len(), lib.len(), "ids must be unique");
        for id in [
            "son-clave-3-2",
            "son-clave-2-3",
            "rumba-clave-3-2",
            "rumba-clave-2-3",
            "afro-6-8",
            "bossa-nova-3-2",
            "bossa-nova-2-3",
            "samba",
            "merengue",
            "solea",
            "seguiriya",
            "buleria",
        ] {
            assert!(ids.contains(id), "missing {id}");
        }
    }

    #[test]
    fn clave_patterns_match_the_reference() {
        let lib = Rhythm::builtin();
        let cases: [(&str, &[usize]); 6] = [
            ("son-clave-3-2", &[0, 3, 6, 10, 12]),
            ("son-clave-2-3", &[2, 4, 8, 11, 14]),
            ("rumba-clave-3-2", &[0, 3, 7, 10, 12]),
            ("rumba-clave-2-3", &[2, 4, 8, 11, 15]),
            ("bossa-nova-3-2", &[0, 3, 6, 10, 13]),
            ("bossa-nova-2-3", &[2, 5, 8, 11, 14]),
        ];
        for (id, expected) in cases {
            let r = get(&lib, id);
            assert_eq!(r.meter.subunits(), 4, "{id}");
            assert_eq!(r.meter.len(), 16, "{id}");
            assert_eq!(strokes(r), expected, "{id}");
            // Only the first stroke is accented, so no bell is added.
            assert!(!r.meter.is_compound(), "{id}");
        }
    }

    #[test]
    fn flamenco_compas_has_twelve_counts() {
        let lib = Rhythm::builtin();
        for id in ["solea", "seguiriya", "buleria"] {
            let r = get(&lib, id);
            assert_eq!(r.category, RhythmCategory::Flamenco);
            assert_eq!((r.meter.subunits(), r.meter.len()), (1, 12), "{id}");
        }
        let accents = |id| {
            get(&lib, id)
                .meter
                .cells()
                .iter()
                .enumerate()
                .filter(|(_, c)| **c == crate::BeatLevel::Accent)
                .map(|(i, _)| i + 1)
                .collect::<Vec<_>>()
        };
        assert_eq!(accents("solea"), [3, 6, 8, 10, 12]);
        assert_eq!(accents("seguiriya"), [1, 3, 5, 8, 11]);
        // Bulería starts on count 12: cell 1 is count 12, cell n is count n-1.
        assert_eq!(accents("buleria"), [1, 4, 7, 9, 11]);
    }

    #[test]
    fn rejects_bad_files() {
        let ok = "category = \"latin\"\n[[rhythm]]\nid = \"a\"\nname = \"A\"\nsubunits = 1\ngrid = \"Xx\"\n";
        assert_eq!(parse_rhythms(ok).unwrap()[0].subdivision, Subdivision::None);
        let with = |extra: &str| format!("{ok}{extra}\n");
        assert!(matches!(
            parse_rhythms(&with("subdivision = \"duple\"")).unwrap()[0].subdivision,
            Subdivision::Duple
        ));
        assert!(matches!(
            parse_rhythms(&with("subdivision = \"eighths\"")),
            Err(RhythmError::Invalid { .. })
        ));
        assert!(matches!(
            parse_rhythms(&with("tempo = 120")),
            Err(RhythmError::Format(_))
        ));
        assert!(matches!(
            parse_rhythms(&ok.replace("Xx", "Xo")),
            Err(RhythmError::Invalid { .. })
        ));
        assert!(matches!(
            parse_rhythms(&ok.replace("latin", "jazz")),
            Err(RhythmError::Format(_))
        ));
    }
}
