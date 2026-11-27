//! Colors and fonts from the approved mockup (`docs/design/`).

use egui::{Color32, FontData, FontDefinitions, FontFamily, FontId, Theme};
use std::sync::Arc;

/// Accent color (beats, Play, slider).
pub const ACCENT: Color32 = Color32::from_rgb(0xFF, 0x8A, 0x3D);
/// Icon color on the accent-filled Play button.
pub const ON_ACCENT: Color32 = Color32::from_rgb(0x1A, 0x12, 0x05);

/// The colors of one theme.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub bg: Color32,
    pub surface: Color32,
    pub border: Color32,
    /// Stronger border: TAP outline, muted beats, slider track.
    pub border2: Color32,
    pub text: Color32,
    pub text2: Color32,
    /// Normal (non-accent) beat.
    pub beat: Color32,
}

impl Palette {
    pub const DARK: Palette = Palette {
        bg: Color32::from_rgb(0x16, 0x17, 0x1B),
        surface: Color32::from_rgb(0x25, 0x27, 0x2E),
        border: Color32::from_rgb(0x30, 0x33, 0x3B),
        border2: Color32::from_rgb(0x4A, 0x4E, 0x58),
        text: Color32::from_rgb(0xF1, 0xF1, 0xF4),
        text2: Color32::from_rgb(0xA3, 0xA5, 0xAD),
        beat: Color32::from_rgb(0xC9, 0xCB, 0xD2),
    };

    pub const LIGHT: Palette = Palette {
        bg: Color32::from_rgb(0xFA, 0xFA, 0xFB),
        surface: Color32::from_rgb(0xEC, 0xED, 0xF0),
        border: Color32::from_rgb(0xDA, 0xDC, 0xE1),
        border2: Color32::from_rgb(0xB9, 0xBC, 0xC4),
        text: Color32::from_rgb(0x18, 0x19, 0x1D),
        text2: Color32::from_rgb(0x58, 0x5B, 0x63),
        beat: Color32::from_rgb(0x6B, 0x6F, 0x78),
    };

    pub fn of(theme: Theme) -> Palette {
        match theme {
            Theme::Dark => Self::DARK,
            Theme::Light => Self::LIGHT,
        }
    }

    /// `surface` lifted a little towards the text color, for hover.
    pub fn surface_hover(&self) -> Color32 {
        mix(self.surface, self.text, 0.08)
    }

    /// `surface` pressed.
    pub fn surface_active(&self) -> Color32 {
        mix(self.surface, self.text, 0.16)
    }
}

/// Linear blend of two colors in sRGB space (`t` = 0 → `a`, 1 → `b`).
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let ch = |x: u8, y: u8| {
        let v = f32::from(x) + (f32::from(y) - f32::from(x)) * t;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let v = v.round().clamp(0.0, 255.0) as u8;
        v
    };
    Color32::from_rgb(ch(a.r(), b.r()), ch(a.g(), b.g()), ch(a.b(), b.b()))
}

/// The semibold family (headings, numbers, chips).
pub fn semibold() -> FontFamily {
    FontFamily::Name("semibold".into())
}

pub fn medium(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

pub fn bold(size: f32) -> FontId {
    FontId::new(size, semibold())
}

/// Installs the embedded Inter subset (Medium and SemiBold, Latin only).
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::empty();
    fonts.font_data.insert(
        "inter-medium".into(),
        Arc::new(FontData::from_static(include_bytes!(
            "../assets/fonts/Inter-Medium.subset.ttf"
        ))),
    );
    fonts.font_data.insert(
        "inter-semibold".into(),
        Arc::new(FontData::from_static(include_bytes!(
            "../assets/fonts/Inter-SemiBold.subset.ttf"
        ))),
    );
    fonts
        .families
        .insert(FontFamily::Proportional, vec!["inter-medium".into()]);
    fonts
        .families
        .insert(FontFamily::Monospace, vec!["inter-medium".into()]);
    fonts
        .families
        .insert(semibold(), vec!["inter-semibold".into()]);
    ctx.set_fonts(fonts);
}

/// egui visuals for both themes (popups, menus, tooltips).
pub fn install_visuals(ctx: &egui::Context) {
    for theme in [Theme::Dark, Theme::Light] {
        let p = Palette::of(theme);
        let mut v = match theme {
            Theme::Dark => egui::Visuals::dark(),
            Theme::Light => egui::Visuals::light(),
        };
        v.panel_fill = p.bg;
        v.window_fill = p.surface;
        v.window_stroke = egui::Stroke::new(1.0, p.border);
        v.extreme_bg_color = p.bg;
        v.faint_bg_color = p.surface;
        v.selection.bg_fill = mix(p.surface, ACCENT, 0.35);
        v.selection.stroke = egui::Stroke::new(1.0, p.text);
        v.hyperlink_color = ACCENT;
        for w in [
            &mut v.widgets.noninteractive,
            &mut v.widgets.inactive,
            &mut v.widgets.hovered,
            &mut v.widgets.active,
            &mut v.widgets.open,
        ] {
            w.fg_stroke.color = p.text;
            w.bg_stroke = egui::Stroke::NONE;
            w.corner_radius = egui::CornerRadius::same(6);
        }
        v.widgets.noninteractive.fg_stroke.color = p.text2;
        v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, p.border);
        v.widgets.inactive.weak_bg_fill = p.surface;
        v.widgets.inactive.bg_fill = p.surface;
        v.widgets.hovered.weak_bg_fill = p.surface_hover();
        v.widgets.hovered.bg_fill = p.surface_hover();
        v.widgets.active.weak_bg_fill = p.surface_active();
        v.widgets.active.bg_fill = p.surface_active();
        v.widgets.open.weak_bg_fill = p.surface_hover();
        ctx.set_visuals_of(theme, v);
    }
    ctx.all_styles_mut(|style| {
        use egui::TextStyle;
        style.text_styles = [
            (TextStyle::Small, medium(11.0)),
            (TextStyle::Body, medium(13.0)),
            (TextStyle::Button, medium(13.0)),
            (TextStyle::Heading, bold(16.0)),
            (TextStyle::Monospace, medium(13.0)),
        ]
        .into();
        style.spacing.button_padding = egui::vec2(10.0, 5.0);
        style.spacing.menu_margin = egui::Margin::same(6);
        style.interaction.selectable_labels = false;
    });
}
