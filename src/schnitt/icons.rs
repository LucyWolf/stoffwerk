//! Werkzeugsymbole als Vektorzeichnung: bleiben bei jeder Bildschirmskalierung scharf, keine Bilddateien.
//! Eigene Zeichnungen im Stil schlichter Strich-Symbole.

use eframe::egui::{self, Color32, Pos2, Rect, Response, Sense, Shape, Stroke, Vec2};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Icon {
    Select,
    EditPoints,
    AddPoint,
    Polygon,
    Rect,
    Ellipse,
    InternalLine,
    Seam,
    Grid,
    Split,
    Undo,
    Redo,
}

const SIZE: f32 = 26.0;

/// Symbolknopf; `active` hebt ihn hervor (gewähltes Werkzeug, eingeschalteter Schalter)
pub fn button(ui: &mut egui::Ui, icon: Icon, active: bool, enabled: bool, tip: &str) -> Response {
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(SIZE), sense);
    let visuals = ui.visuals();
    let bg = if active {
        visuals.selection.bg_fill
    } else if response.hovered() && enabled {
        visuals.widgets.hovered.weak_bg_fill
    } else {
        Color32::TRANSPARENT
    };
    let p = ui.painter();
    p.rect_filled(rect, 4.0, bg);
    let color = if !enabled {
        Color32::from_gray(85)
    } else if active {
        Color32::WHITE
    } else {
        Color32::from_gray(200)
    };
    draw(p, rect.shrink(5.0), icon, color);
    response.on_hover_text(tip)
}

fn draw(p: &egui::Painter, r: Rect, icon: Icon, c: Color32) {
    // Koordinaten 0..1 innerhalb des Symbolfelds
    let at = |x: f32, y: f32| Pos2::new(r.left() + x * r.width(), r.top() + y * r.height());
    let s = Stroke::new(1.4, c);
    let thin = Stroke::new(1.0, c);
    let accent = Color32::from_rgb(255, 170, 60);
    let blue = Color32::from_rgb(110, 160, 255);
    let line = |a: (f32, f32), b: (f32, f32), st: Stroke| p.line_segment([at(a.0, a.1), at(b.0, b.1)], st);
    let poly = |pts: &[(f32, f32)], closed: bool, st: Stroke| {
        let v: Vec<Pos2> = pts.iter().map(|&(x, y)| at(x, y)).collect();
        p.add(if closed { Shape::closed_line(v, st) } else { Shape::line(v, st) });
    };
    let dot = |x: f32, y: f32, col: Color32| p.rect_filled(Rect::from_center_size(at(x, y), Vec2::splat(3.5)), 0.0, col);
    // Mauszeiger (Pfeil), links oben angesetzt
    let cursor = |ox: f32, oy: f32, k: f32| {
        let pts = [(0.0, 0.0), (0.0, 0.75), (0.2, 0.58), (0.33, 0.85), (0.43, 0.8), (0.3, 0.53), (0.55, 0.53)];
        let v: Vec<Pos2> = pts.iter().map(|&(x, y)| at(ox + x * k, oy + y * k)).collect();
        p.add(Shape::convex_polygon(v.clone(), c, Stroke::NONE));
    };

    match icon {
        Icon::Select => {
            poly(&[(0.45, 0.35), (1.0, 0.35), (1.0, 1.0), (0.45, 1.0)], true, thin);
            cursor(0.0, 0.0, 0.8);
        }
        Icon::EditPoints => {
            line((0.35, 0.95), (1.0, 0.45), s);
            dot(0.35, 0.95, accent);
            dot(1.0, 0.45, c);
            cursor(0.0, 0.0, 0.7);
        }
        Icon::AddPoint => {
            line((0.0, 0.85), (1.0, 0.85), s);
            dot(0.5, 0.85, accent);
            line((0.5, 0.05), (0.5, 0.55), s);
            line((0.25, 0.3), (0.75, 0.3), s);
        }
        Icon::Polygon => {
            let pts = [(0.5, 0.02), (0.98, 0.38), (0.8, 0.98), (0.2, 0.98), (0.02, 0.38)];
            poly(&pts, true, s);
            for (x, y) in pts {
                dot(x, y, c);
            }
        }
        Icon::Rect => {
            let pts = [(0.05, 0.15), (0.95, 0.15), (0.95, 0.85), (0.05, 0.85)];
            poly(&pts, true, s);
            for (x, y) in pts {
                dot(x, y, c);
            }
        }
        Icon::Ellipse => {
            p.circle_stroke(r.center(), r.width() * 0.47, s);
            dot(0.97, 0.5, c);
            dot(0.5, 0.03, c);
        }
        Icon::InternalLine => {
            poly(&[(0.05, 0.05), (0.95, 0.05), (0.95, 0.95), (0.05, 0.95)], true, thin);
            poly(&[(0.25, 0.75), (0.5, 0.3), (0.75, 0.7)], false, Stroke::new(1.6, blue));
        }
        Icon::Seam => {
            line((0.1, 0.05), (0.1, 0.95), Stroke::new(2.0, accent));
            line((0.9, 0.05), (0.9, 0.95), Stroke::new(2.0, accent));
            for k in 0..4 {
                let y = 0.15 + k as f32 * 0.23;
                line((0.18, y), (0.82, y), Stroke::new(1.0, c));
            }
        }
        Icon::Grid => {
            for k in 0..4 {
                let t = 0.05 + k as f32 * 0.3;
                line((t, 0.05), (t, 0.95), thin);
                line((0.05, t), (0.95, t), thin);
            }
        }
        Icon::Split => {
            poly(&[(0.05, 0.05), (0.95, 0.05), (0.95, 0.95), (0.05, 0.95)], true, s);
            line((0.05, 0.5), (0.95, 0.5), s);
        }
        Icon::Undo | Icon::Redo => {
            // Bogen mit Pfeilspitze; Wiederholen ist gespiegelt
            let flip = icon == Icon::Redo;
            let fx = |x: f32| if flip { 1.0 - x } else { x };
            let arc: Vec<(f32, f32)> = (0..=12)
                .map(|i| {
                    let a = std::f32::consts::PI * (i as f32 / 12.0);
                    (fx(0.5 + 0.38 * a.cos()), 0.6 - 0.38 * a.sin())
                })
                .collect();
            poly(&arc, false, s);
            let tip = (fx(0.12), 0.6);
            line(tip, (fx(0.0), 0.38), s);
            line(tip, (fx(0.32), 0.48), s);
        }
    }
}
