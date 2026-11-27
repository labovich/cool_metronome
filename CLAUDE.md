# Cool Metronome — notes for Claude

Precise, lightweight, open-source metronome in Rust (egui/eframe + cpal).
**Read `docs/PLAN.md` first**: requirements, decisions, architecture, UI spec, stages.
UI mockup source (exact sizes, colors, layout): `docs/design/` (see its README).

## Working with the owner

- Reply in **Russian**. Code, comments, commit messages and UI strings in English.
- The owner develops on Windows 10/11 x64 (PowerShell). macOS testing on an Intel Mac mini.
- Before starting a stage, say what it will contain and wait for "go".
- Commit after every significant change (a feature, a fix, a stage part) so the history
  tracks the work; run fmt, clippy and tests first. Do not push unless asked.

## Commands

```sh
cargo run -p cm-app                                   # run the app
cargo test --workspace                                # all tests incl. timing accuracy
cargo clippy --workspace --all-targets -- -D warnings # CI treats warnings as errors
cargo fmt --all
```

Run fmt, clippy and tests before handing work back.

## Hard rules

- Timing lives in `cm-engine`. Event positions are computed from the musical position,
  never accumulated. Any timing change needs sample-exact tests in
  `crates/cm-engine/tests/accuracy.rs`.
- Nothing in the audio callback may allocate, free, lock, do I/O or log. Replaced objects
  are returned via `Retired` and dropped on the UI thread.
- Weigh CPU/RAM/binary size in every decision. Repaint the UI only when needed.
- User-visible strings go through Fluent (`.ftl`) once i18n exists (stage 4).
- New dependencies: license must pass `deny.toml`; prefer small, pure-Rust crates.
- In `.github/workflows`, never pin `dtolnay/rust-toolchain@<version>`
  (use `@stable` or `@master` + `toolchain:`).

## Current state

Stages 1–3 and 4a done (see `docs/PLAN.md` → Stages): `cm-app` shows the main window per
the mockup. Next: 4b (title bar, compact mode, keymap).
