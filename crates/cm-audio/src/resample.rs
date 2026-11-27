//! Sample-rate conversion for click sounds: windowed-sinc interpolation.
//!
//! Sounds are short (a few seconds at most) and converted once, off the
//! audio thread, so a direct (non-FFT) filter is fast enough and keeps the
//! dependency list small.

use std::f64::consts::PI;

/// Filter half-length in input samples at full bandwidth.
const HALF_TAPS: f64 = 24.0;
/// Pass band as a fraction of the lower Nyquist frequency.
const BANDWIDTH: f64 = 0.92;

/// Converts mono `input` from `from` Hz to `to` Hz.
#[must_use]
pub fn resample(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || from == 0 || to == 0 || input.is_empty() {
        return input.to_vec();
    }
    let ratio = f64::from(to) / f64::from(from);
    // Cut-off relative to the input rate; lower when downsampling (anti-alias).
    let cutoff = BANDWIDTH * ratio.min(1.0);
    let support = HALF_TAPS / cutoff;
    #[allow(clippy::cast_precision_loss)]
    let in_len = input.len() as f64;
    // ceil(len × to / from) in integers: no float rounding surprises.
    let (n, from64, to64) = (input.len() as u64, u64::from(from), u64::from(to));
    let out_len = usize::try_from((n * to64).div_ceil(from64)).unwrap_or(usize::MAX);

    (0..out_len)
        .map(|j| {
            #[allow(clippy::cast_precision_loss)]
            let t = j as f64 / ratio;
            let first = (t - support).ceil().max(0.0);
            let last = (t + support).floor().min(in_len - 1.0);
            let mut acc = 0.0;
            let mut k = first;
            while k <= last {
                let x = t - k;
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let s = f64::from(input[k as usize]);
                acc += s * cutoff * sinc(cutoff * x) * window(x / support);
                k += 1.0;
            }
            #[allow(clippy::cast_possible_truncation)]
            let v = acc as f32;
            v
        })
        .collect()
}

fn sinc(x: f64) -> f64 {
    if x.abs() < 1e-12 {
        1.0
    } else {
        (PI * x).sin() / (PI * x)
    }
}

/// Blackman window on −1…1.
fn window(x: f64) -> f64 {
    if x.abs() >= 1.0 {
        return 0.0;
    }
    let a = PI * (x + 1.0);
    0.42 - 0.5 * a.cos() + 0.08 * (2.0 * a).cos()
}

#[cfg(test)]
#[allow(clippy::cast_precision_loss)]
mod tests {
    use super::*;

    fn sine(hz: f64, rate: u32, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| {
                #[allow(clippy::cast_possible_truncation)]
                let v = (2.0 * PI * hz * i as f64 / f64::from(rate)).sin() as f32;
                v
            })
            .collect()
    }

    /// Largest difference from the ideal sine, away from the edges.
    fn worst_error(out: &[f32], hz: f64, rate: u32) -> f32 {
        let ideal = sine(hz, rate, out.len());
        let edge = out.len() / 10;
        out[edge..out.len() - edge]
            .iter()
            .zip(&ideal[edge..])
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f32::max)
    }

    #[test]
    fn same_rate_is_unchanged() {
        let x = sine(1000.0, 48_000, 100);
        assert_eq!(resample(&x, 48_000, 48_000), x);
    }

    #[test]
    fn length_follows_the_ratio() {
        let x = vec![0.0; 44_100];
        assert_eq!(resample(&x, 44_100, 48_000).len(), 48_000);
        assert_eq!(resample(&x, 44_100, 22_050).len(), 22_050);
    }

    #[test]
    fn upsampling_keeps_a_sine() {
        let x = sine(1000.0, 44_100, 4410);
        let y = resample(&x, 44_100, 48_000);
        assert!(worst_error(&y, 1000.0, 48_000) < 2e-3);
    }

    #[test]
    fn downsampling_keeps_a_sine_and_removes_what_cannot_fit() {
        let low = sine(1000.0, 96_000, 9600);
        let y = resample(&low, 96_000, 44_100);
        assert!(worst_error(&y, 1000.0, 44_100) < 2e-3);
        // 30 kHz cannot exist at 44.1 kHz: it must not fold down as an alias.
        let high = sine(30_000.0, 96_000, 9600);
        let y = resample(&high, 96_000, 44_100);
        let edge = y.len() / 10;
        let peak = y[edge..y.len() - edge]
            .iter()
            .fold(0.0_f32, |m, s| m.max(s.abs()));
        assert!(peak < 0.01, "alias at {peak}");
    }
}
