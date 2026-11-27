//! Sample-accurate scheduling for Cool Metronome.
//!
//! The engine never touches an audio device. It turns the rhythm model into
//! click events at exact sample positions and mixes them into buffers. Live
//! playback, file export and the accuracy tests all run this same code.

mod grid;

pub use grid::BeatGrid;
