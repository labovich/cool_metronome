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
use egui::text::{CCursor, CCursorRange};
use egui::text::{LayoutJob, TextFormat, TextWrapping};
use egui::{
    Align2, Color32, CursorIcon, FontId, Galley, Id, Key, Popup, PopupCloseBehavior, Rect,
    Response, Role, Sense, Stroke, StrokeKind, TextEdit, Ui, WidgetInfo, pos2, vec2,
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

/// Longest tempo text that can be typed ("999.9").
const TEMPO_CHARS: usize = 5;

/// Typed tempo entry, shown over the BPM number after a double-click.
#[derive(Debug)]
pub struct TempoEdit {
    text: String,
    /// Focus the field and select all on the next frame.
    opening: bool,
    /// The last Enter had an invalid value.
    invalid: bool,
}

impl TempoEdit {
    fn new(tempo: Tempo) -> Self {
        Self {
            text: tempo.to_string(),
            opening: true,
            invalid: false,
        }
    }
}

/// A request from the window chrome (title bar, compact strip).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowAction {
    /// Start moving the window with the mouse.
    Drag,
    /// Switch between the full window and the compact strip.
    ToggleCompact,
    /// Toggle "always on top".
    TogglePin,
    Minimize,
    Close,
}

/// Title bar height (mockup).
pub const TITLE_H: f32 = 36.0;

/// What the main view needs for one frame.
pub struct View<'a> {
    pub player: &'a mut Player,
    pub i18n: &'a I18n,
    pub rhythms: &'a [Rhythm],
    pub tempo_edit: &'a mut Option<TempoEdit>,
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

        let block_h = 64.0 + 2.0 + 15.0;
        let line_top = rect.top() + (rect.height() - block_h) / 2.0;
        let tempo = self.player.tempo();
        if self.tempo_edit.is_some() {
            self.tempo_field(ui, rect, line_top);
        } else {
            self.tempo_number(ui, rect, line_top, tempo);
        }

        let (line, color) = if let Some(edit) = &self.tempo_edit {
            if edit.invalid {
                (self.i18n.tr("tempo-edit-invalid"), ACCENT)
            } else {
                (self.i18n.tr("tempo-edit-hint"), self.p.text2)
            }
        } else if let Some(e) = self.player.error() {
            (error_text(self.i18n, e), ACCENT)
        } else {
            let marking = self.i18n.tr(&format!("tempo-{}", tempo.marking().id()));
            let elapsed = format_elapsed(self.player.elapsed(self.now));
            let line = self.i18n.tr_args(
                "tempo-subtitle",
                &[("marking", marking.into()), ("elapsed", elapsed.into())],
            );
            (line, self.p.text2)
        };
        let painter = ui.painter();
        let max_w = plus.left() - minus.right() - 8.0;
        let sub = text_elided(painter, &line, theme::medium(12.0), color, 0.4, max_w);
        let pos = pos2(rect.center().x - sub.size().x / 2.0, line_top + 66.0);
        painter.galley(pos, sub, color);
    }

    /// The big BPM number: whole part 64 pt, ".tenths" 20 pt, baselines
    /// aligned. A double-click opens [`TempoEdit`].
    fn tempo_number(&mut self, ui: &mut Ui, rect: Rect, line_top: f32, tempo: Tempo) {
        let painter = ui.painter();
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
        // CSS line box of 64 px: the Inter baseline sits 55 px below its top.
        let baseline = line_top + 55.25;
        let width = whole.size().x + 6.0 + frac.size().x;
        let x = rect.center().x - width / 2.0;
        let number = Rect::from_min_size(pos2(x, line_top), vec2(width, 64.0));
        let label = self.i18n.tr("tempo-edit-tooltip");
        let resp = ui
            .interact(number, Id::unique("tempo-number"), Sense::click())
            .on_hover_cursor(CursorIcon::Text)
            .on_hover_text(&label);
        resp.widget_info(|| {
            let mut info = WidgetInfo::labeled(Role::Button, true, &label);
            info.current_text_value = Some(tempo.to_string());
            info
        });
        let painter = ui.painter();
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
        if resp.double_clicked() {
            *self.tempo_edit = Some(TempoEdit::new(tempo));
        }
    }

    /// The tempo text field: Enter applies, Esc cancels, clicking elsewhere
    /// applies a valid value and drops an invalid one.
    fn tempo_field(&mut self, ui: &mut Ui, rect: Rect, line_top: f32) {
        let Some(edit) = self.tempo_edit.as_mut() else {
            return;
        };
        let field =
            Rect::from_center_size(pos2(rect.center().x, line_top + 32.0), vec2(168.0, 64.0));
        let border = if edit.invalid { ACCENT } else { self.p.border2 };
        ui.painter().rect_filled(field, 12.0, self.p.surface);
        ui.painter()
            .rect_stroke(field, 12.0, Stroke::new(1.5, border), StrokeKind::Inside);

        let id = Id::unique("tempo-field");
        let resp = ui.put(
            field.shrink2(vec2(10.0, 4.0)),
            TextEdit::singleline(&mut edit.text)
                .id(id)
                .font(theme::bold(52.0))
                .text_color(self.p.text)
                .align(Align2::CENTER_CENTER)
                .frame(egui::Frame::NONE)
                .char_limit(TEMPO_CHARS)
                .desired_width(field.width() - 20.0),
        );
        let label = self.i18n.tr("tempo-slider");
        resp.widget_info(|| WidgetInfo::text_edit(true, "", &edit.text, &label));

        if edit.opening {
            edit.opening = false;
            resp.request_focus();
            if let Some(mut state) = TextEdit::load_state(ui.ctx(), id) {
                let end = CCursor::new(edit.text.chars().count());
                state
                    .cursor
                    .set_char_range(Some(CCursorRange::two(CCursor::new(0), end)));
                state.store(ui.ctx(), id);
            }
        }
        if resp.changed() {
            edit.text
                .retain(|c| c.is_ascii_digit() || c == '.' || c == ',');
            edit.invalid = false;
        }
        if !resp.lost_focus() {
            return;
        }
        let (enter, escape) = ui.input(|i| (i.key_pressed(Key::Enter), i.key_pressed(Key::Escape)));
        let parsed = parse_tempo(&edit.text);
        if escape {
            *self.tempo_edit = None;
        } else if enter && parsed.is_none() {
            edit.invalid = true;
            resp.request_focus();
        } else {
            *self.tempo_edit = None;
            if let Some(tempo) = parsed {
                self.player.set_tempo(tempo);
            }
        }
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

        self.play_button(ui, play, 18.0, 20.0);
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

    // ------------------------------------------------------------ window chrome

    /// The custom title bar: logo, name, pin, tray, minimize, close.
    /// Dragging moves the window, a double-click switches to compact mode.
    pub fn title_bar(&mut self, ui: &mut Ui, rect: Rect, pinned: bool) -> Option<WindowAction> {
        let mut action = None;
        // The drag area goes first so the buttons on top of it win.
        let bar = ui.interact(rect, Id::unique("title-bar"), Sense::click_and_drag());
        if bar.double_clicked() {
            action = Some(WindowAction::ToggleCompact);
        } else if bar.drag_started() {
            action = Some(WindowAction::Drag);
        }
        // The hint is for the empty part of the bar, not for its buttons.
        let buttons_left = rect.right() - 4.0 - 4.0 * (32.0 + 2.0);
        if bar.hover_pos().is_some_and(|p| p.x < buttons_left) {
            let hint = self.i18n.tr("window-compact-hint");
            bar.on_hover_text_at_pointer(hint);
        }

        let painter = ui.painter();
        let cy = rect.center().y;
        icons::paint(
            painter,
            pos2(rect.left() + 14.0 + 8.0, cy),
            16.0,
            Icon::Logo,
            ACCENT,
            2.0,
        );
        let title = text(
            painter,
            &self.i18n.tr("app-title"),
            theme::bold(13.0),
            self.p.text2,
            0.0,
        );
        painter.galley(
            pos2(rect.left() + 14.0 + 16.0 + 8.0, cy - title.size().y / 2.0),
            title,
            self.p.text2,
        );

        // Buttons from the right: close, minimize, tray, pin (32×28, 2 apart).
        let button = |i: f32| {
            let right = rect.right() - 4.0 - i * (32.0 + 2.0);
            Rect::from_min_size(pos2(right - 32.0, cy - 14.0), vec2(32.0, 28.0))
        };
        let label = self.i18n.tr("window-close");
        if self
            .bar_button(ui, button(0.0), Icon::Close, &label, false, true)
            .clicked()
        {
            action = Some(WindowAction::Close);
        }
        let label = self.i18n.tr("window-minimize");
        if self
            .bar_button(ui, button(1.0), Icon::Minus, &label, false, true)
            .clicked()
        {
            action = Some(WindowAction::Minimize);
        }
        // Tray arrives in stage 6: shown, but inactive.
        let label = self.i18n.tr("window-tray");
        self.bar_button(ui, button(2.0), Icon::Tray, &label, false, false)
            .on_hover_text(self.i18n.tr("window-tray-later"));
        let label = self.i18n.tr("window-pin");
        if self
            .bar_button(ui, button(3.0), Icon::Pin, &label, pinned, true)
            .clicked()
        {
            action = Some(WindowAction::TogglePin);
        }
        action
    }

    /// The compact strip (440×96): tempo ±, beats, Play, expand, close.
    /// The background drags the window; a double-click expands it.
    pub fn show_compact(&mut self, ui: &mut Ui, rect: Rect) -> Option<WindowAction> {
        let mut action = None;
        let bg = ui.interact(rect, Id::unique("compact-bg"), Sense::click_and_drag());
        if bg.double_clicked() {
            action = Some(WindowAction::ToggleCompact);
        } else if bg.drag_started() {
            action = Some(WindowAction::Drag);
        }
        let cy = rect.center().y;

        // Left: − BPM +.
        let mut x = rect.left() + 14.0;
        let minus = Rect::from_min_size(pos2(x, cy - 16.0), vec2(32.0, 32.0));
        x = minus.right() + 6.0;
        let number = Rect::from_min_size(pos2(x, cy - 24.0), vec2(76.0, 48.0));
        x = number.right() + 6.0;
        let plus = Rect::from_min_size(pos2(x, cy - 16.0), vec2(32.0, 32.0));
        let step = if ui.input(|i| i.modifiers.shift) {
            0.1
        } else {
            1.0
        };
        let label = self.i18n.tr("tempo-slower");
        if self
            .circle_icon(ui, minus, Icon::Minus, 14.0, 2.4, &label)
            .clicked()
        {
            self.player.set_tempo(self.player.tempo().offset(-step));
        }
        let label = self.i18n.tr("tempo-faster");
        if self
            .circle_icon(ui, plus, Icon::Plus, 14.0, 2.4, &label)
            .clicked()
        {
            self.player.set_tempo(self.player.tempo().offset(step));
        }
        let tempo = self.player.tempo();
        let painter = ui.painter();
        let bpm_text = tempo.to_string();
        // Tenths make the number longer: shrink it to stay within 76 pt.
        let size = if bpm_text.len() > 3 { 28.0 } else { 34.0 };
        let bpm = text(painter, &bpm_text, theme::bold(size), self.p.text, -1.0);
        let (meter, _) = meter_label(self.player.meter(), self.rhythms, self.i18n);
        let sub = text_elided(
            painter,
            &format!("BPM · {meter}"),
            theme::medium(11.0),
            self.p.text2,
            0.4,
            number.width() + 24.0,
        );
        let top = cy - (36.0 + sub.size().y) / 2.0;
        painter.galley(
            pos2(
                number.center().x - bpm.size().x / 2.0,
                top + (36.0 - bpm.size().y) / 2.0,
            ),
            bpm,
            self.p.text,
        );
        painter.galley(
            pos2(number.center().x - sub.size().x / 2.0, top + 36.0),
            sub,
            self.p.text2,
        );

        // Right: expand and close stacked, then Play.
        let col_x = rect.right() - 6.0 - 30.0;
        let expand = Rect::from_min_size(pos2(col_x, cy - 32.0), vec2(30.0, 30.0));
        let close = Rect::from_min_size(pos2(col_x, cy + 2.0), vec2(30.0, 30.0));
        let label = self.i18n.tr("window-expand");
        if self
            .bar_button(ui, expand, Icon::Expand, &label, false, true)
            .clicked()
        {
            action = Some(WindowAction::ToggleCompact);
        }
        let label = self.i18n.tr("window-close");
        if self
            .bar_button(ui, close, Icon::Close, &label, false, true)
            .clicked()
        {
            action = Some(WindowAction::Close);
        }
        let play = Rect::from_center_size(pos2(col_x - 14.0 - 24.0, cy), vec2(48.0, 48.0));
        self.play_button(ui, play, 16.0, 18.0);

        // Middle: the beats (not clickable here).
        let beats = Rect::from_min_max(
            pos2(plus.right() + 14.0, rect.top()),
            pos2(play.left() - 14.0, rect.bottom()),
        );
        self.compact_beats(ui, beats);
        action
    }

    fn compact_beats(&self, ui: &Ui, rect: Rect) {
        const GAP: f32 = 8.0;
        let cells = self.player.meter().cells();
        let current = self.player.audible_cell(self.now);
        let playing = self.player.is_playing();
        let size = |l: &BeatLevel| if *l == BeatLevel::Accent { 18.0 } else { 14.0 };
        let natural: f32 =
            cells.iter().map(size).sum::<f32>() + GAP * (cells.len().saturating_sub(1)) as f32;
        let scale = ((rect.width() - 4.0) / natural).min(1.0);
        let painter = ui.painter();
        let mut x = rect.center().x - natural * scale / 2.0;
        for (i, level) in cells.iter().enumerate() {
            let d = size(level) * scale;
            let center = pos2(x + d / 2.0, rect.center().y);
            let on = current == Some(i);
            let r = d / 2.0 * if on { 1.2 } else { 1.0 };
            // While playing, the other beats dim so the current one stands out.
            let dim = |c: Color32| {
                if playing && !on {
                    c.gamma_multiply(0.45)
                } else {
                    c
                }
            };
            match level {
                BeatLevel::Accent => painter.circle_filled(center, r, dim(ACCENT)),
                BeatLevel::Normal => painter.circle_filled(center, r, dim(self.p.beat)),
                BeatLevel::Mute => painter.circle_stroke(
                    center,
                    r - scale,
                    Stroke::new(1.5 * scale, dim(self.p.border2)),
                ),
            };
            x += d + GAP * scale;
        }
    }

    /// A transparent title-bar style button (32×28 or 30×30).
    fn bar_button(
        &self,
        ui: &Ui,
        rect: Rect,
        icon: Icon,
        label: &str,
        active: bool,
        enabled: bool,
    ) -> Response {
        let sense = if enabled {
            Sense::click()
        } else {
            Sense::hover()
        };
        let mut resp = ui.interact(rect, Id::unique(("bar", label)), sense);
        if enabled {
            resp = resp.on_hover_cursor(CursorIcon::PointingHand);
        }
        let painter = ui.painter();
        let (fill, color) = if active {
            let fill = theme::mix(self.p.surface, ACCENT, 0.22);
            (Some(fill), ACCENT)
        } else if enabled && resp.is_pointer_button_down_on() {
            (Some(self.p.surface_active()), self.p.text)
        } else if enabled && resp.hovered() {
            (Some(self.p.surface), self.p.text)
        } else if enabled {
            (None, self.p.text2)
        } else {
            (None, self.p.text2.gamma_multiply(0.5))
        };
        if let Some(fill) = fill {
            painter.rect_filled(rect, 6.0, fill);
        }
        icons::paint(
            painter,
            icons::crisp_center(painter, rect),
            15.0,
            icon,
            color,
            2.0,
        );
        resp.widget_info(|| {
            let mut info = WidgetInfo::labeled(Role::Button, enabled, label);
            if icon == Icon::Pin {
                info.selected = Some(active);
            }
            info
        });
        resp
    }

    // ------------------------------------------------------------ widgets

    fn round_icon(&self, ui: &Ui, rect: Rect, icon: Icon, label: &str) -> Response {
        self.circle_icon(ui, rect, icon, 18.0, 2.2, label)
    }

    fn circle_icon(
        &self,
        ui: &Ui,
        rect: Rect,
        icon: Icon,
        size: f32,
        stroke: f32,
        label: &str,
    ) -> Response {
        let resp = ui
            .interact(rect, Id::unique(("round", label)), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
        let fill = self.button_fill(&resp);
        let painter = ui.painter();
        painter.circle_filled(rect.center(), rect.width() / 2.0, fill);
        icons::paint(
            painter,
            icons::crisp_center(painter, rect),
            size,
            icon,
            self.p.text,
            stroke,
        );
        resp.widget_info(|| WidgetInfo::labeled(Role::Button, true, label));
        resp
    }

    /// The accent Play/Stop button; toggles playback when clicked.
    fn play_button(&mut self, ui: &Ui, rect: Rect, stop_size: f32, play_size: f32) {
        let playing = self.player.is_playing();
        let resp = ui
            .interact(rect, Id::unique("play"), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
        let fill = if resp.hovered() {
            theme::mix(ACCENT, Color32::WHITE, 0.12)
        } else {
            ACCENT
        };
        let painter = ui.painter();
        painter.circle_filled(rect.center(), rect.width() / 2.0, fill);
        let (icon, size, label) = if playing {
            (Icon::Stop, stop_size, self.i18n.tr("stop"))
        } else {
            (Icon::Play, play_size, self.i18n.tr("play"))
        };
        icons::paint(painter, rect.center(), size, icon, ON_ACCENT, 2.0);
        resp.widget_info(|| WidgetInfo::labeled(Role::Button, true, &label));
        if resp.clicked() {
            self.player.toggle();
        }
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

/// A typed tempo: 1-999 BPM, "." or "," as the decimal separator,
/// rounded to 0.1. `None` for anything else.
fn parse_tempo(text: &str) -> Option<Tempo> {
    let bpm: f64 = text.trim().replace(',', ".").parse().ok()?;
    let (min, max) = (Tempo::MIN.bpm(), Tempo::MAX.bpm());
    (bpm.is_finite() && (min..=max).contains(&bpm)).then(|| Tempo::from_bpm(bpm))
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
    fn typed_tempo() {
        let t = |s| parse_tempo(s).map(Tempo::bpm);
        assert_eq!(t("120"), Some(120.0));
        assert_eq!(t(" 96.5 "), Some(96.5));
        assert_eq!(t("96,5"), Some(96.5));
        assert_eq!(t("133.37"), Some(133.4));
        assert_eq!(t("1"), Some(1.0));
        assert_eq!(t("999"), Some(999.0));
        assert_eq!(t(".5"), None, "below 1 BPM");
        assert_eq!(t("0"), None);
        assert_eq!(t("1000"), None);
        assert_eq!(t(""), None);
        assert_eq!(t("1.2.3"), None);
        assert_eq!(t("NaN"), None);
    }

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
