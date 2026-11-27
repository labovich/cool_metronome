//! The main window content, laid out as in the mockup (`docs/design/Main-*.dc.html`).
//!
//! Widgets are painted directly at computed rects: the layout is a fixed
//! design scaled by egui's zoom, not a flowing form.

use crate::i18n::I18n;
use crate::icons::{self, Icon};
use crate::player::{Player, PlayerError};
use crate::theme::{self, ACCENT, ON_ACCENT, Palette};
use cm_audio::AudioError;
use cm_core::{BeatLevel, Meter, Rhythm, RhythmCategory, Subdivision, Tempo};
use egui::text::{LayoutJob, TextFormat, TextWrapping};
use egui::{
    Align2, Color32, CursorIcon, FontId, Galley, Id, Popup, PopupCloseBehavior, Rect, Response,
    Role, Sense, Stroke, StrokeKind, Ui, WidgetInfo, pos2, vec2,
};
use std::sync::Arc;
use std::time::{Duration, Instant};

// Mockup measurements, in points.
const PAD_TOP: f32 = 6.0;
const PAD_X: f32 = 20.0;
const PAD_BOTTOM: f32 = 18.0;
const BEATS_H: f32 = 34.0;
const TEMPO_H: f32 = 82.0;
const SLIDER_H: f32 = 20.0;
const BOTTOM_H: f32 = 52.0;
const BEAT_GAP: f32 = 10.0;
const ROW_GAP: f32 = 6.0;
/// Tempo slider range (configurable within 1–999 in stage 5).
const SLIDER_MIN: f64 = 40.0;
const SLIDER_MAX: f64 = 208.0;

/// What the main view needs for one frame.
pub struct View<'a> {
    pub player: &'a mut Player,
    pub i18n: &'a I18n,
    pub rhythms: &'a [Rhythm],
    pub p: Palette,
    pub now: Instant,
}

impl View<'_> {
    pub fn show(&mut self, ui: &mut Ui, rect: Rect) {
        let inner = Rect::from_min_max(
            rect.min + vec2(PAD_X, PAD_TOP),
            rect.max - vec2(PAD_X, PAD_BOTTOM),
        );
        // Rows spread over the height, as `justify-content: space-between`.
        let fixed = BEATS_H + TEMPO_H + SLIDER_H + BOTTOM_H;
        let gap = ((inner.height() - fixed) / 3.0).max(4.0);
        let row =
            |y: f32, h: f32| Rect::from_min_size(pos2(inner.left(), y), vec2(inner.width(), h));
        let mut y = inner.top();
        self.beats(ui, row(y, BEATS_H));
        y += BEATS_H + gap;
        self.tempo(ui, row(y, TEMPO_H));
        y += TEMPO_H + gap;
        self.tempo_slider(ui, row(y, SLIDER_H));
        y += SLIDER_H + gap;
        self.bottom_row(ui, row(y, BOTTOM_H));
    }

    // ------------------------------------------------------------ beats

    fn beats(&mut self, ui: &mut Ui, rect: Rect) {
        let cells: Vec<BeatLevel> = self.player.meter().cells().to_vec();
        let current = self.player.audible_cell(self.now);
        let size = |l: &BeatLevel| if *l == BeatLevel::Accent { 30.0 } else { 24.0 };
        let natural: f32 =
            cells.iter().map(size).sum::<f32>() + BEAT_GAP * (cells.len().saturating_sub(1)) as f32;
        // Long bars (16-cell clave) shrink to fit, keeping room for the ring.
        let scale = ((rect.width() - 12.0) / natural).min(1.0);
        let painter = ui.painter();
        let mut x = rect.center().x - natural * scale / 2.0;
        let mut clicked = None;
        for (i, level) in cells.iter().enumerate() {
            let d = size(level) * scale;
            let center = pos2(x + d / 2.0, rect.center().y);
            let hit = Rect::from_center_size(center, vec2(d + BEAT_GAP * scale, rect.height()));
            let id = Id::unique(("beat", i));
            let resp = ui
                .interact(hit, id, Sense::click())
                .on_hover_cursor(CursorIcon::PointingHand);
            let level_id = match level {
                BeatLevel::Accent => "accent",
                BeatLevel::Normal => "normal",
                BeatLevel::Mute => "mute",
            };
            let label = self.i18n.tr_args(
                "beat-label",
                &[("number", (i + 1).into()), ("level", level_id.into())],
            );
            resp.widget_info(|| WidgetInfo::labeled(Role::Button, true, &label));

            // No transition animation: one frame per beat keeps CPU near zero.
            let on = current == Some(i);
            let r = d / 2.0 * if on { 1.12 } else { 1.0 };
            let ring = if *level == BeatLevel::Accent {
                ACCENT
            } else {
                self.p.beat
            };
            match level {
                BeatLevel::Accent => painter.circle_filled(center, r, ACCENT),
                BeatLevel::Normal => painter.circle_filled(center, r, self.p.beat),
                BeatLevel::Mute => painter.circle_stroke(
                    center,
                    r - scale,
                    Stroke::new(2.0 * scale, self.p.border2),
                ),
            };
            if on {
                // box-shadow: 3 px gap in the background color, then a 3 px ring.
                painter.circle_stroke(center, r + 4.5 * scale, Stroke::new(3.0 * scale, ring));
            }
            if resp.clicked() {
                clicked = Some(i);
            }
            x += d + BEAT_GAP * scale;
        }
        if let Some(i) = clicked {
            self.player.cycle_cell(i);
        }
    }

    // ------------------------------------------------------------ tempo

    fn tempo(&mut self, ui: &mut Ui, rect: Rect) {
        let cy = rect.center().y;
        let minus = Rect::from_center_size(pos2(rect.left() + 22.0, cy), vec2(44.0, 44.0));
        let plus = Rect::from_center_size(pos2(rect.right() - 22.0, cy), vec2(44.0, 44.0));
        let step = if ui.input(|i| i.modifiers.shift) {
            0.1
        } else {
            1.0
        };
        let label = self.i18n.tr("tempo-slower");
        if self.round_icon(ui, minus, Icon::Minus, &label).clicked() {
            self.player.set_tempo(self.player.tempo().offset(-step));
        }
        let label = self.i18n.tr("tempo-faster");
        if self.round_icon(ui, plus, Icon::Plus, &label).clicked() {
            self.player.set_tempo(self.player.tempo().offset(step));
        }

        // Big number: whole part 64 pt, ".tenths" 20 pt, baselines aligned.
        let painter = ui.painter();
        let tempo = self.player.tempo();
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let tenths = (tempo.bpm() * 10.0).round() as u32;
        let whole = text(
            painter,
            &(tenths / 10).to_string(),
            theme::bold(64.0),
            self.p.text,
            -2.0,
        );
        let frac = text(
            painter,
            &format!(".{}", tenths % 10),
            theme::bold(20.0),
            self.p.text2,
            0.0,
        );
        let block_h = 64.0 + 2.0 + 15.0;
        let line_top = rect.top() + (rect.height() - block_h) / 2.0;
        // CSS line box of 64 px: Inter's baseline sits 55 px below its top.
        let baseline = line_top + 55.25;
        let width = whole.size().x + 6.0 + frac.size().x;
        let x = rect.center().x - width / 2.0;
        painter.galley(
            pos2(x, baseline - baseline_of(&whole)),
            whole.clone(),
            self.p.text,
        );
        painter.galley(
            pos2(x + whole.size().x + 6.0, baseline - baseline_of(&frac)),
            frac,
            self.p.text2,
        );

        let (line, color) = match self.player.error() {
            Some(e) => (error_text(self.i18n, e), ACCENT),
            None => {
                let marking = self.i18n.tr(&format!("tempo-{}", tempo.marking().id()));
                let elapsed = format_elapsed(self.player.elapsed(self.now));
                let line = self.i18n.tr_args(
                    "tempo-subtitle",
                    &[("marking", marking.into()), ("elapsed", elapsed.into())],
                );
                (line, self.p.text2)
            }
        };
        let max_w = plus.left() - minus.right() - 8.0;
        let sub = text_elided(painter, &line, theme::medium(12.0), color, 0.4, max_w);
        let pos = pos2(rect.center().x - sub.size().x / 2.0, line_top + 66.0);
        painter.galley(pos, sub, color);
    }

    fn tempo_slider(&mut self, ui: &mut Ui, rect: Rect) {
        let bpm = self.player.tempo().bpm();
        let t = ((bpm - SLIDER_MIN) / (SLIDER_MAX - SLIDER_MIN)).clamp(0.0, 1.0);
        #[allow(clippy::cast_possible_truncation)]
        let t = t as f32;
        let label = self.i18n.tr("tempo-slider");
        if let Some(t) = slider(
            ui,
            rect,
            Id::unique("tempo-slider"),
            t,
            &label,
            bpm,
            &self.p,
        ) {
            let bpm = SLIDER_MIN + f64::from(t) * (SLIDER_MAX - SLIDER_MIN);
            self.player.set_tempo(Tempo::from_bpm(bpm.round()));
        }
    }

    // ------------------------------------------------------------ bottom row

    fn bottom_row(&mut self, ui: &mut Ui, rect: Rect) {
        let cy = rect.center().y;
        let painter = ui.painter().clone();

        // Right side first: Play, then TAP.
        let play = Rect::from_center_size(pos2(rect.right() - 26.0, cy), vec2(52.0, 52.0));
        let tap_text = text(
            &painter,
            &self.i18n.tr("tap"),
            theme::bold(13.0),
            self.p.text,
            1.0,
        );
        let tap_w = tap_text.size().x + 28.0;
        let tap = Rect::from_min_size(
            pos2(play.left() - ROW_GAP - tap_w, cy - 22.0),
            vec2(tap_w, 44.0),
        );

        // Left side: meter chip (gets the remaining width), subdivision chip, presets, volume.
        let sub_label = self
            .i18n
            .tr(&format!("subdivision-{}", self.player.subdivision().id()));
        let sub_text = text(&painter, &sub_label, theme::medium(13.0), self.p.text, 0.0);
        let sub_w = chip_width(&sub_text);
        let fixed_right = ROW_GAP + sub_w + ROW_GAP + 36.0 + ROW_GAP + 36.0 + ROW_GAP;
        let meter_max = (tap.left() - rect.left() - fixed_right - CHIP_EXTRA).max(24.0);
        let (meter_label, meter_full) = meter_label(self.player.meter(), self.rhythms, self.i18n);
        let meter_text = text_elided(
            &painter,
            &meter_label,
            theme::bold(13.0),
            self.p.text,
            0.0,
            meter_max,
        );

        let mut x = rect.left();
        let meter_rect =
            Rect::from_min_size(pos2(x, cy - 18.0), vec2(chip_width(&meter_text), 36.0));
        x = meter_rect.right() + ROW_GAP;
        let sub_rect = Rect::from_min_size(pos2(x, cy - 18.0), vec2(sub_w, 36.0));
        x = sub_rect.right() + ROW_GAP;
        let presets_rect = Rect::from_min_size(pos2(x, cy - 18.0), vec2(36.0, 36.0));
        x = presets_rect.right() + ROW_GAP;
        let volume_rect = Rect::from_min_size(pos2(x, cy - 18.0), vec2(36.0, 36.0));

        let meter_resp = self.chip(ui, meter_rect, meter_text, "meter", &meter_full);
        let meter_resp = if meter_full == meter_label {
            meter_resp
        } else {
            meter_resp.on_hover_text(&meter_full)
        };
        self.meter_menu(&meter_resp);
        let sub_resp = self.chip(ui, sub_rect, sub_text, "subdivision", &sub_label);
        self.subdivision_menu(&sub_resp);

        // Presets arrive in stage 5: shown, but inactive.
        let presets = ui.interact(presets_rect, Id::unique("presets"), Sense::hover());
        painter.rect_filled(presets_rect, 8.0, self.p.surface);
        icons::paint(
            &painter,
            icons::crisp_center(&painter, presets_rect),
            16.0,
            Icon::Presets,
            self.p.text2.gamma_multiply(0.6),
            2.0,
        );
        let label = self.i18n.tr("presets");
        presets.widget_info(|| WidgetInfo::labeled(Role::Button, false, &label));
        presets.on_hover_text(self.i18n.tr("presets-later"));

        let muted = self.player.volume() <= 0.0;
        let icon = if muted {
            Icon::VolumeMuted
        } else {
            Icon::Volume
        };
        let label = self.i18n.tr("volume");
        let volume = self.square_icon(ui, volume_rect, icon, &label);
        self.volume_popup(&volume);

        let tap_resp = ui
            .interact(tap, Id::unique("tap"), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
        // Tap on press, not release: the press is the musical moment.
        if tap_resp.contains_pointer() && ui.input(|i| i.pointer.primary_pressed()) {
            self.player.tap(self.now);
        }
        if tap_resp.hovered() {
            painter.rect_filled(tap, 22.0, self.p.surface);
        }
        painter.rect_stroke(
            tap,
            22.0,
            Stroke::new(1.5, self.p.border2),
            StrokeKind::Inside,
        );
        painter.galley(tap.center() - tap_text.size() / 2.0, tap_text, self.p.text);
        let label = self.i18n.tr("tap-hint");
        tap_resp.widget_info(|| WidgetInfo::labeled(Role::Button, true, &label));

        let playing = self.player.is_playing();
        let play_resp = ui
            .interact(play, Id::unique("play"), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
        let fill = if play_resp.hovered() {
            theme::mix(ACCENT, Color32::WHITE, 0.12)
        } else {
            ACCENT
        };
        painter.circle_filled(play.center(), 26.0, fill);
        let (icon, size, label) = if playing {
            (Icon::Stop, 18.0, self.i18n.tr("stop"))
        } else {
            (Icon::Play, 20.0, self.i18n.tr("play"))
        };
        icons::paint(&painter, play.center(), size, icon, ON_ACCENT, 2.0);
        play_resp.widget_info(|| WidgetInfo::labeled(Role::Button, true, &label));
        if play_resp.clicked() {
            self.player.toggle();
        }
    }

    fn meter_menu(&mut self, resp: &Response) {
        enum Pick {
            Meter(Meter),
            Rhythm(usize),
        }
        let current = self.player.meter().clone();
        let mut pick = None;
        let (i18n, rhythms, p) = (self.i18n, self.rhythms, self.p);
        Popup::menu(resp).show(|ui| {
            ui.set_min_width(150.0);
            ui.label(egui::RichText::new(i18n.tr("meter-section-meters")).color(p.text2));
            for (label, meter) in meter_presets() {
                if ui.selectable_label(meter == current, label).clicked() {
                    pick = Some(Pick::Meter(meter));
                }
            }
            ui.separator();
            for category in [RhythmCategory::Latin, RhythmCategory::Flamenco] {
                let title = i18n.tr(&format!("rhythm-category-{}", category.id()));
                ui.menu_button(title, |ui| {
                    ui.set_min_width(150.0);
                    for (i, r) in rhythms.iter().enumerate() {
                        if r.category != category {
                            continue;
                        }
                        let name = i18n.tr_or(&format!("rhythm-{}", r.id), &r.name);
                        if ui.selectable_label(r.meter == current, name).clicked() {
                            pick = Some(Pick::Rhythm(i));
                        }
                    }
                });
            }
        });
        match pick {
            Some(Pick::Meter(m)) => self.player.set_meter(m),
            Some(Pick::Rhythm(i)) => self.player.set_rhythm(&self.rhythms[i]),
            None => {}
        }
    }

    fn subdivision_menu(&mut self, resp: &Response) {
        let current = self.player.subdivision();
        let mut pick = None;
        let i18n = self.i18n;
        Popup::menu(resp).show(|ui| {
            ui.set_min_width(140.0);
            for s in Subdivision::ALL {
                let label = i18n.tr(&format!("subdivision-{}", s.id()));
                if ui.selectable_label(s == current, label).clicked() {
                    pick = Some(s);
                }
            }
        });
        if let Some(s) = pick {
            self.player.set_subdivision(s);
        }
    }

    fn volume_popup(&mut self, resp: &Response) {
        let volume = self.player.volume();
        let mut new = None;
        let (i18n, p) = (self.i18n, self.p);
        Popup::from_toggle_button_response(resp)
            .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                let (rect, _) = ui.allocate_exact_size(vec2(170.0, 28.0), Sense::hover());
                let slider_rect =
                    Rect::from_min_max(rect.min, pos2(rect.right() - 40.0, rect.max.y));
                let label = i18n.tr("volume");
                new = slider(
                    ui,
                    slider_rect,
                    Id::unique("volume-slider"),
                    volume,
                    &label,
                    f64::from(volume),
                    &p,
                );
                let shown = new.unwrap_or(volume);
                let value = if shown <= 0.0 {
                    i18n.tr("volume-muted")
                } else {
                    format!("{:.0}%", shown * 100.0)
                };
                ui.painter().text(
                    pos2(rect.right(), rect.center().y),
                    Align2::RIGHT_CENTER,
                    value,
                    theme::medium(12.0),
                    p.text2,
                );
            });
        if let Some(v) = new {
            self.player.set_volume(v);
        }
    }

    // ------------------------------------------------------------ widgets

    fn round_icon(&self, ui: &Ui, rect: Rect, icon: Icon, label: &str) -> Response {
        let resp = ui
            .interact(rect, Id::unique(("round", label)), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
        let fill = self.button_fill(&resp);
        let painter = ui.painter();
        painter.circle_filled(rect.center(), rect.width() / 2.0, fill);
        icons::paint(
            painter,
            icons::crisp_center(painter, rect),
            18.0,
            icon,
            self.p.text,
            2.2,
        );
        resp.widget_info(|| WidgetInfo::labeled(Role::Button, true, label));
        resp
    }

    fn square_icon(&self, ui: &Ui, rect: Rect, icon: Icon, label: &str) -> Response {
        let resp = ui
            .interact(rect, Id::unique(("square", label)), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
        let fill = self.button_fill(&resp);
        let painter = ui.painter();
        painter.rect_filled(rect, 8.0, fill);
        icons::paint(
            painter,
            icons::crisp_center(painter, rect),
            16.0,
            icon,
            self.p.text,
            2.0,
        );
        resp.widget_info(|| WidgetInfo::labeled(Role::Button, true, label));
        resp
    }

    fn chip(&self, ui: &Ui, rect: Rect, galley: Arc<Galley>, id: &str, value: &str) -> Response {
        let resp = ui
            .interact(rect, Id::unique(("chip", id)), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
        let fill = if Popup::is_id_open(ui.ctx(), Popup::default_response_id(&resp)) {
            self.p.surface_active()
        } else {
            self.button_fill(&resp)
        };
        let painter = ui.painter();
        painter.rect_filled(rect, 8.0, fill);
        let text_pos = pos2(rect.left() + 10.0, rect.center().y - galley.size().y / 2.0);
        painter.galley(text_pos, galley, self.p.text);
        let chevron = pos2(rect.right() - 10.0 - 6.0, rect.center().y);
        icons::paint(painter, chevron, 12.0, Icon::Chevron, self.p.text, 2.4);
        let name = self.i18n.tr(id);
        resp.widget_info(|| {
            let mut info = WidgetInfo::labeled(Role::ComboBox, true, &name);
            info.current_text_value = Some(value.to_owned());
            info
        });
        resp
    }

    fn button_fill(&self, resp: &Response) -> Color32 {
        if resp.is_pointer_button_down_on() {
            self.p.surface_active()
        } else if resp.hovered() {
            self.p.surface_hover()
        } else {
            self.p.surface
        }
    }
}

/// Chip padding + gap + chevron, beside the text.
const CHIP_EXTRA: f32 = 10.0 + 4.0 + 12.0 + 10.0;

fn chip_width(text: &Galley) -> f32 {
    text.size().x + CHIP_EXTRA
}

/// A horizontal slider as in the mockup (accent fill, round thumb).
/// Returns the new position 0…1 while the user drags or clicks it.
fn slider(
    ui: &Ui,
    rect: Rect,
    id: Id,
    t: f32,
    label: &str,
    value: f64,
    p: &Palette,
) -> Option<f32> {
    const THUMB_R: f32 = 8.0;
    let resp = ui
        .interact(rect, id, Sense::click_and_drag())
        .on_hover_cursor(CursorIcon::PointingHand);
    let track = rect.shrink2(vec2(THUMB_R, 0.0));
    let mut new = None;
    if (resp.dragged() || resp.is_pointer_button_down_on())
        && let Some(pos) = resp.interact_pointer_pos()
    {
        new = Some(((pos.x - track.left()) / track.width()).clamp(0.0, 1.0));
    }
    let t = new.unwrap_or(t);
    let painter = ui.painter();
    let cy = rect.center().y;
    let x = track.left() + t * track.width();
    let bar = |a: f32, b: f32, c: Color32| {
        painter.rect_filled(
            Rect::from_min_max(pos2(a, cy - 2.0), pos2(b, cy + 2.0)),
            2.0,
            c,
        );
    };
    bar(track.left(), track.right(), p.border2);
    bar(track.left(), x, ACCENT);
    let r = if resp.hovered() || resp.dragged() {
        THUMB_R + 1.0
    } else {
        THUMB_R
    };
    painter.circle_filled(pos2(x, cy), r, ACCENT);
    resp.widget_info(|| WidgetInfo::slider(true, value, label));
    new
}

fn text(
    painter: &egui::Painter,
    s: &str,
    font: FontId,
    color: Color32,
    spacing: f32,
) -> Arc<Galley> {
    painter.layout_job(LayoutJob::single_section(
        s.to_owned(),
        TextFormat {
            font_id: font,
            color,
            extra_letter_spacing: spacing,
            ..Default::default()
        },
    ))
}

/// One line of text, cut with "…" to `max_width`.
fn text_elided(
    painter: &egui::Painter,
    s: &str,
    font: FontId,
    color: Color32,
    spacing: f32,
    max_width: f32,
) -> Arc<Galley> {
    let mut job = LayoutJob::single_section(
        s.to_owned(),
        TextFormat {
            font_id: font,
            color,
            extra_letter_spacing: spacing,
            ..Default::default()
        },
    );
    job.wrap = TextWrapping {
        max_width,
        max_rows: 1,
        break_anywhere: true,
        overflow_character: Some('…'),
    };
    painter.layout_job(job)
}

/// Baseline of the first row, from the galley top.
fn baseline_of(galley: &Galley) -> f32 {
    galley
        .rows
        .first()
        .and_then(|r| r.row.glyphs.first().map(|g| r.pos.y + g.pos.y))
        .unwrap_or(galley.size().y * 0.8)
}

/// "1:23", or "1:02:03" past an hour.
fn format_elapsed(d: Duration) -> String {
    let s = d.as_secs();
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

/// The meter presets of the menu (keys 1–8 in stage 4b).
fn meter_presets() -> Vec<(&'static str, Meter)> {
    let formula = |f| Meter::from_formula(f).expect("valid preset");
    let grid = |s, g| Meter::from_grid(s, g).expect("valid preset");
    vec![
        ("1/4", formula("1")),
        ("2/4", formula("2")),
        ("3/4", formula("3")),
        ("4/4", formula("4")),
        ("3+2", formula("3+2")),
        // Cut bars at the quarter-note tempo: 5/8 as 3+2, 7/8 as 2+2+3.
        ("5/8", grid(2, "Xx xX x")),
        ("7/8", grid(2, "Xx Xx Xx x")),
    ]
}

/// Chip text for a meter (short, full): the rhythm name, the grouping, or
/// "N/denominator".
fn meter_label(meter: &Meter, rhythms: &[Rhythm], i18n: &I18n) -> (String, String) {
    if let Some(r) = rhythms.iter().find(|r| r.meter == *meter) {
        let id = format!("rhythm-{}", r.id);
        let full = i18n.tr_or(&id, &r.name);
        return (
            i18n.attr(&id, "short").unwrap_or_else(|| full.clone()),
            full,
        );
    }
    let label = plain_meter_label(meter);
    (label.clone(), label)
}

fn plain_meter_label(meter: &Meter) -> String {
    if let Some((label, _)) = meter_presets().into_iter().find(|(_, m)| m == meter) {
        return label.to_owned();
    }
    let n = meter.len();
    match meter.subunits() {
        1 => {
            let groups = meter.label();
            if groups.contains('+') {
                groups
            } else {
                format!("{n}/4")
            }
        }
        2 | 3 => format!("{n}/8"),
        4 => format!("{n}/16"),
        s => format!("{n} × 1/{s}"),
    }
}

fn error_text(i18n: &I18n, error: &PlayerError) -> String {
    match error {
        PlayerError::NotResponding => i18n.tr("audio-error-not-responding"),
        PlayerError::Audio(e) => match e {
            AudioError::NoDevice => i18n.tr("audio-error-no-device"),
            AudioError::DeviceNotFound(name) => i18n.tr_args(
                "audio-error-device-not-found",
                &[("name", name.as_str().into())],
            ),
            AudioError::Config(d) => {
                i18n.tr_args("audio-error-config", &[("detail", d.as_str().into())])
            }
            AudioError::UnsupportedFormat(d) => {
                i18n.tr_args("audio-error-format", &[("detail", d.as_str().into())])
            }
            AudioError::Stream(d) => {
                i18n.tr_args("audio-error-stream", &[("detail", d.as_str().into())])
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elapsed_format() {
        assert_eq!(format_elapsed(Duration::from_secs(0)), "0:00");
        assert_eq!(format_elapsed(Duration::from_secs(83)), "1:23");
        assert_eq!(format_elapsed(Duration::from_secs(3723)), "1:02:03");
    }

    #[test]
    fn meter_labels() {
        let i18n = I18n::english();
        let rhythms = Rhythm::builtin();
        let label = |m: &Meter| meter_label(m, &rhythms, &i18n).0;
        assert_eq!(label(&Meter::default()), "4/4");
        assert_eq!(label(&Meter::from_formula("2+2+3").unwrap()), "2+2+3");
        assert_eq!(label(&Meter::from_grid(2, "Xx xX x").unwrap()), "5/8");
        assert_eq!(label(&Meter::from_grid(3, "Xxx xxx").unwrap()), "6/8");
        assert_eq!(label(&rhythms[0].meter), "Son 3-2");
        assert_eq!(
            meter_label(&rhythms[0].meter, &rhythms, &i18n).1,
            "Son clave 3-2"
        );
        let mut m = Meter::default();
        m.set_level(1, BeatLevel::Mute);
        assert_eq!(label(&m), "4/4");
    }

    #[test]
    fn presets_are_distinct() {
        let presets = meter_presets();
        for (i, (_, a)) in presets.iter().enumerate() {
            for (_, b) in &presets[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }
}
