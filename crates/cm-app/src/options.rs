//! The options window: a separate OS window with tabs (Interface, Audio,
//! Volume). It edits the settings in place; the app applies the changes.

use crate::i18n::I18n;
use crate::player::Player;
use crate::settings::{AudioSettings, InterfaceSettings, ThemeChoice, VolumeSettings};
use crate::theme::{self, Palette};
use egui::{ComboBox, DragValue, Grid, Key, RichText, Slider, Ui, ViewportBuilder, ViewportId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Tab {
    #[default]
    Interface,
    Audio,
    Volume,
}

impl Tab {
    const ALL: [Tab; 3] = [Self::Interface, Self::Audio, Self::Volume];

    fn id(self) -> &'static str {
        match self {
            Self::Interface => "options-tab-interface",
            Self::Audio => "options-tab-audio",
            Self::Volume => "options-tab-volume",
        }
    }
}

/// The settings the window edits.
pub struct Editable<'a> {
    pub interface: &'a mut InterfaceSettings,
    pub audio: &'a mut AudioSettings,
    pub volume: &'a mut VolumeSettings,
}

#[derive(Debug, Default)]
pub struct OptionsWindow {
    pub open: bool,
    tab: Tab,
    /// Output device names, listed when the Audio tab is first shown.
    devices: Option<Vec<String>>,
}

impl OptionsWindow {
    /// Shows the window while it is open. `on_top` follows the main window.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        mut edit: Editable<'_>,
        player: &Player,
        i18n: &I18n,
        on_top: bool,
    ) {
        if !self.open {
            self.devices = None;
            return;
        }
        let title = i18n.tr("options-title");
        let mut builder = ViewportBuilder::default()
            .with_title(&title)
            .with_inner_size([460.0, 340.0])
            .with_min_inner_size([420.0, 300.0]);
        if on_top {
            builder = builder.with_window_level(egui::WindowLevel::AlwaysOnTop);
        }
        ctx.show_viewport_immediate(ViewportId::from_hash_of("options"), builder, |ui, _| {
            let close = ui
                .ctx()
                .input(|i| i.viewport().close_requested() || i.key_pressed(Key::Escape));
            if close {
                self.open = false;
            }
            let p = Palette::of(ui.ctx().theme());
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.fill(p.bg).inner_margin(16))
                .show(ui, |ui| {
                    self.tabs(ui, i18n, &p);
                    ui.add_space(10.0);
                    match self.tab {
                        Tab::Interface => interface_tab(ui, edit.interface, i18n),
                        Tab::Audio => self.audio_tab(ui, edit.audio, player, i18n, &p),
                        Tab::Volume => volume_tab(ui, &mut edit, i18n),
                    }
                });
        });
        if !self.open {
            // Let the main window drop the closed viewport.
            ctx.request_repaint();
        }
    }

    fn tabs(&mut self, ui: &mut Ui, i18n: &I18n, p: &Palette) {
        ui.horizontal(|ui| {
            for tab in Tab::ALL {
                let text = RichText::new(i18n.tr(tab.id())).font(theme::bold(13.0));
                let text = if tab == self.tab {
                    text.color(p.text)
                } else {
                    text.color(p.text2)
                };
                if ui.selectable_label(tab == self.tab, text).clicked() {
                    self.tab = tab;
                }
            }
        });
    }

    fn audio_tab(
        &mut self,
        ui: &mut Ui,
        audio: &mut AudioSettings,
        player: &Player,
        i18n: &I18n,
        p: &Palette,
    ) {
        let devices = self
            .devices
            .get_or_insert_with(cm_audio::output_device_names);
        let default_name = i18n.tr("options-device-default");
        let mut refresh = false;
        Grid::new("audio")
            .num_columns(2)
            .spacing([16.0, 12.0])
            .show(ui, |ui| {
                ui.label(i18n.tr("options-device"));
                ui.horizontal(|ui| {
                    let shown = audio.device.clone().unwrap_or_else(|| default_name.clone());
                    ComboBox::from_id_salt("device")
                        .selected_text(shown)
                        .width(230.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut audio.device, None, &default_name);
                            for name in devices.iter() {
                                ui.selectable_value(&mut audio.device, Some(name.clone()), name);
                            }
                        });
                    refresh = ui.button(i18n.tr("options-refresh")).clicked();
                });
                ui.end_row();

                ui.label(i18n.tr("options-now-playing"));
                match player.output_info() {
                    Some(info) => ui
                        .add(egui::Label::new(RichText::new(&info).color(p.text2)).truncate())
                        .on_hover_text(info),
                    None => {
                        ui.label(RichText::new(i18n.tr("options-no-output")).color(theme::ACCENT))
                    }
                };
                ui.end_row();
            });
        if player.on_fallback() {
            ui.add_space(8.0);
            ui.label(RichText::new(i18n.tr("options-device-missing")).color(theme::ACCENT));
        }
        if refresh {
            self.devices = None;
        }
    }
}

fn interface_tab(ui: &mut Ui, s: &mut InterfaceSettings, i18n: &I18n) {
    Grid::new("interface")
        .num_columns(2)
        .spacing([16.0, 12.0])
        .show(ui, |ui| {
            ui.label(i18n.tr("options-theme"));
            ui.horizontal(|ui| {
                let current = s.theme();
                for t in ThemeChoice::ALL {
                    let label = i18n.tr(&format!("theme-{}", t.id()));
                    if ui.selectable_label(current == t, label).clicked() {
                        s.theme = t.id().to_owned();
                    }
                }
            });
            ui.end_row();

            ui.label(i18n.tr("options-slider-range"));
            ui.horizontal(|ui| {
                let (mut min, mut max) = s.slider_range();
                let bpm = format!(" {}", i18n.tr("bpm"));
                ui.add(DragValue::new(&mut min).range(1.0..=998.0).speed(1.0));
                ui.label("–");
                ui.add(
                    DragValue::new(&mut max)
                        .range(2.0..=999.0)
                        .speed(1.0)
                        .suffix(bpm),
                );
                s.slider_min = min;
                s.slider_max = max.max(min + 1.0);
            });
            ui.end_row();

            ui.label(i18n.tr("options-quick-jump"));
            ui.horizontal(|ui| {
                ui.add(
                    DragValue::new(&mut s.quick_jump)
                        .range(0.1..=100.0)
                        .speed(0.1)
                        .max_decimals(1),
                );
                ui.selectable_value(&mut s.quick_jump_percent, false, i18n.tr("bpm"));
                ui.selectable_value(&mut s.quick_jump_percent, true, "%");
            });
            ui.end_row();

            ui.label(i18n.tr("options-tap"));
            ui.checkbox(&mut s.tap_fractional, i18n.tr("options-tap-fractional"));
            ui.end_row();
        });
    ui.add_space(14.0);
    if ui.button(i18n.tr("options-defaults")).clicked() {
        *s = InterfaceSettings::default();
    }
}

fn volume_tab(ui: &mut Ui, edit: &mut Editable<'_>, i18n: &I18n) {
    let v = &mut *edit.volume;
    Grid::new("volume")
        .num_columns(2)
        .spacing([16.0, 10.0])
        .show(ui, |ui| {
            let slots = [
                ("options-slot-accent", &mut v.accent_db),
                ("options-slot-normal", &mut v.normal_db),
                ("options-slot-sub", &mut v.sub_db),
                ("options-slot-bell", &mut v.bell_db),
            ];
            for (id, db) in slots {
                ui.label(i18n.tr(id));
                ui.add(
                    Slider::new(db, VolumeSettings::MIN_DB..=0.0)
                        .step_by(0.5)
                        .suffix(" dB"),
                );
                ui.end_row();
            }

            ui.label(i18n.tr("options-pan"));
            let (left, right, center) = (
                i18n.tr("options-pan-left"),
                i18n.tr("options-pan-right"),
                i18n.tr("options-pan-center"),
            );
            ui.add(
                Slider::new(&mut v.pan, -1.0..=1.0)
                    .step_by(0.05)
                    .custom_formatter(move |x, _| {
                        #[allow(clippy::cast_possible_truncation)]
                        let pct = (x.abs() * 100.0).round() as i32;
                        match x {
                            x if x < -0.001 => format!("{left} {pct}"),
                            x if x > 0.001 => format!("{right} {pct}"),
                            _ => center.clone(),
                        }
                    }),
            );
            ui.end_row();

            ui.label("");
            ui.checkbox(&mut v.bell, i18n.tr("options-bell"));
            ui.end_row();
        });
    ui.add_space(14.0);
    if ui.button(i18n.tr("options-defaults")).clicked() {
        *v = VolumeSettings::default();
    }
}
