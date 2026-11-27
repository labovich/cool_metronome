//! Sample-accurate scheduling and mixing for Cool Metronome.
//!
//! The engine never touches an audio device. It turns the rhythm model into
//! click events at exact sample positions and mixes them into buffers. Live
//! playback, file export and the accuracy tests all run this same code.

mod engine;
mod grid;
mod pattern;
mod scheduler;
mod sounds;

pub use engine::{
    BeatInfo, Command, Engine, EngineStatus, FADE_FLOOR_DB, MAX_FADE_SECONDS, MAX_VOICES, Retired,
    fade_gain,
};
pub use grid::BeatGrid;
pub use pattern::{BarPattern, ClickKind, PatternEvent};
pub use scheduler::{ScheduledEvent, Scheduler};
pub use sounds::SoundBank;
