use crate::pattern::{BarPattern, ClickKind};
use crate::scheduler::Scheduler;
use crate::sounds::SoundBank;
use cm_core::Tempo;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

/// Most clicks that can sound at once. The oldest is cut when more start.
pub const MAX_VOICES: usize = 32;
/// Retired objects kept until the owner collects them.
const RETIRED_SLOTS: usize = 8;

/// A request from the UI thread to the engine.
#[derive(Debug)]
pub enum Command {
    /// Start playing from the next rendered sample.
    Start,
    /// Stop scheduling new clicks (sounding ones ring out).
    Stop,
    /// Change the tempo; takes effect at the next click.
    SetTempo(Tempo),
    /// Replace the bar pattern; takes effect at the next bar.
    SetPattern(Box<BarPattern>),
    /// Replace the click sounds.
    SetSounds(Box<SoundBank>),
    /// Linear gain of one sound slot (see [`ClickKind::sound_index`]).
    SetGain(ClickKind, f32),
    /// Master linear gain.
    SetMaster(f32),
    /// Stereo position, -1 (left) … 1 (right).
    SetPan(f32),
}

/// Something the engine no longer needs. Must be dropped off the audio thread.
#[derive(Debug)]
pub enum Retired {
    /// A replaced bar pattern.
    Pattern(Box<BarPattern>),
    /// A replaced sound bank.
    Sounds(Box<SoundBank>),
}

/// Lock-free state the UI reads to follow playback.
#[derive(Debug, Default)]
pub struct EngineStatus {
    playing: AtomicBool,
    clock: AtomicU64,
    beat_sample: AtomicU64,
    beat_cell: AtomicU32,
    beat_bar: AtomicU64,
    beats_seen: AtomicU64,
}

/// The last meter cell that started, as seen by the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BeatInfo {
    /// Sample at which it started.
    pub sample: u64,
    /// Meter cell index.
    pub cell: u16,
    /// Bar number since start.
    pub bar: u64,
    /// Increments on every new cell; lets the UI notice changes cheaply.
    pub serial: u64,
}

impl EngineStatus {
    /// True while playing.
    #[must_use]
    pub fn is_playing(&self) -> bool {
        self.playing.load(Ordering::Relaxed)
    }

    /// Samples rendered since the engine was created.
    #[must_use]
    pub fn clock(&self) -> u64 {
        self.clock.load(Ordering::Relaxed)
    }

    /// The last cell that started, or `None` before the first one.
    #[must_use]
    pub fn last_beat(&self) -> Option<BeatInfo> {
        let serial = self.beats_seen.load(Ordering::Acquire);
        if serial == 0 {
            return None;
        }
        Some(BeatInfo {
            sample: self.beat_sample.load(Ordering::Relaxed),
            cell: u16::try_from(self.beat_cell.load(Ordering::Relaxed)).unwrap_or(0),
            bar: self.beat_bar.load(Ordering::Relaxed),
            serial,
        })
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct Voice {
    active: bool,
    slot: usize,
    /// Next sample of the sound to play.
    pos: usize,
    /// Frames to wait in the current block before starting.
    delay: usize,
    /// Start order, for stealing the oldest voice.
    age: u64,
}

/// Renders the click track. Lives on the audio thread; `render` and `handle`
/// never allocate, lock or block.
#[derive(Debug)]
pub struct Engine {
    scheduler: Scheduler,
    sounds: Box<SoundBank>,
    voices: [Voice; MAX_VOICES],
    voice_counter: u64,
    gains: [f32; ClickKind::SOUNDING],
    master: f32,
    pan: [f32; 2],
    clock: u64,
    status: Arc<EngineStatus>,
    retired: [Option<Retired>; RETIRED_SLOTS],
}

impl Engine {
    /// Default linear gains: accent 0 dB, normal −5 dB, sub −15 dB, bell −9 dB.
    pub const DEFAULT_GAINS: [f32; ClickKind::SOUNDING] = [1.0, 0.562, 0.178, 0.355];

    /// Creates a stopped engine.
    #[must_use]
    pub fn new(
        sample_rate: u32,
        tempo: Tempo,
        pattern: Box<BarPattern>,
        sounds: Box<SoundBank>,
        status: Arc<EngineStatus>,
    ) -> Self {
        Self {
            scheduler: Scheduler::new(pattern, tempo, sample_rate),
            sounds,
            voices: [Voice::default(); MAX_VOICES],
            voice_counter: 0,
            gains: Self::DEFAULT_GAINS,
            master: 1.0,
            pan: pan_gains(0.0),
            clock: 0,
            status,
            retired: Default::default(),
        }
    }

    /// Applies a command.
    pub fn handle(&mut self, command: Command) {
        match command {
            Command::Start => {
                let r = self.scheduler.start(self.clock);
                self.retire_pattern(r);
                self.status.playing.store(true, Ordering::Relaxed);
            }
            Command::Stop => {
                let r = self.scheduler.stop();
                self.retire_pattern(r);
                self.status.playing.store(false, Ordering::Relaxed);
            }
            Command::SetTempo(t) => self.scheduler.set_tempo(t, self.clock),
            Command::SetPattern(p) => {
                let r = self.scheduler.set_pattern(p);
                self.retire_pattern(r);
            }
            Command::SetSounds(s) => {
                let old = std::mem::replace(&mut self.sounds, s);
                self.retire(Retired::Sounds(old));
            }
            Command::SetGain(kind, g) => {
                if let Some(i) = kind.sound_index() {
                    self.gains[i] = sanitize_gain(g);
                }
            }
            Command::SetMaster(g) => self.master = sanitize_gain(g),
            Command::SetPan(p) => self.pan = pan_gains(p),
        }
    }

    /// Hands every retired object to `f` (to be dropped off the audio thread).
    pub fn drain_retired(&mut self, mut f: impl FnMut(Retired)) {
        for slot in &mut self.retired {
            if let Some(r) = slot.take() {
                f(r);
            }
        }
    }

    /// Samples rendered so far.
    #[must_use]
    pub fn clock(&self) -> u64 {
        self.clock
    }

    /// Fills `out` (interleaved, `channels` per frame) with the next block.
    ///
    /// Channel 0 is left, 1 is right; further channels stay silent. Mono
    /// output gets the sum at unity gain.
    pub fn render(&mut self, out: &mut [f32], channels: usize) {
        let channels = channels.max(1);
        let frames = out.len() / channels;
        out.fill(0.0);
        let block_end = self.clock + frames as u64;

        // 1. Start the clicks that fall into this block.
        while let Some(ev) = self.scheduler.peek() {
            if ev.sample >= block_end {
                break;
            }
            // An event can only be in the past right after a tempo change
            // into a much faster tempo; play it at once rather than drop it.
            let offset = usize::try_from(ev.sample.saturating_sub(self.clock)).unwrap_or(0);
            if let Some(slot) = ev.kind.sound_index() {
                self.start_voice(slot, offset);
            }
            if ev.kind.is_cell() {
                self.publish_beat(ev.sample.max(self.clock), ev.cell, ev.bar);
            }
            let r = self.scheduler.advance();
            self.retire_pattern(r);
        }

        // 2. Mix the sounding voices.
        let [pan_l, pan_r] = self.pan;
        for v in &mut self.voices {
            if !v.active {
                continue;
            }
            let sound = self.sounds.sound(v.slot);
            let gain = self.gains[v.slot] * self.master;
            let start = v.delay.min(frames);
            v.delay = 0;
            let available = sound.len().saturating_sub(v.pos);
            let n = available.min(frames - start);
            let src = &sound[v.pos..v.pos + n];
            if channels == 1 {
                for (o, s) in out[start..start + n].iter_mut().zip(src) {
                    *o += s * gain;
                }
            } else {
                for (frame, s) in out[start * channels..(start + n) * channels]
                    .chunks_exact_mut(channels)
                    .zip(src)
                {
                    let x = s * gain;
                    frame[0] += x * pan_l;
                    frame[1] += x * pan_r;
                }
            }
            v.pos += n;
            if v.pos >= sound.len() {
                v.active = false;
            }
        }

        for s in out.iter_mut() {
            *s = s.clamp(-1.0, 1.0);
        }
        self.clock = block_end;
        self.status.clock.store(self.clock, Ordering::Relaxed);
    }

    fn start_voice(&mut self, slot: usize, delay: usize) {
        self.voice_counter += 1;
        let v = Voice {
            active: true,
            slot,
            pos: 0,
            delay,
            age: self.voice_counter,
        };
        if let Some(free) = self.voices.iter_mut().find(|v| !v.active) {
            *free = v;
        } else if let Some(oldest) = self.voices.iter_mut().min_by_key(|v| v.age) {
            *oldest = v;
        }
    }

    fn publish_beat(&self, sample: u64, cell: u16, bar: u64) {
        self.status.beat_sample.store(sample, Ordering::Relaxed);
        self.status
            .beat_cell
            .store(u32::from(cell), Ordering::Relaxed);
        self.status.beat_bar.store(bar, Ordering::Relaxed);
        self.status.beats_seen.fetch_add(1, Ordering::Release);
    }

    fn retire_pattern(&mut self, p: Option<Box<BarPattern>>) {
        if let Some(p) = p {
            self.retire(Retired::Pattern(p));
        }
    }

    fn retire(&mut self, r: Retired) {
        if let Some(slot) = self.retired.iter_mut().find(|s| s.is_none()) {
            *slot = Some(r);
        }
        // All slots full: the owner is not collecting. Dropping here is the
        // lesser evil (one deallocation) compared to growing a list.
    }
}

fn sanitize_gain(g: f32) -> f32 {
    if g.is_finite() {
        g.clamp(0.0, 4.0)
    } else {
        0.0
    }
}

/// Constant-power pan law.
fn pan_gains(pan: f32) -> [f32; 2] {
    let p = if pan.is_finite() {
        pan.clamp(-1.0, 1.0)
    } else {
        0.0
    };
    let angle = (p + 1.0) * std::f32::consts::FRAC_PI_4;
    [angle.cos(), angle.sin()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use cm_core::{Meter, Subdivision};

    fn engine(bpm: f64, rate: u32) -> Engine {
        let pattern = BarPattern::new(&Meter::default(), Subdivision::None, false);
        let mut e = Engine::new(
            rate,
            Tempo::from_bpm(bpm),
            Box::new(pattern),
            Box::new(SoundBank::impulses()),
            Arc::new(EngineStatus::default()),
        );
        e.handle(Command::SetGain(ClickKind::Normal, 1.0));
        e
    }

    #[test]
    fn silent_until_started() {
        let mut e = engine(120.0, 48_000);
        let mut buf = vec![1.0; 256];
        e.render(&mut buf, 1);
        assert!(buf.iter().all(|s| *s == 0.0));
    }

    #[test]
    fn first_click_on_first_sample() {
        let mut e = engine(120.0, 48_000);
        let mut buf = vec![0.0; 256];
        e.render(&mut buf, 1);
        e.handle(Command::Start);
        e.render(&mut buf, 1);
        assert!((buf[0] - 0.5).abs() < 1e-6);
        assert!(buf[1..].iter().all(|s| *s == 0.0));
        assert_eq!(e.status.last_beat().unwrap().sample, 256);
    }

    #[test]
    fn stereo_center_pan() {
        let mut e = engine(120.0, 48_000);
        e.handle(Command::Start);
        let mut buf = vec![0.0; 64];
        e.render(&mut buf, 2);
        let g = 0.5 * std::f32::consts::FRAC_1_SQRT_2;
        assert!((buf[0] - g).abs() < 1e-6 && (buf[1] - g).abs() < 1e-6);
    }

    #[test]
    fn replaced_objects_are_retired_not_dropped() {
        let mut e = engine(120.0, 48_000);
        e.handle(Command::SetSounds(Box::new(SoundBank::impulses())));
        let p = BarPattern::new(&Meter::simple(3).unwrap(), Subdivision::None, false);
        e.handle(Command::SetPattern(Box::new(p)));
        let mut n = 0;
        e.drain_retired(|_| n += 1);
        assert_eq!(n, 2);
    }

    #[test]
    fn stop_lets_sound_ring_out() {
        let rate = 48_000;
        let pattern = BarPattern::new(&Meter::default(), Subdivision::None, false);
        let mut e = Engine::new(
            rate,
            Tempo::from_bpm(120.0),
            Box::new(pattern),
            Box::new(SoundBank::synthesized(rate)),
            Arc::new(EngineStatus::default()),
        );
        e.handle(Command::Start);
        let mut buf = vec![0.0; 128];
        e.render(&mut buf, 1);
        e.handle(Command::Stop);
        e.render(&mut buf, 1);
        assert!(buf.iter().any(|s| *s != 0.0), "tail keeps sounding");
        assert!(!e.status.is_playing());
    }
}
