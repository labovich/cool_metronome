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

### Fixed
- Audio housekeeping (freeing replaced objects, recovering a lost device) now
  also runs while the window is minimized or the metronome is idle.
- CI runs on pushes to `master`.
- Playing used ~20 % of a CPU core for UI repaints; now ~2.5 %.
