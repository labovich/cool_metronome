# Cool Metronome — development plan

Status as of 2026-11-27. Source of truth for decisions made with the project owner.
Original discussion was in Russian; reply to the owner in Russian.

## Goal

Open-source, cross-platform metronome in Rust that reproduces the base features of
Fine Metronome 3 (Windows) with a modern UI. Top priority: sample-accurate timing with
no drift. Second: minimal resource use — weigh it in every technical decision.

## Requirements (decided)

| Topic | Decision |
| --- | --- |
| Platforms | Windows 10+ x64 first. macOS 10.15+ universal (x86_64 + arm64): built and released from CI now; owner tests manually on an Intel Mac mini (macOS 12 Monterey), arm64 only via CI. No Windows ARM64. Linux later — keep the architecture portable. |
| Language / UI | Rust, egui via eframe with the **glow** backend (not wgpu: smaller binary, less RAM). |
| License | MIT OR Apache-2.0. Dependencies must be compatible (`cargo deny`). |
| Tempo | Typed input 1–999 BPM, step 0.1. Slider range 40–208 by default, its bounds configurable within 1–999 (as in the original). |
| Audio output | System shared mode (WASAPI / CoreAudio) via cpal; device and buffer size selectable. ASIO / WASAPI exclusive: not now. |
| External sync | None (no MIDI clock, no Ableton Link). |
| Sounds | 4 built-in own sounds (accent, normal, subdivision, bell), synthesized in code; user can load own samples (WAV/FLAC/OGG/MP3 decode). |
| Export | WAV, FLAC, OGG. **No MP3** (deliberate). |
| Not in 1.0 | Diary, animator (FPS) mode, gamepad/joystick control, MP3, ASIO, Linux, translations. |
| Language | English UI only; all strings in Fluent `.ftl` so languages can be added without code changes. |
| Windows | Light and dark themes (default: follow system), always-on-top, tray. The close button **quits** the app; a separate button hides to tray. |
| Hotkeys | Local and global, all rebindable. Global default **Ctrl+Alt+Space** (Alt+Space is the Windows system menu). |
| Settings | In the user profile: `%APPDATA%\CoolMetronome`, `~/Library/Application Support/CoolMetronome`. TOML with a schema version. No compatibility with Fine Metronome files. |
| Distribution | Installer without admin rights (Inno Setup, per-user into `%LOCALAPPDATA%\Programs`). No code signing for now. Update check via GitHub Releases API (notify only, at most daily, can be disabled). |
| Repo / CI | GitHub + GitHub Actions. Releases built on tag `v*`. |

## Features of version 1.0 (base functionality of Fine Metronome 3)

- Play/stop, tempo (slider, typed, ±1, ±0.1, quick jump step linear or logarithmic,
  ×2 / ÷2), elapsed timer, beat counter.
- Tap tempo: average of recent taps, integer or fractional (0.1) result.
- Meters: presets 1, 2, 3, 4, 3+2, 5/2, 7/2; keys 1–8 (5 = 3+2).
- Subdivisions: none, single, duple … septuple, shuffle (triplet 1st+3rd),
  quadruple 1st+4th. Independent of subunits ("two against three").
- Rhythm library: son clave 3-2/2-3, rumba clave 3-2/2-3, 6/8 Afro, bossa nova 3-2/2-3,
  samba, merengue; flamenco: soleá, seguiriya, bulería. Clave patterns use 4 subunits per beat.
- Meter constructor: grouping formula (`3+2+2`), grid of cells (accent / normal / mute),
  ±beat, subunits per beat (the last subunit can be cut: 5/8, 7/8 without doubling the tempo).
- Presets: name, tempo, meter, subdivision, divisor (= subunits per beat); add, edit, delete, search.
- Sound: 4 slots, own samples, "mix accent with beat sound".
- Volume: master (lowest slider position = mute) + per slot in dB (defaults: accent 0,
  normal −5, sub −15, bell −9), pan, bell in compound meters, fade-in after start, +6 dB boost.
- Beat flasher: separate borderless always-on-top window, colors for downbeat/beat/tap,
  shows the beat number, remembers position and size, has reset.
- Export: one loopable bar, N bars or a duration; 44.1/48 kHz, 16/24 bit.
- System: output device, buffer size, switch device when the current one disappears.
- Bell (not described in the original manual): marks the bar start in meters with
  several accented beat starts (3+2). Verified by the owner against Fine Metronome.

## Architecture

Cargo workspace (`crates/`):

| Crate | Role | Status |
| --- | --- | --- |
| `cm-core` | Pure model, no I/O: `Tempo`, `Meter`, `BeatLevel`, `Subdivision`, rhythm library (TOML in `crates/cm-core/rhythms/`, embedded at compile time); later presets. | Done for stage 3 |
| `cm-engine` | Sample-accurate scheduler + mixer, no device, no allocations in the hot path. Shared by playback, export and tests. | Done |
| `cm-audio` | cpal output, rtrb command queue, retired-object queue, device recovery; later sample decoding (symphonia) + resampling (rubato). | Done (basic) |
| `cm-export` | WAV (hound), FLAC (flacenc), OGG (vorbis_rs). | Stage 7 |
| `cm-app` | egui app; tray-icon, global-hotkey, settings, Fluent i18n, update check. | Temporary test screen |
| `cm-probe` | Loopback measurement tool for real hardware. | Stage 8 |

### Timing rules (do not break)

1. Clicks are scheduled inside the audio callback, never with OS timers.
2. Event sample = `round(anchor_sample + (beats - anchor_beat) * samples_per_beat)`,
   computed from the musical position each time — never by adding periods. Error < 0.5
   sample, no drift.
3. Tempo change: the pending click keeps its position; the new tempo starts from it.
4. Pattern change: applied at the next bar start (immediately when stopped).
5. Audio callback: no allocation, free, locks, I/O or logging. Replaced objects go back
   through a queue and are dropped on the UI thread (`AudioOutput::collect_garbage`).
6. UI repaints only on events / while playing. Idle CPU ≈ 0%.

Resource targets (guidelines, measure before release): idle ≈0% CPU, playing ≤1%,
≤50 MB RAM, installer ≤10 MB.

## UI (approved mockup)

Mockup: https://claude.ai/artifact/BVhZeHuTeRKm6ELGkoPBXx (approved 2026-11-22).
Own modern design, not a copy of the original.

- Main window 440×320 logical points by default (Full HD 100% = 440×320 px, 4K 200% =
  880×640 px). Resizable, min 380×280, size/position remembered. Per-monitor DPI plus own
  UI zoom 80–200% (Ctrl+= / Ctrl+− / Ctrl+0).
- Custom title bar: app name; buttons pin (always on top), hide to tray, minimize, close (= quit).
- Beat row: circles per beat, accent larger and orange (#FF8A3D), muted = outline; click
  cycles normal → accent → mute; current beat highlighted.
- Big BPM number with − / + buttons; below: "BPM · tempo name (Allegro…) · timer".
- Tempo slider.
- Bottom row: meter chip, subdivision chip, presets, volume, TAP, big round Play.
- Compact mode (double-click title bar): 440×96 strip with BPM ±, beats, Play, expand, close.
- Dark palette: bg #16171B, surface #25272E, border #30333B, text #F1F1F4, secondary #A3A5AD.
  Light: bg #FAFAFB, surface #ECEDF0, border #DADCE1, text #18191D, secondary #585B63. Font Inter.
- Default keymap (as in the original): Space play/stop; ←/→ ±1 BPM (Shift ±0.1);
  PgUp/PgDn and Ctrl+←/→ quick jump (6 by default); Shift+− / Shift+= halve/double;
  ↑/↓ volume (Ctrl coarse); Enter or T tap; 1–8 beats; Ctrl+0…7 subdivision, Alt+3 shuffle,
  Alt+4 quadruple 1st+4th; M meter constructor, O options, P presets, E export;
  mouse wheel tempo, Ctrl+wheel volume. Global: Ctrl+Alt+Space play/stop.

## Testing

| Check | What | Criterion |
| --- | --- | --- |
| Engine accuracy (CI) | `crates/cm-engine/tests/accuracy.rs`: 1 h at 48 kHz for 5 meters × 4 tempos; 44.1–192 kHz at 1–999 BPM; render vs schedule for random and fixed block sizes; random tempo changes during playback. | ≤ 1 sample, no drift |
| Hardware (manual, before release) | `cm-probe`: play + record the same card (loopback), detect onsets, report jitter/drift. | jitter ≤ 1 ms; thresholds refined after first runs |
| Model / settings / export (CI) | Unit tests; export round-trip. | Green |
| Resources (manual) | CPU/RAM idle, playing, in tray; installer size in CI. | Targets above |

The accuracy tests were mutation-checked: removing the tempo-change anchor or computing
the period in f32 makes them fail.

## CI / release

- `.github/workflows/ci.yml`: fmt; clippy + tests + release build on Windows and macOS;
  Linux and MSRV 1.95 jobs build everything except `cm-app` (they install `libasound2-dev`);
  cargo-deny. `RUSTFLAGS=-D warnings`.
- `dtolnay/rust-toolchain`: use `@stable`, or `@master` with `toolchain:` input. Never `@1.95`
  — Dependabot "upgrades" it to non-existent Rust versions.
- `.github/workflows/release.yml` (tag `v*`): Windows zip (Inno Setup installer in stage 8),
  macOS universal `.app` (ad-hoc signed) in `.dmg`, SHA256SUMS, GitHub Release.

## Stages

1. **Done.** Workspace, licenses, README, CI, release skeleton.
2. **Done, pending owner's check by ear.** Engine, 4 synthesized sounds, cpal output,
   accuracy tests, temporary test screen in `cm-app` (it looks rough — it is temporary).
3. **Done.** Rhythm model: rhythm library (Latin, flamenco) as TOML data files with a
   cell grid (`X` accent, `x` normal, `.` mute), picker on the test screen, accuracy
   tests for every library rhythm; bell logic verified by the owner. Samba, merengue and
   flamenco grids are typical variants — to be refined by ear later.
4. UI per mockup: main window, themes, Fluent strings, custom title bar, compact mode.
5. Settings and data: options window (Sound, Volume, Interface, Hotkeys, Audio), own
   samples, per-slot volume, pan, fade-in, boost, device choice, profile storage,
   meter constructor, presets.
6. System integration: tray, always-on-top, beat flasher window, local + global hotkeys
   with editor, update check.
7. Export WAV/FLAC/OGG + tests.
8. Release 1.0: `cm-probe` measurements on the owner's hardware, resource profiling,
   Inno Setup installer, manual checklist, tag v1.0.0.

After 1.0: Linux, code signing (SignPath for OSS / Apple notarization), diary, animator
mode, translations, ASIO / WASAPI exclusive, gamepad.

## Risks

- Unsigned builds: SmartScreen warning on Windows; Gatekeeper on macOS (README explains).
- Bluetooth output adds 150–300 ms latency: rhythm unaffected, flasher lags — add a manual
  visual offset setting.
- egui needs OpenGL (glow); show a clear message if unavailable.
- macOS arm64 is only CI-tested.
