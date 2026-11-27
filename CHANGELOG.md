# Changelog

All notable changes to this project are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- Project skeleton: Cargo workspace with `cm-core`, `cm-engine`, `cm-audio`,
  `cm-export`, `cm-app` and `cm-probe`.
- `Tempo` type (1–999 BPM, 0.1 step) and a drift-free `BeatGrid`.
- CI on Windows, macOS and Linux; release workflow skeleton.
- Rhythm model: meters from grouping formulas (`3+2`), accent/normal/mute cells,
  subunits per beat with cut bars (5/8, 7/8), ten subdivision types.
- Sample-accurate engine: bar patterns, drift-free scheduler with tempo changes
  at the next click, voice mixer with gains and constant-power pan, four
  synthesized click sounds (accent, normal, subdivision, bell).
- Audio output through cpal with a lock-free command queue and automatic
  recovery when the output device disappears.
- Timing-accuracy tests: one hour at 48 kHz, 44.1–192 kHz at 1–999 BPM,
  block-size independence, random tempo changes during playback.
- Temporary engine test screen: play/stop, tempo, meter, subdivision, bell,
  clickable beat indicator.
- Rhythm library as TOML data files embedded in the binary: son and rumba clave
  3-2/2-3, 6/8 Afro, bossa nova 3-2/2-3, samba, merengue, soleá, seguiriya,
  bulería. Meters can be written as cell grids (`X..x ..x.`).
- Main window per the approved mockup: beat row (click cycles normal / accent /
  mute), big BPM with tempo marking and elapsed timer, tempo slider, meter menu
  (presets and the rhythm library), subdivision menu, master volume, tap tempo,
  Play. Light and dark themes follow the system.
- Embedded Inter font subset; egui's default fonts are no longer linked (the
  release binary got ~0.5 MB smaller).
- UI strings in Fluent (`crates/cm-app/i18n/en-US.ftl`).
- UI zoom 80–200 % with Ctrl+= / Ctrl+− / Ctrl+Shift+0.
- Typed tempo: double-click the BPM number, type 1–999 ("." or "," for tenths),
  Enter applies, Esc cancels.
- The beat indicator lights up when the beat is heard, not when it is rendered.
- Own window frame: rounded borderless window, title bar with keep-on-top, minimize
  and close; drag by the title bar, resize by the edges.
- Settings are kept in the user profile (`settings.toml`): tempo, meter,
  subdivision, volume, window size and position, compact mode, keep on top, zoom.
- Options window (key O, or right-click → Options…): theme, tempo slider range,
  quick jump step in BPM or percent, fractional tap tempo, output device,
  per-sound volume in dB, pan, bell in compound meters.
- Own click sounds (Options → Sound): WAV, FLAC, OGG or MP3 per slot, with a
  Try button; a file that cannot be used keeps the built-in sound and says why.
- Options → Volume: fade-in after start, +6 dB boost, accent also plays the
  normal sound. Options → Audio: buffer size.
- Compact mode (440×96): double-click the title bar; expand button or a
  double-click on the strip returns to the full window.
- Keyboard: Space play/stop, ←/→ ±1 BPM (Shift ±0.1), PgUp/PgDn and Ctrl+←/→ ±6,
  Shift+= / Shift+− double/halve, ↑/↓ volume (Ctrl coarse), Enter or T tap,
  1–8 meters (5 = 3+2), Ctrl+0…7 subdivisions, Alt+3 shuffle, Alt+4 quadruple
  1st+4th. Mouse wheel changes the tempo, Ctrl+wheel the volume.

### Fixed
- Audio housekeeping (freeing replaced objects, recovering a lost device) now
  also runs while the window is minimized or the metronome is idle.
- CI runs on pushes to `master`.
- Every push to `master` publishes a "Development build" pre-release (tag `dev`)
  with the Windows zip and the macOS dmg; tagged versions still get regular
  releases. Release archives now include the Inter font license.
- The title bar hint no longer shows over the title bar buttons.
- The tempo and volume sliders no longer react to the right mouse button.
- Playing used ~20 % of a CPU core for UI repaints; now ~2.5 %.
