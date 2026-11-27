//! Rhythm model for Cool Metronome.
//!
//! This crate is pure data and math: no audio devices, no files, no UI.

mod meter;
mod subdivision;
mod tempo;

pub use meter::{BeatLevel, Meter, MeterError};
pub use subdivision::Subdivision;
pub use tempo::Tempo;
