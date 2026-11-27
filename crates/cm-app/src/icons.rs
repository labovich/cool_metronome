//! Vector icons from the mockup's SVGs (24×24 view box), drawn with the
//! painter: no icon font, no image decoding.

use egui::{Color32, Painter, Pos2, Shape, Stroke, pos2, vec2};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Minus,
    Plus,
    Chevron,
    Presets,
    Volume,
    VolumeMuted,
    Play,
    Stop,
    Logo,
    Pin,
    Tray,
    Close,
    Expand,
}

type Path = &'static [(f32, f32)];

const SPEAKER: Path = &[
    (4.0, 9.0),
    (4.0, 15.0),
    (8.0, 15.0),
    (13.0, 19.0),
    (13.0, 5.0),
    (8.0, 9.0),
    (4.0, 9.0),
];

/// Outline paths of a stroked icon.
fn paths(icon: Icon) -> &'static [Path] {
    match icon {
        Icon::Minus => &[&[(5.0, 12.0), (19.0, 12.0)]],
        Icon::Plus => &[&[(5.0, 12.0), (19.0, 12.0)], &[(12.0, 5.0), (12.0, 19.0)]],
        Icon::Chevron => &[&[(6.0, 9.0), (12.0, 15.0), (18.0, 9.0)]],
        Icon::Presets => &[&[
            (6.0, 3.0),
            (18.0, 3.0),
            (18.0, 21.0),
            (12.0, 17.0),
            (6.0, 21.0),
            (6.0, 3.0),
        ]],
        // The sound wave arc is added in `paint`.
        Icon::Volume => &[SPEAKER],
        Icon::VolumeMuted => &[
            SPEAKER,
            &[(16.5, 9.5), (21.5, 14.5)],
            &[(21.5, 9.5), (16.5, 14.5)],
        ],
        Icon::Logo => &[
            &[
                (9.0, 3.0),
                (15.0, 3.0),
                (19.0, 21.0),
                (5.0, 21.0),
                (9.0, 3.0),
            ],
            &[(12.0, 15.0), (17.0, 6.0)],
        ],
        Icon::Pin => &[
            &[
                (9.0, 3.0),
                (15.0, 3.0),
                (14.0, 10.0),
                (18.0, 13.0),
                (6.0, 13.0),
                (10.0, 10.0),
                (9.0, 3.0),
            ],
            &[(12.0, 13.0), (12.0, 21.0)],
        ],
        Icon::Tray => &[
            &[(12.0, 4.0), (12.0, 14.0)],
            &[(8.0, 10.0), (12.0, 14.0), (16.0, 10.0)],
            &[(4.0, 16.0), (4.0, 20.0), (20.0, 20.0), (20.0, 16.0)],
        ],
        Icon::Close => &[&[(6.0, 6.0), (18.0, 18.0)], &[(18.0, 6.0), (6.0, 18.0)]],
        Icon::Expand => &[
            &[(14.0, 4.0), (20.0, 4.0), (20.0, 10.0)],
            &[(10.0, 20.0), (4.0, 20.0), (4.0, 14.0)],
            &[(20.0, 4.0), (13.0, 11.0)],
            &[(4.0, 20.0), (11.0, 13.0)],
        ],
        Icon::Play | Icon::Stop => &[],
    }
}

/// Paints `icon` `size` points wide, centered at `center`. `svg_stroke` is
/// the SVG stroke width (in view box units).
pub fn paint(
    painter: &Painter,
    center: Pos2,
    size: f32,
    icon: Icon,
    color: Color32,
    svg_stroke: f32,
) {
    let scale = size / 24.0;
    let origin = center - vec2(12.0, 12.0) * scale;
    let at = |(x, y): (f32, f32)| origin + vec2(x, y) * scale;
    let stroke = Stroke::new(svg_stroke * scale, color);

    match icon {
        Icon::Play => {
            let pts = [(8.0, 5.5), (8.0, 18.5), (19.0, 12.0)].map(at).to_vec();
            painter.add(Shape::convex_polygon(pts, color, Stroke::NONE));
            return;
        }
        Icon::Stop => {
            let rect = egui::Rect::from_min_max(at((6.0, 6.0)), at((18.0, 18.0)));
            painter.rect_filled(rect, 2.0 * scale, color);
            return;
        }
        _ => {}
    }

    for path in paths(icon) {
        let pts: Vec<Pos2> = path.iter().copied().map(at).collect();
        polyline(painter, pts, stroke);
    }
    if icon == Icon::Volume {
        // SVG "M16.5 9a4 4 0 010 6": arc of radius 4 bulging right.
        let c = (16.5 - 7.0_f32.sqrt(), 12.0);
        let half = (3.0_f32 / 4.0).asin();
        let pts: Vec<Pos2> = (0..=8)
            .map(|i| {
                let a = -half + 2.0 * half * i as f32 / 8.0;
                at((c.0 + 4.0 * a.cos(), c.1 + 4.0 * a.sin()))
            })
            .collect();
        polyline(painter, pts, stroke);
    }
}

/// A stroked polyline with round caps and joins.
fn polyline(painter: &Painter, pts: Vec<Pos2>, stroke: Stroke) {
    let r = stroke.width / 2.0;
    for p in &pts {
        painter.circle_filled(*p, r, stroke.color);
    }
    painter.add(Shape::line(pts, stroke));
}

/// Center of a rect rounded to the pixel grid, so thin strokes stay crisp.
pub fn crisp_center(painter: &Painter, rect: egui::Rect) -> Pos2 {
    let ppp = painter.pixels_per_point();
    let c = rect.center();
    pos2((c.x * ppp).round() / ppp, (c.y * ppp).round() / ppp)
}
