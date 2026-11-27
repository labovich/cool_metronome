//! Rhythm model for Cool Metronome.
//!
//! This crate is pure data and math: no audio devices, no file I/O, no UI.
//! The built-in rhythm library is embedded at compile time.

mod meter;
mod rhythm;
mod subdivision;
mod tempo;

pub use meter::{BeatLevel, Meter, MeterError};
pub use rhythm::{Rhythm, RhythmCategory, RhythmError, parse_rhythms};
pub use subdivision::Subdivision;
pub use tempo::Tempo;
