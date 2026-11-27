//! Loading own click sounds: WAV (hound), FLAC (claxon), OGG Vorbis
//! (lewton) and MP3 (nanomp3), mixed down to mono and resampled to the
//! output rate. Runs on the UI thread; the result goes to the engine whole.

use crate::resample::resample;
use std::fmt;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

/// Longest sound kept, in seconds (clicks are short; this bounds memory).
pub const MAX_SECONDS: f64 = 4.0;
/// Fade at the end of a cut sound, so it never ends with a click.
const CUT_FADE_SECONDS: f64 = 0.005;

/// A decoded sound: mono samples at its own rate.
#[derive(Debug, Clone, PartialEq)]
pub struct Decoded {
    pub samples: Vec<f32>,
    pub rate: u32,
}

/// Why a sound could not be loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SampleError {
    /// The file could not be read.
    Io(String),
    /// Not one of WAV, FLAC, OGG, MP3 (by extension).
    UnknownFormat,
    /// The file is damaged or uses an unsupported variant.
    Decode(String),
    /// No audio in the file.
    Empty,
}

impl fmt::Display for SampleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "cannot read the file: {e}"),
            Self::UnknownFormat => write!(f, "use a WAV, FLAC, OGG or MP3 file"),
            Self::Decode(e) => write!(f, "cannot decode the file: {e}"),
            Self::Empty => write!(f, "the file has no audio"),
        }
    }
}

impl std::error::Error for SampleError {}

/// File extensions [`decode_file`] understands.
pub const EXTENSIONS: [&str; 4] = ["wav", "flac", "ogg", "mp3"];

/// Decodes a sound file to mono at its own rate, cut to [`MAX_SECONDS`].
///
/// # Errors
/// If the file is missing, of an unknown type, damaged or silent-empty.
pub fn decode_file(path: &Path) -> Result<Decoded, SampleError> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if !EXTENSIONS.contains(&ext.as_str()) {
        return Err(SampleError::UnknownFormat);
    }
    // A missing or unreadable file is a read error, whatever the decoder says.
    std::fs::File::open(path).map_err(io)?;
    let (interleaved, channels, rate) = match ext.as_str() {
        "wav" => wav(path)?,
        "flac" => flac(path)?,
        "ogg" => ogg(path)?,
        "mp3" => mp3(path)?,
        _ => return Err(SampleError::UnknownFormat),
    };
    if channels == 0 || rate == 0 || interleaved.is_empty() {
        return Err(SampleError::Empty);
    }
    let mut samples = mono(&interleaved, usize::from(channels));
    cut(&mut samples, rate);
    Ok(Decoded { samples, rate })
}

/// The sound at the output `rate`, ready for the engine.
#[must_use]
pub fn prepare(sound: &Decoded, rate: u32) -> Vec<f32> {
    resample(&sound.samples, sound.rate, rate)
}

type Pcm = (Vec<f32>, u16, u32);

fn io(e: impl fmt::Display) -> SampleError {
    SampleError::Io(e.to_string())
}

fn decode(e: impl fmt::Display) -> SampleError {
    SampleError::Decode(e.to_string())
}

/// Samples to read at most: [`MAX_SECONDS`] of `channels` at `rate`, plus
/// one frame so [`cut`] can tell that the sound was longer.
fn limit(rate: u32, channels: u16) -> usize {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let n = (MAX_SECONDS * f64::from(rate)).round() as usize + 1;
    n * usize::from(channels.max(1))
}

/// Integer PCM of `bits` → −1…1.
fn int_scale(bits: u32) -> f32 {
    let bits = bits.clamp(1, 32);
    #[allow(clippy::cast_precision_loss)]
    let full = (1_u64 << (bits - 1)) as f32;
    1.0 / full
}

fn wav(path: &Path) -> Result<Pcm, SampleError> {
    let mut r = hound::WavReader::open(path).map_err(decode)?;
    let spec = r.spec();
    let max = limit(spec.sample_rate, spec.channels);
    let data: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => r
            .samples::<f32>()
            .take(max)
            .collect::<Result<_, _>>()
            .map_err(decode)?,
        hound::SampleFormat::Int => {
            let k = int_scale(u32::from(spec.bits_per_sample));
            #[allow(clippy::cast_precision_loss)]
            let v = r
                .samples::<i32>()
                .take(max)
                .map(|s| s.map(|s| s as f32 * k))
                .collect::<Result<_, _>>()
                .map_err(decode)?;
            v
        }
    };
    Ok((data, spec.channels, spec.sample_rate))
}

fn flac(path: &Path) -> Result<Pcm, SampleError> {
    let mut r = claxon::FlacReader::open(path).map_err(decode)?;
    let info = r.streaminfo();
    let channels = u16::try_from(info.channels).unwrap_or(0);
    let max = limit(info.sample_rate, channels);
    let k = int_scale(info.bits_per_sample);
    #[allow(clippy::cast_precision_loss)]
    let data = r
        .samples()
        .take(max)
        .map(|s| s.map(|s| s as f32 * k))
        .collect::<Result<_, _>>()
        .map_err(decode)?;
    Ok((data, channels, info.sample_rate))
}

fn ogg(path: &Path) -> Result<Pcm, SampleError> {
    let file = File::open(path).map_err(io)?;
    let mut r = lewton::inside_ogg::OggStreamReader::new(BufReader::new(file)).map_err(decode)?;
    let channels = u16::from(r.ident_hdr.audio_channels);
    let rate = r.ident_hdr.audio_sample_rate;
    let max = limit(rate, channels);
    let k = int_scale(16);
    let mut data = Vec::new();
    while data.len() < max {
        match r.read_dec_packet_itl().map_err(decode)? {
            Some(packet) => data.extend(packet.into_iter().map(|s| f32::from(s) * k)),
            None => break,
        }
    }
    data.truncate(max);
    Ok((data, channels, rate))
}

fn mp3(path: &Path) -> Result<Pcm, SampleError> {
    let bytes = std::fs::read(path).map_err(io)?;
    let d = nanomp3::decode_all::<f32>(&bytes);
    let channels = d.channels.map_or(0, |c| u16::from(c.num()));
    let mut data = d.samples;
    data.truncate(limit(d.sample_rate, channels));
    Ok((data, channels, d.sample_rate))
}

/// Interleaved → mono by averaging the channels.
fn mono(interleaved: &[f32], channels: usize) -> Vec<f32> {
    if channels == 1 {
        return interleaved.to_vec();
    }
    #[allow(clippy::cast_precision_loss)]
    let k = 1.0 / channels as f32;
    interleaved
        .chunks_exact(channels)
        .map(|f| f.iter().sum::<f32>() * k)
        .collect()
}

/// Keeps at most [`MAX_SECONDS`]; a cut sound fades out over its last 5 ms.
fn cut(samples: &mut Vec<f32>, rate: u32) {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let max = (MAX_SECONDS * f64::from(rate)).round() as usize;
    if samples.len() <= max {
        return;
    }
    samples.truncate(max);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let fade = ((CUT_FADE_SECONDS * f64::from(rate)) as usize).clamp(1, max);
    let start = max - fade;
    for (i, s) in samples[start..].iter_mut().enumerate() {
        #[allow(clippy::cast_precision_loss)]
        let g = 1.0 - (i + 1) as f32 / fade as f32;
        *s *= g;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    struct Tmp(PathBuf);
    impl Tmp {
        fn new(name: &str) -> Self {
            Self(std::env::temp_dir().join(format!("cm-sample-{}-{name}", std::process::id())))
        }
    }
    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    fn write_wav(path: &Path, channels: u16, rate: u32, frames: &[[i16; 2]]) {
        let spec = hound::WavSpec {
            channels,
            sample_rate: rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(path, spec).unwrap();
        for f in frames {
            for s in &f[..usize::from(channels)] {
                w.write_sample(*s).unwrap();
            }
        }
        w.finalize().unwrap();
    }

    #[test]
    fn wav_stereo_to_mono() {
        let t = Tmp::new("stereo.wav");
        write_wav(
            &t.0,
            2,
            44_100,
            &[[16_384, 0], [-16_384, -16_384], [0, 32_767]],
        );
        let d = decode_file(&t.0).unwrap();
        assert_eq!(d.rate, 44_100);
        assert_eq!(d.samples.len(), 3);
        assert!((d.samples[0] - 0.25).abs() < 1e-4);
        assert!((d.samples[1] + 0.5).abs() < 1e-4);
        assert!((d.samples[2] - 0.5).abs() < 1e-3);
        // Prepared for 48 kHz: longer by the rate ratio.
        assert_eq!(prepare(&d, 48_000).len(), 4);
    }

    #[test]
    fn long_sounds_are_cut_and_faded() {
        let t = Tmp::new("long.WAV");
        let rate = 8_000;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let n = (6.0 * f64::from(rate)) as usize;
        write_wav(&t.0, 1, rate, &vec![[10_000, 0]; n]);
        let d = decode_file(&t.0).unwrap();
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let max = (MAX_SECONDS * f64::from(rate)) as usize;
        assert_eq!(d.samples.len(), max);
        assert!(d.samples[max - 1].abs() < 1e-3, "ends silent");
        assert!(d.samples[max / 2] > 0.29);
    }

    #[test]
    fn errors() {
        assert_eq!(
            decode_file(Path::new("click.aiff")),
            Err(SampleError::UnknownFormat)
        );
        assert!(matches!(
            decode_file(Path::new("does-not-exist.mp3")),
            Err(SampleError::Io(_))
        ));
        assert!(matches!(
            decode_file(Path::new("does-not-exist.wav")),
            Err(SampleError::Io(_))
        ));
        let t = Tmp::new("broken.wav");
        std::fs::write(&t.0, b"not a wav file").unwrap();
        assert!(matches!(decode_file(&t.0), Err(SampleError::Decode(_))));
        let t = Tmp::new("empty.wav");
        write_wav(&t.0, 1, 48_000, &[]);
        assert_eq!(decode_file(&t.0), Err(SampleError::Empty));
    }
}
