//! Owns the audio output and keeps it in sync with the metronome settings.

use cm_audio::{AudioOutput, EngineSetup, Waker};
use cm_core::{Meter, Subdivision, Tempo};
use cm_engine::{BarPattern, Command};
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct Player {
    audio: Option<AudioOutput>,
    waker: Waker,
    error: Option<String>,
    last_attempt: Option<Instant>,
    /// Set after a command that may make the engine retire an object;
    /// one more [`Player::tick`] is due then to drop it.
    cleanup_after: Option<Instant>,
    tempo: Tempo,
    meter: Meter,
    subdivision: Subdivision,
    bell: bool,
    playing: bool,
}

impl Player {
    /// How often to retry opening the audio device after a failure.
    pub const RETRY_INTERVAL: Duration = Duration::from_secs(2);
    /// How long after a command to collect what the engine replaced.
    /// The audio callback runs every few milliseconds, so it is done by then.
    const CLEANUP_DELAY: Duration = Duration::from_millis(100);

    /// `waker` must make the owner call [`Player::tick`] soon; it is called
    /// from another thread when the audio device fails.
    pub fn new(waker: Waker) -> Self {
        let mut p = Self {
            audio: None,
            waker,
            error: None,
            last_attempt: None,
            cleanup_after: None,
            tempo: Tempo::DEFAULT,
            meter: Meter::default(),
            subdivision: Subdivision::None,
            bell: true,
            playing: false,
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
        let setup = EngineSetup {
            tempo: self.tempo,
            pattern: self.pattern(),
            sounds: None,
        };
        match AudioOutput::open(None, setup, Some(self.waker.clone())) {
            Ok(mut audio) => {
                if self.playing {
                    let _ = audio.send(Command::Start);
                }
                self.audio = Some(audio);
                self.error = None;
            }
            Err(e) => {
                self.audio = None;
                self.error = Some(e.to_string());
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
            .then(|| "the audio engine is not responding".to_owned());
    }

    fn push_pattern(&mut self) {
        let pattern = Box::new(self.pattern());
        self.send(Command::SetPattern(pattern));
    }

    pub fn toggle(&mut self) {
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
        self.subdivision = subdivision;
        self.push_pattern();
    }

    pub fn bell(&self) -> bool {
        self.bell
    }

    pub fn set_bell(&mut self, bell: bool) {
        self.bell = bell;
        self.push_pattern();
    }

    /// Meter cell that is sounding now, while playing.
    pub fn current_cell(&self) -> Option<usize> {
        if !self.playing {
            return None;
        }
        let beat = self.audio.as_ref()?.status().last_beat()?;
        Some(usize::from(beat.cell))
    }

    pub fn device_info(&self) -> Option<String> {
        self.audio.as_ref().map(|a| {
            format!(
                "{} · {} Hz · {} ch",
                a.device_name(),
                a.sample_rate(),
                a.channels()
            )
        })
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}
