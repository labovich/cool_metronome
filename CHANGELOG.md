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
