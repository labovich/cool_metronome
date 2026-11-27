# UI mockup (approved 2026-11-22)

Source of the approved mockup from the Claude design canvas:
https://claude.ai/artifact/BVhZeHuTeRKm6ELGkoPBXx (private to the owner).

| File | What |
| --- | --- |
| `Main-Dark.dc.html` | Main window, dark theme, 440×320 logical points |
| `Main-Light.dc.html` | Main window, light theme (same layout) |
| `Compact.dc.html` | Compact mode, 440×96 |

These files are the canvas's "Design Component" format: HTML + inline CSS with
`{{placeholders}}`, `<sc-for>` loops and a small `Component` class at the bottom.
They do not open as plain web pages, but they are the exact reference for
layout, sizes (px = logical points), colors, fonts, spacing and behavior
(beat click cycles normal → accent → mute, play/stop, tap, meter/subdivision chips).

Screenshots, when exported from the canvas, go next to these files as `*.png`.
The written UI spec is in `../PLAN.md` → "UI (approved mockup)".
