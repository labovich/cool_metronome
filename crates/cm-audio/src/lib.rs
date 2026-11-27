//! Audio output for Cool Metronome.
//!
//! Opens an output device with `cpal` and runs [`cm_engine::Engine`] inside
//! the device callback. The UI talks to the engine through a lock-free queue
//! ([`AudioOutput::send`]); objects the engine replaces come back through a
//! second queue and are dropped on the UI thread ([`AudioOutput::collect_garbage`]),
//! so the audio callback never allocates, frees, locks or blocks. A [`Waker`]
//! tells the owner when the device fails, so it can recover even while idle.

use cm_core::Tempo;
use cm_engine::{BarPattern, Command, Engine, EngineStatus, Retired, SoundBank};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Capacity of the UI → engine command queue.
const COMMAND_QUEUE: usize = 256;
/// Capacity of the engine → UI queue of replaced objects.
const RETIRED_QUEUE: usize = 64;
/// The engine renders at most this many frames at a time.
const MAX_BLOCK_FRAMES: usize = 4096;

/// Why the audio output could not be opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioError {
    /// The system has no default output device.
    NoDevice,
    /// No output device with this name.
    DeviceNotFound(String),
    /// The device did not report a usable configuration.
    Config(String),
    /// The device uses a sample format the app does not support.
    UnsupportedFormat(String),
    /// The stream could not be created or started.
    Stream(String),
}

impl fmt::Display for AudioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoDevice => write!(f, "no audio output device found"),
            Self::DeviceNotFound(name) => write!(f, "audio device \"{name}\" not found"),
            Self::Config(e) => write!(f, "cannot read the audio device configuration: {e}"),
            Self::UnsupportedFormat(format) => {
                write!(f, "unsupported audio sample format {format}")
            }
            Self::Stream(e) => write!(f, "cannot start audio output: {e}"),
        }
    }
}

impl std::error::Error for AudioError {}

/// Wakes the owner when the output fails, so it can recover even while idle
/// (no UI frames are running).
///
/// Called from the stream's error callback, never from the audio data callback,
/// so it may lock or allocate (e.g. `egui::Context::request_repaint`).
#[derive(Clone)]
pub struct Waker(Arc<dyn Fn() + Send + Sync>);

impl Waker {
    /// Wraps the function to call.
    pub fn new(wake: impl Fn() + Send + Sync + 'static) -> Self {
        Self(Arc::new(wake))
    }

    fn wake(&self) {
        (self.0)();
    }
}

impl fmt::Debug for Waker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Waker")
    }
}

/// What the engine starts with.
#[derive(Debug, Clone)]
pub struct EngineSetup {
    /// Initial tempo.
    pub tempo: Tempo,
    /// Initial bar pattern.
    pub pattern: BarPattern,
    /// Click sounds; `None` uses the built-in synthesized ones.
    pub sounds: Option<SoundBank>,
}

/// A running output stream with the metronome engine inside.
///
/// Dropping it closes the stream.
pub struct AudioOutput {
    _stream: cpal::Stream,
    commands: rtrb::Producer<Command>,
    retired: rtrb::Consumer<Retired>,
    status: Arc<EngineStatus>,
    failed: Arc<AtomicBool>,
    sample_rate: u32,
    channels: u16,
    device_name: String,
}

impl fmt::Debug for AudioOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AudioOutput")
            .field("device_name", &self.device_name)
            .field("sample_rate", &self.sample_rate)
            .field("channels", &self.channels)
            .finish_non_exhaustive()
    }
}

/// Names of the available output devices, for a device picker.
#[must_use]
pub fn output_device_names() -> Vec<String> {
    cpal::default_host()
        .output_devices()
        .map(|devices| devices.map(|d| d.to_string()).collect())
        .unwrap_or_default()
}

impl AudioOutput {
    /// Opens `device` (or the system default output) and starts the stream.
    /// The engine is stopped until [`Command::Start`] is sent. `waker` is
    /// called once when the device fails (see [`AudioOutput::has_failed`]).
    ///
    /// # Errors
    /// If the device is missing, its format is unsupported or the stream fails.
    pub fn open(
        device: Option<&str>,
        setup: EngineSetup,
        waker: Option<Waker>,
    ) -> Result<Self, AudioError> {
        let host = cpal::default_host();
        let device = match device {
            Some(name) => host
                .output_devices()
                .map_err(|e| AudioError::Config(e.to_string()))?
                .find(|d| d.to_string() == name)
                .ok_or_else(|| AudioError::DeviceNotFound(name.to_owned()))?,
            None => host.default_output_device().ok_or(AudioError::NoDevice)?,
        };
        let device_name = device.to_string();

        let supported = device
            .default_output_config()
            .map_err(|e| AudioError::Config(e.to_string()))?;
        let format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();
        let sample_rate: u32 = config.sample_rate;
        let channels: u16 = config.channels;

        let status = Arc::new(EngineStatus::default());
        let sounds = setup
            .sounds
            .unwrap_or_else(|| SoundBank::synthesized(sample_rate));
        let engine = Engine::new(
            sample_rate,
            setup.tempo,
            Box::new(setup.pattern),
            Box::new(sounds),
            Arc::clone(&status),
        );
        let (commands, command_rx) = rtrb::RingBuffer::new(COMMAND_QUEUE);
        let (retired_tx, retired) = rtrb::RingBuffer::new(RETIRED_QUEUE);
        let failed = Arc::new(AtomicBool::new(false));
        let parts = CallbackParts {
            engine,
            commands: command_rx,
            retired: retired_tx,
            failed: Arc::clone(&failed),
            waker,
        };

        let stream = match format {
            SampleFormat::F32 => build::<f32>(&device, config, parts),
            SampleFormat::I16 => build::<i16>(&device, config, parts),
            SampleFormat::U16 => build::<u16>(&device, config, parts),
            SampleFormat::I32 => build::<i32>(&device, config, parts),
            other => Err(AudioError::UnsupportedFormat(format!("{other:?}"))),
        }?;
        stream
            .play()
            .map_err(|e| AudioError::Stream(e.to_string()))?;

        Ok(Self {
            _stream: stream,
            commands,
            retired,
            status,
            failed,
            sample_rate,
            channels,
            device_name,
        })
    }

    /// Sends a command to the engine.
    ///
    /// # Errors
    /// Gives the command back when the queue is full (the engine is not running).
    pub fn send(&mut self, command: Command) -> Result<(), Command> {
        if self.commands.is_full() {
            return Err(command);
        }
        // Single producer: a slot is free, so the push succeeds.
        let _ = self.commands.push(command);
        Ok(())
    }

    /// Drops the objects the engine replaced. Call regularly from the UI thread.
    pub fn collect_garbage(&mut self) {
        while let Ok(retired) = self.retired.pop() {
            drop(retired);
        }
    }

    /// Playback state shared with the engine.
    #[must_use]
    pub fn status(&self) -> &Arc<EngineStatus> {
        &self.status
    }

    /// True after the device reported an error (e.g. it was unplugged).
    /// The owner should drop this output and open a new one.
    #[must_use]
    pub fn has_failed(&self) -> bool {
        self.failed.load(Ordering::Relaxed)
    }

    /// Device sample rate in Hz.
    #[must_use]
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Number of output channels.
    #[must_use]
    pub fn channels(&self) -> u16 {
        self.channels
    }

    /// Name of the output device.
    #[must_use]
    pub fn device_name(&self) -> &str {
        &self.device_name
    }
}

/// Everything the audio callback owns.
struct CallbackParts {
    engine: Engine,
    commands: rtrb::Consumer<Command>,
    retired: rtrb::Producer<Retired>,
    failed: Arc<AtomicBool>,
    waker: Option<Waker>,
}

fn build<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    parts: CallbackParts,
) -> Result<cpal::Stream, AudioError>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = usize::from(config.channels).max(1);
    let CallbackParts {
        mut engine,
        mut commands,
        mut retired,
        failed,
        waker,
    } = parts;
    // Allocated once here, never in the callback.
    let mut scratch = vec![0.0_f32; MAX_BLOCK_FRAMES * channels];

    let data_callback = move |data: &mut [T], _info: &cpal::OutputCallbackInfo| {
        while let Ok(command) = commands.pop() {
            engine.handle(command);
        }
        for chunk in data.chunks_mut(MAX_BLOCK_FRAMES * channels) {
            let buf = &mut scratch[..chunk.len()];
            engine.render(buf, channels);
            for (out, sample) in chunk.iter_mut().zip(buf.iter()) {
                *out = T::from_sample(*sample);
            }
        }
        engine.drain_retired(|r| {
            // Full queue: the UI is not collecting; dropping here is a rare fallback.
            let _ = retired.push(r);
        });
    };
    // Not the real-time path: the stream is already broken when this runs.
    let error_callback = move |_err| {
        let first = !failed.swap(true, Ordering::Relaxed);
        if first && let Some(waker) = &waker {
            waker.wake();
        }
    };

    device
        .build_output_stream(config, data_callback, error_callback, None)
        .map_err(|e| AudioError::Stream(e.to_string()))
}
