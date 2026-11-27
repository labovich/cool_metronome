# Cool Metronome: English UI strings.
# To add a language, copy this file to <lang>.ftl and translate the values.

app-title = Cool Metronome

## Window

window-pin = Keep on top
window-tray = Hide to tray
window-tray-later = Hiding to the tray arrives in a later version.
window-minimize = Minimize
window-close = Close
window-expand = Expand to full window
window-compact-hint = Double-click for compact mode

## Options window

options-open = Options…
options-title = Options
options-tab-interface = Interface
options-tab-audio = Audio
options-tab-volume = Volume
options-defaults = Restore defaults

options-theme = Theme
theme-system = System
theme-light = Light
theme-dark = Dark
options-slider-range = Tempo slider
options-quick-jump = Quick jump
options-tap = Tap tempo
options-tap-fractional = Fractional result (0.1 BPM)

options-device = Output device
options-device-default = System default
options-refresh = Refresh
options-now-playing = Playing on
options-no-output = No audio output
options-device-missing = The chosen device is not available; playing on the system default.

options-slot-accent = Accent
options-slot-normal = Normal beat
options-slot-sub = Subdivision
options-slot-bell = Bell
options-pan = Pan
options-pan-left = L
options-pan-right = R
options-pan-center = Center
options-bell = Bell on the bar start in compound meters (3+2…)
options-fade-in = Fade-in after start
options-off = Off
options-try = Try
options-boost = Boost +6 dB
options-accent-mix = Accent also plays the normal beat sound

## Tempo

# Shown under the big BPM number: "BPM · Allegro · 1:23".
tempo-subtitle = BPM · { $marking } · { $elapsed }
tempo-slower = Slower
tempo-faster = Faster
tempo-slider = Tempo
bpm = BPM
tempo-edit-tooltip = Double-click to type a tempo
tempo-edit-hint = Enter to apply · Esc to cancel
tempo-edit-invalid = Type a tempo from 1 to 999 BPM

tempo-largo = Largo
tempo-larghetto = Larghetto
tempo-adagio = Adagio
tempo-andante = Andante
tempo-moderato = Moderato
tempo-allegro = Allegro
tempo-presto = Presto
tempo-prestissimo = Prestissimo

## Beats

# Accessible name of a beat circle.
beat-label = Beat { $number }: { $level ->
    [accent] accent
    [mute] muted
   *[normal] normal
}

## Bottom row

meter = Meter
meter-section-meters = Meters
subdivision = Subdivision
presets = Presets
presets-later = Presets arrive in a later version.
volume = Volume
volume-muted = Muted
tap = TAP
tap-hint = Tap tempo
play = Play
stop = Stop

subdivision-none = No subdiv
subdivision-single = Single
subdivision-duple = Duple
subdivision-triple = Triplet
subdivision-quadruple = Quad
subdivision-quintuple = Quintuplet
subdivision-sextuple = Sextuplet
subdivision-septuple = Septuplet
subdivision-shuffle = Shuffle
subdivision-quadruple-fourth = Quad 1st + 4th

## Rhythm library (ids from crates/cm-core/rhythms)
# Optional .short: shown on the meter chip when the full name is long.

rhythm-category-latin = Latin
rhythm-category-flamenco = Flamenco

rhythm-son-clave-3-2 = Son clave 3-2
    .short = Son 3-2
rhythm-son-clave-2-3 = Son clave 2-3
    .short = Son 2-3
rhythm-rumba-clave-3-2 = Rumba clave 3-2
    .short = Rumba 3-2
rhythm-rumba-clave-2-3 = Rumba clave 2-3
    .short = Rumba 2-3
rhythm-afro-6-8 = 6/8 Afro
rhythm-bossa-nova-3-2 = Bossa nova 3-2
    .short = Bossa 3-2
rhythm-bossa-nova-2-3 = Bossa nova 2-3
    .short = Bossa 2-3
rhythm-samba = Samba
rhythm-merengue = Merengue
rhythm-solea = Soleá
rhythm-seguiriya = Seguiriya
rhythm-buleria = Bulería

## Audio errors

audio-error-no-device = No audio output device found
audio-error-device-not-found = Audio device "{ $name }" not found
audio-error-config = Cannot read the audio device settings: { $detail }
audio-error-format = Unsupported audio sample format { $detail }
audio-error-stream = Cannot start audio output: { $detail }
audio-error-not-responding = The audio engine is not responding
