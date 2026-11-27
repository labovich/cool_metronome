//! User settings: one TOML file in the user profile, with a schema version.
//!
//! - Windows: `%APPDATA%\CoolMetronome\settings.toml`
//! - macOS: `~/Library/Application Support/CoolMetronome/settings.toml`
//! - elsewhere: `$XDG_CONFIG_HOME/CoolMetronome` (or `~/.config/CoolMetronome`)
//!
//! Missing fields take their defaults and unknown ones are ignored, so files
//! from older and newer versions both load. A file that cannot be read is
//! kept as `settings.toml.bak` and the defaults are used. Writes go to a
//! temporary file that then replaces the old one, so a crash never leaves a
//! half-written file.

use cm_core::{Meter, Subdivision, Tempo};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Version of the file layout; bumped when a field changes meaning.
pub const SCHEMA_VERSION: u32 = 1;
const FILE_NAME: &str = "settings.toml";
/// Changes are written this long after the last one (and on exit).
const SAVE_DELAY: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub version: u32,
    pub metronome: MetronomeState,
    pub window: WindowState,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: SCHEMA_VERSION,
            metronome: MetronomeState::default(),
            window: WindowState::default(),
        }
    }
}

/// What is playing: tempo, bar, subdivision, master volume.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MetronomeState {
    /// BPM, 1â€“999, 0.1 steps.
    pub tempo: f64,
    /// Meter cells per beat.
    pub subunits: u8,
    /// Meter cells: `X` accent, `x` normal, `.` mute (see `Meter::from_grid`).
    pub grid: String,
    /// `Subdivision::id`.
    pub subdivision: String,
    /// Master volume slider position, 0 (mute) â€¦ 1.
    pub volume: f32,
}

impl Default for MetronomeState {
    fn default() -> Self {
        let meter = Meter::default();
        Self {
            tempo: Tempo::DEFAULT.bpm(),
            subunits: meter.subunits(),
            grid: meter.grid(),
            subdivision: Subdivision::None.id().to_owned(),
            volume: 1.0,
        }
    }
}

impl MetronomeState {
    pub fn new(tempo: Tempo, meter: &Meter, subdivision: Subdivision, volume: f32) -> Self {
        Self {
            tempo: tempo.bpm(),
            subunits: meter.subunits(),
            grid: meter.grid(),
            subdivision: subdivision.id().to_owned(),
            volume,
        }
    }

    pub fn tempo(&self) -> Tempo {
        Tempo::from_bpm(self.tempo)
    }

    /// The saved meter, or the default one if the grid is invalid.
    pub fn meter(&self) -> Meter {
        Meter::from_grid(self.subunits, &self.grid).unwrap_or_default()
    }

    pub fn subdivision(&self) -> Subdivision {
        Subdivision::from_id(&self.subdivision).unwrap_or_default()
    }

    pub fn volume(&self) -> f32 {
        if self.volume.is_finite() {
            self.volume.clamp(0.0, 1.0)
        } else {
            1.0
        }
    }
}

/// Window placement and mode.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowState {
    /// Full window size in points (also kept while compact).
    pub size: Option<[f32; 2]>,
    /// Top-left corner in points.
    pub position: Option<[f32; 2]>,
    pub compact: bool,
    /// Always on top.
    pub pinned: bool,
    /// UI zoom on top of the OS display scale.
    pub zoom: f32,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            size: None,
            position: None,
            compact: false,
            pinned: false,
            zoom: 1.0,
        }
    }
}

impl WindowState {
    /// The saved position if it looks like a real one (not the far-away
    /// spot Windows reports for minimized windows).
    pub fn position(&self) -> Option<[f32; 2]> {
        self.position
            .filter(|p| p.iter().all(|v| v.is_finite() && v.abs() < 20_000.0))
    }

    pub fn size(&self, min: [f32; 2]) -> Option<[f32; 2]> {
        self.size
            .filter(|s| s.iter().all(|v| v.is_finite() && *v < 20_000.0))
            .map(|[w, h]| [w.max(min[0]), h.max(min[1])])
    }
}

/// Folder of the settings file in the user profile.
pub fn settings_dir() -> Option<PathBuf> {
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
    };
    base.map(|b| b.join("CoolMetronome"))
}

/// Loads, keeps track of and saves the settings file.
#[derive(Debug)]
pub struct Store {
    path: Option<PathBuf>,
    /// What the file holds.
    saved: Settings,
    /// Changed settings and when they last changed.
    pending: Option<(Settings, Instant)>,
    /// Last write error, if any (the app keeps running).
    error: Option<String>,
}

impl Store {
    /// Opens the settings in the user profile.
    pub fn open() -> (Self, Settings) {
        Self::open_at(settings_dir().map(|d| d.join(FILE_NAME)))
    }

    /// Opens a settings file at `path` (`None`: nothing is read or written).
    pub fn open_at(path: Option<PathBuf>) -> (Self, Settings) {
        let settings = path.as_deref().map(load).unwrap_or_default();
        let store = Self {
            path,
            saved: settings.clone(),
            pending: None,
            error: None,
        };
        (store, settings)
    }

    /// Reports the current settings; they are written [`SAVE_DELAY`] after
    /// the last change.
    pub fn update(&mut self, current: Settings, now: Instant) {
        match &self.pending {
            Some((p, _)) if *p == current => {}
            _ if current == self.saved => self.pending = None,
            _ => self.pending = Some((current, now)),
        }
    }

    /// Writes the pending change if it is old enough.
    pub fn save_if_due(&mut self, now: Instant) {
        if self
            .pending
            .as_ref()
            .is_some_and(|(_, t)| now.duration_since(*t) >= SAVE_DELAY)
        {
            self.flush();
        }
    }

    /// When [`Store::save_if_due`] should run next, if anything is pending.
    pub fn next_due(&self, now: Instant) -> Option<Duration> {
        self.pending
            .as_ref()
            .map(|(_, t)| SAVE_DELAY.saturating_sub(now.duration_since(*t)))
    }

    /// Writes any pending change now (on exit).
    pub fn flush(&mut self) {
        let Some((settings, _)) = self.pending.take() else {
            return;
        };
        if let Some(path) = &self.path {
            match save(path, &settings) {
                Ok(()) => self.error = None,
                Err(e) => self.error = Some(e.to_string()),
            }
        }
        self.saved = settings;
    }

    /// The last write error, if any.
    #[allow(dead_code)] // Shown in the options window (stage 5b).
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

/// Reads the file; defaults if it is missing, defaults and a `.bak` copy
/// if it is unreadable.
fn load(path: &Path) -> Settings {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Settings::default(),
        Err(_) => {
            backup(path);
            return Settings::default();
        }
    };
    match toml::from_str::<Settings>(&text) {
        Ok(mut s) => {
            s.version = SCHEMA_VERSION;
            s
        }
        Err(_) => {
            backup(path);
            Settings::default()
        }
    }
}

fn backup(path: &Path) {
    let _ = std::fs::rename(path, path.with_extension("toml.bak"));
}

fn save(path: &Path, settings: &Settings) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let text = toml::to_string(settings).map_err(std::io::Error::other)?;
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, text)?;
    // Replaces the old file in one step (also on Windows).
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh folder under the system temp dir, removed on drop.
    struct TempDir(PathBuf);
    impl TempDir {
        fn new(name: &str) -> Self {
            use std::sync::atomic::{AtomicU32, Ordering};
            static NEXT: AtomicU32 = AtomicU32::new(0);
            let n = NEXT.fetch_add(1, Ordering::Relaxed);
            let dir =
                std::env::temp_dir().join(format!("cm-settings-{name}-{}-{n}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            Self(dir)
        }
        fn file(&self) -> PathBuf {
            self.0.join(FILE_NAME)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn sample() -> Settings {
        let meter = Meter::from_formula("3+2").unwrap();
        Settings {
            version: SCHEMA_VERSION,
            metronome: MetronomeState::new(Tempo::from_bpm(96.5), &meter, Subdivision::Triple, 0.4),
            window: WindowState {
                size: Some([500.0, 360.0]),
                position: Some([120.0, 80.0]),
                compact: true,
                pinned: true,
                zoom: 1.2,
            },
        }
    }

    #[test]
    fn round_trip_through_the_file() {
        let dir = TempDir::new("roundtrip");
        let (mut store, loaded) = Store::open_at(Some(dir.file()));
        assert_eq!(loaded, Settings::default(), "no file yet");
        let s = sample();
        let t0 = Instant::now();
        store.update(s.clone(), t0);
        store.save_if_due(t0);
        assert!(!dir.file().exists(), "not before the delay");
        store.save_if_due(t0 + SAVE_DELAY);
        let (_, again) = Store::open_at(Some(dir.file()));
        assert_eq!(again, s);
        assert_eq!(again.metronome.meter(), Meter::from_formula("3+2").unwrap());
        assert_eq!(again.metronome.subdivision(), Subdivision::Triple);
        assert_eq!(again.metronome.tempo(), Tempo::from_bpm(96.5));
    }

    #[test]
    fn delay_restarts_on_every_change() {
        let (mut store, _) = Store::open_at(None);
        let t0 = Instant::now();
        let mut s = sample();
        store.update(s.clone(), t0);
        s.metronome.tempo = 100.0;
        store.update(s.clone(), t0 + Duration::from_millis(800));
        assert_eq!(
            store.next_due(t0 + Duration::from_millis(800)),
            Some(SAVE_DELAY)
        );
        // Unchanged settings do not restart it.
        store.update(s, t0 + Duration::from_millis(900));
        assert_eq!(
            store.next_due(t0 + Duration::from_millis(900)),
            Some(Duration::from_millis(900))
        );
        // Going back to what is saved cancels the write.
        store.update(Settings::default(), t0 + Duration::from_millis(950));
        assert_eq!(store.next_due(t0), None);
    }

    #[test]
    fn broken_file_is_kept_as_backup() {
        let dir = TempDir::new("broken");
        std::fs::create_dir_all(&dir.0).unwrap();
        std::fs::write(dir.file(), "this is [not toml").unwrap();
        let (_, loaded) = Store::open_at(Some(dir.file()));
        assert_eq!(loaded, Settings::default());
        assert!(dir.file().with_extension("toml.bak").exists());
        assert!(!dir.file().exists());
    }

    #[test]
    fn missing_and_unknown_fields() {
        let text = "version = 7\nfuture = 1\n[metronome]\ntempo = 88.0\n[window]\nzoom = 1.5\nnew = \"x\"\n";
        let s: Settings = toml::from_str(text).unwrap();
        assert_eq!(s.metronome.tempo(), Tempo::from_bpm(88.0));
        assert_eq!(s.metronome.meter(), Meter::default());
        assert!((s.window.zoom - 1.5).abs() < f32::EPSILON);
        assert!(!s.window.compact);
    }

    #[test]
    fn invalid_values_fall_back() {
        let mut m = MetronomeState {
            grid: "Xo".into(),
            subdivision: "eighths".into(),
            volume: f32::NAN,
            ..MetronomeState::default()
        };
        assert_eq!(m.meter(), Meter::default());
        assert_eq!(m.subdivision(), Subdivision::None);
        assert!((m.volume() - 1.0).abs() < f32::EPSILON);
        m.tempo = 5000.0;
        assert_eq!(m.tempo(), Tempo::MAX);
        let w = WindowState {
            position: Some([-32_000.0, -32_000.0]),
            size: Some([100.0, 100.0]),
            ..WindowState::default()
        };
        assert_eq!(w.position(), None, "minimized-window position");
        assert_eq!(w.size([380.0, 280.0]), Some([380.0, 280.0]));
    }
}
