//! Timing-accuracy tests for the click scheduler and the renderer.
//!
//! Requirement: every click starts within one sample of its ideal time,
//! with no drift however long the metronome runs, for any tempo, meter,
//! subdivision, sample rate, block size and tempo changes during playback.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::float_cmp
)]

use cm_core::{BeatLevel, Meter, Subdivision, Tempo};
use cm_engine::{BarPattern, ClickKind, Command, Engine, EngineStatus, Scheduler, SoundBank};
use std::sync::Arc;

/// Allowed distance from the ideal position, in samples.
const TOLERANCE: f64 = 1.0;

fn meters() -> Vec<(&'static str, Meter, Subdivision)> {
    use BeatLevel::{Accent as A, Mute as M, Normal as N};
    // Son clave 3-2 in sixteenths: 4 subunits per beat, 16 cells.
    let mut clave = vec![M; 16];
    for i in [0, 3, 6, 10, 12] {
        clave[i] = A;
    }
    vec![
        ("4/4", Meter::default(), Subdivision::None),
        (
            "3+2 triplets",
            Meter::from_formula("3+2").unwrap(),
            Subdivision::Triple,
        ),
        (
            "7/8 cut, eighths",
            Meter::new(2, vec![A, N, N, N, A, N, N]).unwrap(),
            Subdivision::Duple,
        ),
        (
            "son clave, septuplets",
            Meter::new(4, clave).unwrap(),
            Subdivision::Septuple,
        ),
        ("6 shuffle", Meter::simple(6).unwrap(), Subdivision::Shuffle),
    ]
}

/// Runs the scheduler for `seconds` and checks every event against the ideal
/// position computed independently from the bar layout. Returns the worst error.
fn check_schedule(meter: &Meter, sub: Subdivision, bpm: f64, rate: u32, seconds: f64) -> f64 {
    let pattern = BarPattern::new(meter, sub, true);
    let positions: Vec<f64> = pattern.events().iter().map(|e| e.beat).collect();
    let bar_len = pattern.length();
    let mut s = Scheduler::new(Box::new(pattern), Tempo::from_bpm(bpm), rate);
    let start = 12_345;
    let _ = s.start(start);

    let tempo = Tempo::from_bpm(bpm).bpm();
    let samples_per_beat_ideal = 60.0 * f64::from(rate) / tempo;
    let end = start + (seconds * f64::from(rate)) as u64;
    let mut worst: f64 = 0.0;
    let mut last = 0u64;
    let mut n = 0u64;
    loop {
        let ev = s.peek().expect("a running pattern always has events");
        if ev.sample > end {
            break;
        }
        let bar = n / positions.len() as u64;
        let j = (n % positions.len() as u64) as usize;
        let ideal_beats = bar as f64 * bar_len + positions[j];
        let ideal = start as f64 + ideal_beats * samples_per_beat_ideal;
        let err = (ev.sample as f64 - ideal).abs();
        worst = worst.max(err);
        assert!(
            ev.sample >= last,
            "events out of order at #{n}: {} after {last}",
            ev.sample
        );
        last = ev.sample;
        let _ = s.advance();
        n += 1;
    }
    assert!(n > 0);
    worst
}

#[test]
fn one_hour_without_drift_at_48k() {
    for (name, meter, sub) in meters() {
        for bpm in [40.0, 97.3, 133.7, 208.0] {
            let worst = check_schedule(&meter, sub, bpm, 48_000, 3600.0);
            assert!(
                worst <= TOLERANCE,
                "{name} @ {bpm} BPM: worst error {worst:.3} samples"
            );
        }
    }
}

#[test]
fn all_sample_rates_and_extreme_tempos() {
    for rate in [44_100, 48_000, 88_200, 96_000, 192_000] {
        for (name, meter, sub) in meters() {
            for bpm in [1.0, 40.0, 60.5, 120.0, 333.3, 999.0] {
                let worst = check_schedule(&meter, sub, bpm, rate, 300.0);
                assert!(
                    worst <= TOLERANCE,
                    "{name} @ {bpm} BPM, {rate} Hz: worst error {worst:.3} samples"
                );
            }
        }
    }
}

// ---------------------------------------------------------------- rendering

/// Small deterministic PRNG (xorshift64*), so the tests need no dependencies.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn range(&mut self, lo: u64, hi: u64) -> u64 {
        lo + self.next() % (hi - lo + 1)
    }
}

fn impulse_engine(meter: &Meter, sub: Subdivision, bpm: f64, rate: u32) -> Engine {
    let pattern = BarPattern::new(meter, sub, true);
    let mut e = Engine::new(
        rate,
        Tempo::from_bpm(bpm),
        Box::new(pattern),
        Box::new(SoundBank::impulses()),
        Arc::new(EngineStatus::default()),
    );
    for kind in [
        ClickKind::Accent,
        ClickKind::Normal,
        ClickKind::Sub,
        ClickKind::Bell,
    ] {
        e.handle(Command::SetGain(kind, 1.0));
    }
    e
}

/// Renders `total` mono samples in blocks whose sizes come from `block`.
fn render(e: &mut Engine, total: usize, mut block: impl FnMut() -> usize) -> Vec<f32> {
    let mut out = vec![0.0; total];
    let mut pos = 0;
    while pos < total {
        let n = block().min(total - pos);
        e.render(&mut out[pos..pos + n], 1);
        pos += n;
    }
    out
}

fn impulse_height(kind: ClickKind) -> f32 {
    match kind {
        ClickKind::Accent => 0.5,
        ClickKind::Normal => 0.25,
        ClickKind::Sub => 0.125,
        ClickKind::Bell => 0.0625,
        ClickKind::Silent => 0.0,
    }
}

#[test]
fn rendered_clicks_land_exactly_where_scheduled_for_any_block_size() {
    let rate = 48_000;
    let total = 20 * rate as usize;
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for (name, meter, sub) in meters() {
        for bpm in [61.7, 144.0, 512.3] {
            // Expected output straight from the scheduler.
            let pattern = BarPattern::new(&meter, sub, true);
            let mut s = Scheduler::new(Box::new(pattern), Tempo::from_bpm(bpm), rate);
            let _ = s.start(0);
            let mut expected = vec![0.0_f32; total];
            while let Some(ev) = s.peek() {
                if ev.sample >= total as u64 {
                    break;
                }
                expected[ev.sample as usize] += impulse_height(ev.kind);
                let _ = s.advance();
            }

            let mut e = impulse_engine(&meter, sub, bpm, rate);
            e.handle(Command::Start);
            let random_blocks = render(&mut e, total, || rng.range(1, 4096) as usize);
            assert!(
                random_blocks == expected,
                "{name} @ {bpm}: random-block render differs from the schedule"
            );

            let mut e = impulse_engine(&meter, sub, bpm, rate);
            e.handle(Command::Start);
            let fixed_blocks = render(&mut e, total, || 512);
            assert!(
                fixed_blocks == expected,
                "{name} @ {bpm}: 512-block render differs"
            );
        }
    }
}

#[test]
fn tempo_changes_during_playback_keep_the_grid() {
    let rate = 48_000_u32;
    let mut rng = Rng(42);
    for round in 0..20 {
        let mut e = impulse_engine(&Meter::default(), Subdivision::None, 120.0, rate);
        e.handle(Command::Start);
        // (clock at which the command was sent, samples per beat after it)
        let mut changes: Vec<(u64, f64)> = Vec::new();
        let spb = |t: Tempo| t.beat_seconds() * f64::from(rate);
        let total = 60 * rate as usize;
        let mut out = vec![0.0_f32; total];
        let mut pos = 0;
        while pos < total {
            if rng.range(0, 9) == 0 {
                let tempo = Tempo::from_bpm(rng.range(400, 9990) as f64 / 10.0);
                e.handle(Command::SetTempo(tempo));
                changes.push((pos as u64, spb(tempo)));
            }
            let n = (rng.range(1, 3000) as usize).min(total - pos);
            e.render(&mut out[pos..pos + n], 1);
            pos += n;
        }

        let onsets: Vec<u64> = out
            .iter()
            .enumerate()
            .filter(|(_, s)| **s != 0.0)
            .map(|(i, _)| i as u64)
            .collect();
        assert!(onsets.len() > 10, "round {round}: too few clicks");

        // The interval leaving onset i-1 follows the last tempo command sent at
        // or before onset i-1 (the pending click keeps its place, the new tempo
        // starts from it). Positions are checked against the pivot click, so
        // any accumulated drift would show up.
        let period_at = |sample: u64| {
            changes
                .iter()
                .rev()
                .find(|(c, _)| *c <= sample)
                .map_or(spb(Tempo::from_bpm(120.0)), |(_, p)| *p)
        };
        let mut pivot = 0usize;
        let mut pivot_period = period_at(onsets[0]);
        for i in 1..onsets.len() {
            let period = period_at(onsets[i - 1]);
            if (period - pivot_period).abs() > 1e-9 {
                pivot = i - 1;
                pivot_period = period;
            }
            let expected = onsets[pivot] as f64 + (i - pivot) as f64 * period;
            let err = (onsets[i] as f64 - expected).abs();
            assert!(
                err <= TOLERANCE,
                "round {round}: click {i} at {} is {err:.2} samples off (expected {expected:.1})",
                onsets[i]
            );
        }
    }
}

#[test]
fn stop_and_restart_starts_a_fresh_bar() {
    let rate = 48_000;
    let mut e = impulse_engine(&Meter::default(), Subdivision::None, 120.0, rate);
    e.handle(Command::Start);
    let _ = render(&mut e, 30_000, || 1000);
    e.handle(Command::Stop);
    let silent = render(&mut e, 50_000, || 700);
    assert!(silent.iter().all(|s| *s == 0.0));
    e.handle(Command::Start);
    let again = render(&mut e, 30_000, || 333);
    assert_eq!(again[0], 0.5, "restart begins with the accented downbeat");
    assert_eq!(again[24_000], 0.25);
}
