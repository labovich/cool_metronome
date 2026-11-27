//! Audio output for Cool Metronome.
//!
//! Will hold the `cpal` output stream, device selection and buffer size,
//! decoding and resampling of user samples, and the lock-free command queue
//! between the UI and the audio callback. Implemented in stage 2.
