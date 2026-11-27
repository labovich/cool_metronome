//! Owns the audio output and keeps it in sync with the metronome settings.

use cm_audio::{AudioError, AudioOutput, Decoded, EngineSetup, SampleError, Waker};
use cm_core::{Meter, Rhythm, Subdivision, TapTempo, Tempo};
use cm_engine::{BarPattern, ClickKind, Command, EngineStatus, SoundBank};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// An own sound in a slot: its file and the decoded audio (or why not).
#[derive(Debug)]
struct OwnSound {
    path: PathBuf,
    decoded: Result<Decoded, SampleError>,
}

/// Why there is no sound.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayerError {
    /// The output could not be opened (retried periodically).
    Audio(AudioError),
    /// The command queue is full: the audio thread is stuck.
    NotResponding,
}

#[derive(Debug)]
pub struct Player {
    audio: Option<AudioOutput>,
    waker: Waker,
    error: Option<PlayerError>,
    last_attempt: Option<Instant>,
    /// Set after a command that may make the engine retire an object;
    /// one more [`Player::tick`] is due then to drop it.
    cleanup_after: Option<Instant>,
    tempo: Tempo,
    meter: Meter,
    subdivision: Subdivision,
    bell: bool,
    playing: bool,
    /// Master volume slider position, 0 (mute) … 1 (0 dB).
    volume: f32,
    tap: TapTempo,
    tap_epoch: Instant,
    /// Sound slot levels in dB: accent, normal, subdivision, bell.
    slots_db: [f32; 4],
    /// Stereo position, −1 … 1.
    pan: f32,
    /// +6 dB on top of the master volume.
    boost: bool,
    /// Fade-in after Start, seconds (0 = off).
    fade_in: f32,
    /// Accented beats also play the normal sound.
    accent_mix: bool,
    /// Chosen output device; `None` is the system default.
    device: Option<String>,
    /// Requested buffer size in frames; `None` is the device default.
    buffer: Option<u32>,
    /// Own sounds per slot (accent, normal, subdivision, bell); `None` is built in.
    sounds: [Option<OwnSound>; 4],
    /// The chosen device is missing and the system default plays instead.
    on_fallback: bool,
    /// Elapsed time shown while stopped (the length of the last run).
    elapsed_stopped: Duration,
}

/// Wake-up interval while waiting for the engine to publish a beat.
const POLL: Duration = Duration::from_millis(5);

impl Player {
    /// How often to retry opening the audio device after a failure.
    pub const RETRY_INTERVAL: Duration = Duration::from_secs(2);
    /// How long after a command to collect what the engine replaced.
    /// The audio callback runs every few milliseconds, so it is done by then.
    const CLEANUP_DELAY: Duration = Duration::from_millis(100);
    /// Volume range of the master slider above mute.
    const VOLUME_RANGE_DB: f32 = 40.0;

    /// Starts stopped with the given settings. `waker` must make the owner
    /// call [`Player::tick`] soon; it is called from another thread when the
    /// audio device fails.
    pub fn new(
        waker: Waker,
        tempo: Tempo,
        meter: Meter,
        subdivision: Subdivision,
        volume: f32,
    ) -> Self {
        let mut p = Self {
            audio: None,
            waker,
            error: None,
            last_attempt: None,
            cleanup_after: None,
            tempo,
            meter,
            subdivision,
            bell: true,
            playing: false,
            volume: volume.clamp(0.0, 1.0),
            tap: TapTempo::new(false),
            tap_epoch: Instant::now(),
            slots_db: [0.0, -5.0, -15.0, -9.0],
            pan: 0.0,
            boost: false,
            fade_in: 0.0,
            accent_mix: false,
            device: None,
            buffer: None,
            sounds: Default::default(),
            on_fallback: false,
            elapsed_stopped: Duration::ZERO,
        };
        p.open();
        p
    }

    /// Frees replaced engine objects and recovers from a lost device
    /// (unplugged headphones, changed default output). Call it on every UI
    /// pass, including while the window is hidden, and again after
    /// [`Player::next_tick`].
    pub fn tick(&mut self) {
        if let Some(audio) = &mut self.audio {
            audio.collect_garbage();
            if self
                .cleanup_after
                .is_some_and(|t| t.elapsed() >= Self::CLEANUP_DELAY)
            {
                self.cleanup_after = None;
            }
            if audio.has_failed() {
                self.audio = None;
                self.open();
            }
        } else if self
            .last_attempt
            .is_none_or(|t| t.elapsed() >= Self::RETRY_INTERVAL)
        {
            self.open();
        }
    }

    /// When [`Player::tick`] must run again even if nothing else happens
    /// (besides a wake-up through the [`Waker`]). `None`: not needed.
    pub fn next_tick(&self) -> Option<Duration> {
        if self.audio.is_none() {
            return Some(Self::RETRY_INTERVAL);
        }
        self.cleanup_after
            .map(|t| Self::CLEANUP_DELAY.saturating_sub(t.elapsed()))
    }

    fn open(&mut self) {
        self.last_attempt = Some(Instant::now());
        let (tempo, pattern) = (self.tempo, self.pattern());
        let setup = || EngineSetup {
            tempo,
            pattern: pattern.clone(),
            sounds: None,
        };
        let waker = Some(self.waker.clone());
        let mut result =
            AudioOutput::open(self.device.as_deref(), self.buffer, setup(), waker.clone());
        // A missing chosen device: play on the system default instead.
        self.on_fallback = false;
        if matches!(result, Err(AudioError::DeviceNotFound(_))) {
            result = AudioOutput::open(None, self.buffer, setup(), waker);
            self.on_fallback = result.is_ok();
        }
        match result {
            Ok(mut audio) => {
                let _ = audio.send(Command::SetMaster(self.master_gain()));
                let _ = audio.send(Command::SetFadeIn(self.fade_in));
                let _ = audio.send(Command::SetAccentMix(self.accent_mix));
                for (slot, db) in self.slots_db.into_iter().enumerate() {
                    let _ = audio.send(Command::SetGain(SLOTS[slot], db_gain(db)));
                }
                let _ = audio.send(Command::SetPan(self.pan));
                if self.playing {
                    let _ = audio.send(Command::Start);
                }
                self.audio = Some(audio);
                self.error = None;
                if self.sounds.iter().any(Option::is_some) {
                    self.push_sounds();
                }
            }
            Err(e) => {
                self.audio = None;
                self.error = Some(PlayerError::Audio(e));
            }
        }
    }

    fn pattern(&self) -> BarPattern {
        BarPattern::new(&self.meter, self.subdivision, self.bell)
    }

    fn send(&mut self, command: Command) {
        let Some(audio) = &mut self.audio else {
            return;
        };
        if matches!(
            command,
            Command::Start | Command::Stop | Command::SetPattern(_) | Command::SetSounds(_)
        ) {
            self.cleanup_after = Some(Instant::now());
        }
        // While a device is open, the only possible error is this one.
        self.error = audio
            .send(command)
            .is_err()
            .then_some(PlayerError::NotResponding);
    }

    fn push_pattern(&mut self) {
        let pattern = Box::new(self.pattern());
        self.send(Command::SetPattern(pattern));
    }

    pub fn toggle(&mut self) {
        self.elapsed_stopped = if self.playing {
            self.elapsed(Instant::now())
        } else {
            Duration::ZERO
        };
        self.playing = !self.playing;
        self.send(if self.playing {
            Command::Start
        } else {
            Command::Stop
        });
    }

    pub fn is_playing(&self) -> bool {
        self.playing
    }

    pub fn tempo(&self) -> Tempo {
        self.tempo
    }

    pub fn set_tempo(&mut self, tempo: Tempo) {
        if tempo != self.tempo {
            self.tempo = tempo;
            self.send(Command::SetTempo(tempo));
        }
    }

    pub fn meter(&self) -> &Meter {
        &self.meter
    }

    pub fn set_meter(&mut self, meter: Meter) {
        if meter != self.meter {
            self.meter = meter;
            self.push_pattern();
        }
    }

    /// Switches to a library rhythm (its meter and subdivision) in one step.
    pub fn set_rhythm(&mut self, rhythm: &Rhythm) {
        if rhythm.meter != self.meter || rhythm.subdivision != self.subdivision {
            self.meter = rhythm.meter.clone();
            self.subdivision = rhythm.subdivision;
            self.push_pattern();
        }
    }

    pub fn cycle_cell(&mut self, index: usize) {
        if let Some(level) = self.meter.cells().get(index).copied() {
            self.meter.set_level(index, level.cycled());
            self.push_pattern();
        }
    }

    pub fn subdivision(&self) -> Subdivision {
        self.subdivision
    }

    pub fn set_subdivision(&mut self, subdivision: Subdivision) {
        if subdivision != self.subdivision {
            self.subdivision = subdivision;
            self.push_pattern();
        }
    }

    /// Master volume slider position, 0 (mute) … 1.
    pub fn volume(&self) -> f32 {
        self.volume
    }

    pub fn set_volume(&mut self, volume: f32) {
        let volume = volume.clamp(0.0, 1.0);
        if (volume - self.volume).abs() > f32::EPSILON {
            self.volume = volume;
            self.send(Command::SetMaster(self.master_gain()));
        }
    }

    /// Level of a sound slot (accent, normal, subdivision, bell) in dB.
    pub fn set_slot_db(&mut self, slot: usize, db: f32) {
        if let Some(current) = self.slots_db.get_mut(slot)
            && (*current - db).abs() > f32::EPSILON
        {
            *current = db;
            self.send(Command::SetGain(SLOTS[slot], db_gain(db)));
        }
    }

    pub fn set_pan(&mut self, pan: f32) {
        if (pan - self.pan).abs() > f32::EPSILON {
            self.pan = pan;
            self.send(Command::SetPan(pan));
        }
    }

    /// Bell on the bar start in compound meters.
    pub fn set_bell(&mut self, bell: bool) {
        if bell != self.bell {
            self.bell = bell;
            self.push_pattern();
        }
    }

    /// +6 dB on top of the master volume.
    pub fn set_boost(&mut self, boost: bool) {
        if boost != self.boost {
            self.boost = boost;
            self.send(Command::SetMaster(self.master_gain()));
        }
    }

    /// Fade-in after Start, seconds (0 = off).
    pub fn set_fade_in(&mut self, seconds: f32) {
        if (seconds - self.fade_in).abs() > f32::EPSILON {
            self.fade_in = seconds;
            self.send(Command::SetFadeIn(seconds));
        }
    }

    /// Accented beats also play the normal beat sound.
    pub fn set_accent_mix(&mut self, on: bool) {
        if on != self.accent_mix {
            self.accent_mix = on;
            self.send(Command::SetAccentMix(on));
        }
    }

    /// Plays one sound slot (accent, normal, subdivision, bell) now.
    pub fn preview(&mut self, slot: usize) {
        if let Some(kind) = SLOTS.get(slot) {
            self.send(Command::Preview(*kind));
        }
    }

    fn master_gain(&self) -> f32 {
        volume_gain(self.volume) * if self.boost { 2.0 } else { 1.0 }
    }

    /// Plays on `device` (`None`: the system default), reopening the output.
    pub fn set_device(&mut self, device: Option<String>) {
        if device != self.device {
            self.device = device;
            self.audio = None;
            self.open();
        }
    }

    /// Asks for a buffer of `frames` (`None`: the device default), reopening
    /// the output.
    pub fn set_buffer(&mut self, frames: Option<u32>) {
        if frames != self.buffer {
            self.buffer = frames;
            self.audio = None;
            self.open();
        }
    }

    /// Uses the sound file at `path` for a slot (`None`: the built-in sound).
    /// The file is decoded once here; a bad file keeps the built-in sound
    /// and reports why (see [`Player::sound_error`]).
    pub fn set_sound(&mut self, slot: usize, path: Option<&Path>) {
        let Some(current) = self.sounds.get_mut(slot) else {
            return;
        };
        if current.as_ref().map(|s| s.path.as_path()) == path {
            return;
        }
        *current = path.map(|p| OwnSound {
            path: p.to_path_buf(),
            decoded: cm_audio::decode_file(p),
        });
        self.push_sounds();
    }

    /// Why the own sound of a slot could not be used, if it could not.
    pub fn sound_error(&self, slot: usize) -> Option<&SampleError> {
        self.sounds.get(slot)?.as_ref()?.decoded.as_ref().err()
    }

    /// Sends the sound bank for the current output rate to the engine.
    fn push_sounds(&mut self) {
        let Some(rate) = self.audio.as_ref().map(AudioOutput::sample_rate) else {
            return;
        };
        let builtin = SoundBank::synthesized(rate);
        let sounds = std::array::from_fn(|i| match &self.sounds[i] {
            Some(OwnSound { decoded: Ok(d), .. }) => cm_audio::prepare(d, rate),
            _ => builtin.sound(i).to_vec(),
        });
        self.send(Command::SetSounds(Box::new(SoundBank::from_sounds(sounds))));
    }

    /// Frames per audio callback of the open output, and the sample rate.
    pub fn block(&self) -> Option<(u32, u32)> {
        let audio = self.audio.as_ref()?;
        Some((audio.block_frames()?, audio.sample_rate()))
    }

    /// Tap tempo result to 0.1 BPM instead of whole BPM.
    pub fn set_tap_fractional(&mut self, fractional: bool) {
        self.tap = TapTempo::new(fractional);
    }

    /// "Device · 48000 Hz · 2 ch" of the open output.
    pub fn output_info(&self) -> Option<String> {
        self.audio.as_ref().map(|a| {
            format!(
                "{} · {} Hz · {} ch",
                a.device_name(),
                a.sample_rate(),
                a.channels()
            )
        })
    }

    /// The chosen device is missing; the system default plays instead.
    pub fn on_fallback(&self) -> bool {
        self.on_fallback
    }

    /// Registers a tap; sets the tempo once two or more taps are in.
    pub fn tap(&mut self, now: Instant) {
        let at = now.duration_since(self.tap_epoch).as_secs_f64();
        if let Some(tempo) = self.tap.tap(at) {
            self.set_tempo(tempo);
        }
    }

    pub fn error(&self) -> Option<&PlayerError> {
        self.error.as_ref()
    }

    /// The engine state of the current run, if the engine is playing it.
    fn running_status(&self) -> Option<&EngineStatus> {
        if !self.playing {
            return None;
        }
        let status = self.audio.as_ref()?.status();
        status.is_playing().then_some(&**status)
    }

    /// The meter cell being heard now (not merely rendered), while playing.
    pub fn audible_cell(&self, now: Instant) -> Option<usize> {
        let status = self.running_status()?;
        let started = status.started_at();
        let heard = |sample| status.heard_at(sample).unwrap_or(now);
        let last = status.last_beat().filter(|b| b.sample >= started)?;
        if heard(last.sample) <= now {
            return Some(usize::from(last.cell));
        }
        status
            .previous_beat()
            .filter(|b| b.sample >= started && heard(b.sample) <= now)
            .map(|b| usize::from(b.cell))
    }

    /// How long until the display changes by itself (next beat heard, next
    /// timer second). `None` while stopped.
    pub fn next_visual_change(&self, now: Instant) -> Option<Duration> {
        let status = self.running_status()?;
        let into_second = Duration::from_nanos(u64::from(self.elapsed(now).subsec_nanos()));
        let mut wake = Duration::from_secs(1) - into_second;

        let started = status.started_at();
        match status.last_beat().filter(|b| b.sample >= started) {
            None => wake = wake.min(POLL),
            Some(beat) => {
                let heard = status.heard_at(beat.sample).unwrap_or(now);
                let until = if heard > now {
                    heard - now
                } else {
                    // Predict the next cell; if it is late, poll briefly.
                    let cell = Duration::from_secs_f64(
                        self.tempo.beat_seconds() / f64::from(self.meter.subunits()),
                    );
                    (heard + cell)
                        .checked_duration_since(now)
                        .filter(|d| !d.is_zero())
                        .unwrap_or(POLL)
                };
                wake = wake.min(until);
            }
        }
        Some(wake)
    }

    /// Time since Play, as heard; frozen at the last run's length while stopped.
    pub fn elapsed(&self, now: Instant) -> Duration {
        match self.running_status() {
            Some(status) => status
                .heard_at(status.started_at())
                .map_or(Duration::ZERO, |start| now.saturating_duration_since(start)),
            None => self.elapsed_stopped,
        }
    }
}

/// Engine sound slots in settings order: accent, normal, subdivision, bell.
const SLOTS: [ClickKind; 4] = [
    ClickKind::Accent,
    ClickKind::Normal,
    ClickKind::Sub,
    ClickKind::Bell,
];

/// Decibels → linear gain.
fn db_gain(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

/// Slider position → linear gain: 0 is silence, then −40 dB … 0 dB.
fn volume_gain(position: f32) -> f32 {
    if position <= 0.0 {
        0.0
    } else {
        10f32.powf((position.min(1.0) - 1.0) * Player::VOLUME_RANGE_DB / 20.0)
    }
}

#[cfg(test)]
mod tests {
    use super::volume_gain;

    #[test]
    fn decibels() {
        assert!((super::db_gain(0.0) - 1.0).abs() < 1e-6);
        assert!((super::db_gain(-20.0) - 0.1).abs() < 1e-6);
        // The engine defaults are the same levels as the settings defaults.
        let defaults = [0.0, -5.0, -15.0, -9.0].map(super::db_gain);
        for (a, b) in defaults.iter().zip(cm_engine::Engine::DEFAULT_GAINS) {
            assert!((a - b).abs() < 1e-3, "{a} vs {b}");
        }
    }

    #[test]
    fn volume_curve() {
        assert!(volume_gain(0.0).abs() < f32::EPSILON);
        assert!((volume_gain(1.0) - 1.0).abs() < 1e-6);
        assert!((volume_gain(0.5) - 0.1).abs() < 1e-6, "−20 dB at half");
        assert!(volume_gain(0.01) > 0.0);
    }
}
