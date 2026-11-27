# Cool Metronome

A precise, lightweight, open-source metronome for Windows and macOS, written in Rust.

> **Status:** early development (stage 4 of 8: main window done; title bar, compact mode
> and keyboard control next).

## Goals

- **Accurate.** Clicks are scheduled inside the audio stream with sample accuracy
  (error under half a sample, no drift over any session length), not with OS timers.
  Accuracy is checked by an automated test on every commit and by a loopback
  measurement on real hardware before each release.
- **Light.** Near-zero CPU when idle, small memory footprint, small installer.
- **Practical.** Complex and additive meters (3+2, 7/8…), per-beat accents,
  subdivisions, Latin and flamenco rhythm patterns, presets, tap tempo,
  global hotkeys, tray icon, always-on-top and compact modes, export to WAV/FLAC/OGG.

## Platforms

| Platform | Status |
| --- | --- |
| Windows 10 and later, x64 | Primary target |
| macOS 10.15 and later, Intel and Apple Silicon (universal) | Built and released; manual testing on Intel only |
| Linux | Planned |

## Download

- **Development build** — the latest commit on `master`, rebuilt on every push:
  [Releases → Development build](https://github.com/labovich/cool_metronome/releases/tag/dev)
  - Windows x64: [CoolMetronome-dev-windows-x64.zip](https://github.com/labovich/cool_metronome/releases/download/dev/CoolMetronome-dev-windows-x64.zip)
  - macOS (Intel and Apple Silicon): [CoolMetronome-dev-macos-universal.dmg](https://github.com/labovich/cool_metronome/releases/download/dev/CoolMetronome-dev-macos-universal.dmg)
- **Versions** — tagged releases on the [Releases](https://github.com/labovich/cool_metronome/releases) page
  (none yet).

On Windows, unzip and run `CoolMetronome.exe`; nothing is installed. On macOS, open the
`.dmg` and drag the app to *Applications*. The builds are not code-signed yet, see
[Installing unsigned builds](#installing-unsigned-builds).

## Building from source

Requires Rust 1.95 or later (installed automatically from `rust-toolchain.toml` via rustup).

```sh
cargo run -p cm-app            # run the app (debug)
cargo build -p cm-app --release
cargo test --workspace         # unit tests and the timing-accuracy tests
```

## Project layout

| Crate | Purpose |
| --- | --- |
| `cm-core` | Rhythm model: tempo, meters, subdivisions, rhythm library, presets. No I/O. |
| `cm-engine` | Sample-accurate scheduler and mixer. Shared by playback, export and tests. |
| `cm-audio` | Audio output (cpal), device selection, sample decoding and resampling. |
| `cm-export` | Export to WAV, FLAC and OGG. |
| `cm-app` | The egui desktop application. |
| `cm-probe` | Loopback tool that measures timing on real audio hardware. |

## Installing unsigned builds

Release builds are not code-signed yet.

- **Windows:** SmartScreen may warn about an unknown publisher. Click
  *More info → Run anyway*.
- **macOS:** the first launch is blocked by Gatekeeper. On macOS 14 and earlier,
  right-click the app and choose *Open*. On macOS 15 and later, open
  *System Settings → Privacy & Security* and click *Open Anyway*.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
