//! Rhythm model for Cool Metronome.
//!
//! This crate is pure data and math: no audio devices, no files, no UI.
//! Meters, subdivisions, the rhythm library and presets will live here.

mod tempo;

pub use tempo::Tempo;
