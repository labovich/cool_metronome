use crate::pattern::ClickKind;
use std::f64::consts::TAU;

/// The four click sounds as mono samples at the engine's sample rate.
///
/// Built off the audio thread (synthesized or decoded and resampled) and
/// handed to the engine whole.
#[derive(Debug, Clone, PartialEq)]
pub struct SoundBank {
    sounds: [Vec<f32>; ClickKind::SOUNDING],
}

/// A decaying sum of sine partials: `(frequency ratio, amplitude)`.
struct Voice {
    base_hz: f64,
    partials: &'static [(f64, f64)],
    decay_s: f64,
    length_s: f64,
    noise: f64,
}

const ACCENT: Voice = Voice {
    base_hz: 1900.0,
    partials: &[(1.0, 1.0), (1.51, 0.45), (2.76, 0.2)],
    decay_s: 0.030,
    length_s: 0.15,
    noise: 0.35,
};
const NORMAL: Voice = Voice {
    base_hz: 1250.0,
    partials: &[(1.0, 1.0), (1.51, 0.35), (2.76, 0.15)],
    decay_s: 0.028,
    length_s: 0.15,
    noise: 0.25,
};
const SUB: Voice = Voice {
    base_hz: 2600.0,
    partials: &[(1.0, 1.0), (1.8, 0.3)],
    decay_s: 0.012,
    length_s: 0.06,
    noise: 0.2,
};
const BELL: Voice = Voice {
    base_hz: 1046.5,
    partials: &[(1.0, 1.0), (2.76, 0.45), (5.40, 0.22), (8.93, 0.1)],
    decay_s: 0.35,
    length_s: 1.2,
    noise: 0.0,
};

impl SoundBank {
    /// The built-in sounds, synthesized for `sample_rate`.
    #[must_use]
    pub fn synthesized(sample_rate: u32) -> Self {
        Self {
            sounds: [
                synth(&ACCENT, sample_rate, 0.95),
                synth(&NORMAL, sample_rate, 0.9),
                synth(&SUB, sample_rate, 0.8),
                synth(&BELL, sample_rate, 0.8),
            ],
        }
    }

    /// One-sample impulses with distinct heights (1/2, 1/4, 1/8, 1/16):
    /// lets tests read exact click positions and kinds from rendered audio.
    #[must_use]
    pub fn impulses() -> Self {
        Self {
            sounds: [vec![0.5], vec![0.25], vec![0.125], vec![0.0625]],
        }
    }

    /// Builds a bank from four sounds (accent, normal, sub, bell).
    #[must_use]
    pub fn from_sounds(sounds: [Vec<f32>; ClickKind::SOUNDING]) -> Self {
        Self { sounds }
    }

    /// Samples for a sound slot (see [`ClickKind::sound_index`]).
    #[must_use]
    pub fn sound(&self, index: usize) -> &[f32] {
        self.sounds.get(index).map_or(&[], Vec::as_slice)
    }
}

fn synth(v: &Voice, sample_rate: u32, peak: f32) -> Vec<f32> {
    let rate = f64::from(sample_rate);
    // Length is a fraction of a second at most a few hundred thousand samples.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let len = (v.length_s * rate).round() as usize;
    let attack = 0.0003 * rate; // 0.3 ms: sharp but click-free
    let mut noise_state: u32 = 0x1234_5678;
    let mut out: Vec<f64> = (0..len)
        .map(|i| {
            #[allow(clippy::cast_precision_loss)]
            let n = i as f64;
            let t = n / rate;
            let env = (n / attack).min(1.0) * (-t / v.decay_s).exp();
            let tone: f64 = v
                .partials
                .iter()
                .map(|(ratio, amp)| amp * (TAU * v.base_hz * ratio * t).sin())
                .sum();
            // Short noise burst gives the wooden "tick" attack.
            noise_state ^= noise_state << 13;
            noise_state ^= noise_state >> 17;
            noise_state ^= noise_state << 5;
            let white = f64::from(noise_state) / f64::from(u32::MAX) * 2.0 - 1.0;
            let burst = v.noise * white * (-t / 0.002).exp();
            env * tone + (n / attack).min(1.0) * burst
        })
        .collect();
    // Fade the tail to zero over the last 5 ms so a cut sound never clicks.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let fade = ((0.005 * rate) as usize).min(len);
    for (k, s) in out[len - fade..].iter_mut().enumerate() {
        #[allow(clippy::cast_precision_loss)]
        let g = 1.0 - (k as f64 + 1.0) / fade as f64;
        *s *= g;
    }
    let max = out.iter().fold(0.0_f64, |m, s| m.max(s.abs()));
    let scale = if max > 0.0 {
        f64::from(peak) / max
    } else {
        0.0
    };
    #[allow(clippy::cast_possible_truncation)]
    out.drain(..).map(|s| (s * scale) as f32).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthesized_sounds_are_sane() {
        for rate in [44_100, 48_000, 96_000] {
            let bank = SoundBank::synthesized(rate);
            for i in 0..ClickKind::SOUNDING {
                let s = bank.sound(i);
                assert!(!s.is_empty());
                let peak = s.iter().fold(0.0_f32, |m, x| m.max(x.abs()));
                assert!(peak > 0.5 && peak <= 0.951, "peak {peak}");
                assert!(s.last().unwrap().abs() < 1e-3, "tail must fade out");
                // Onset within the first millisecond.
                let onset = s.iter().position(|x| x.abs() > 0.05).unwrap();
                assert!(onset < rate as usize / 1000);
            }
        }
    }

    #[test]
    fn missing_slot_is_empty() {
        assert!(SoundBank::impulses().sound(9).is_empty());
    }
}
