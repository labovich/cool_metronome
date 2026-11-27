# Contributing to Cool Metronome

Thanks for your interest! Bug reports, ideas and pull requests are welcome.

## Before you open a pull request

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

CI runs the same checks on Windows, macOS and Linux.

## Ground rules

- **Timing first.** Anything that affects when a click sounds lives in `cm-engine`
  and must be covered by tests that check exact sample positions.
- **No blocking in the audio callback.** No allocation, locks, I/O or logging
  in code that runs on the audio thread.
- **Mind the resources.** Prefer solutions that keep CPU near zero when idle and
  the binary small. Repaint the UI only when something changed.
- **Strings go through localization.** User-visible text belongs in the Fluent
  (`.ftl`) files, not in code.
- **Dependencies** must be licensed compatibly with MIT OR Apache-2.0
  (`cargo deny check` enforces this).

## Commit messages

Short imperative subject line (`Add tap tempo averaging`), optional body explaining why.
Add a line to `CHANGELOG.md` under *Unreleased* for user-visible changes.
