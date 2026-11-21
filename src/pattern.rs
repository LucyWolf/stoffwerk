//! 2D-Schnittansicht. In Abschnitt 1 nur die Zeichenfläche: Raster in Zentimetern, verschieben, zoomen.
//! Die Schnittteile selbst kommen in Abschnitt 2.

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};

pub struct PatternView {
    /// Lage des Nullpunkts relativ zur Mitte der Fläche, in Bildpunkten
    offset: Vec2,
    /// Bildpunkte pro Zentimeter
    zoom: f32,
}

impl Default for PatternView {
    fn default() -> Self {
        Self { offset: Vec2::ZERO, zoom: 4.0 }
    }
}

impl PatternView {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, Color32::from_rgb(24, 26, 30));

        // Verschieben: mittlere oder rechte Maustaste, oder Leertaste + linke
        let space = ui.input(|i| i.key_down(egui::Key::Space));
        if response.dragged_by(egui::PointerButton::Middle)
            || response.dragged_by(egui::PointerButton::Secondary)
            || (space && response.dragged_by(egui::PointerButton::Primary))
        {
            self.offset += response.drag_delta();
        }

        // Zoomen zur Mausposition hin
        if let Some(hover) = response.hover_pos() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                let before = self.to_cm(rect, hover);
                self.zoom = (self.zoom * (scroll * 0.002).exp()).clamp(0.3, 80.0);
                let after = self.to_screen(rect, before);
                self.offset += hover - after;
            }
        }

        self.draw_grid(&painter, rect);

        // Hinweis, solange es noch keine Teile gibt
        painter.text(
            rect.center_bottom() - Vec2::new(0.0, 14.0),
            egui::Align2::CENTER_BOTTOM,
            "Schnittteile zeichnen kommt im nächsten Abschnitt",
            egui::FontId::proportional(13.0),
            Color32::from_gray(110),
        );

        // Mausposition in cm
        if let Some(hover) = response.hover_pos() {
            let cm = self.to_cm(rect, hover);
            painter.text(
                rect.left_top() + Vec2::new(8.0, 6.0),
                egui::Align2::LEFT_TOP,
                format!("x {:.1} cm   y {:.1} cm", cm.x, cm.y),
                egui::FontId::monospace(12.0),
                Color32::from_gray(150),
            );
        }
    }

    fn origin(&self, rect: Rect) -> Pos2 {
        rect.center() + self.offset
    }

    /// Bildschirm → Zentimeter (y nach oben)
    fn to_cm(&self, rect: Rect, p: Pos2) -> Vec2 {
        let d = p - self.origin(rect);
        Vec2::new(d.x / self.zoom, -d.y / self.zoom)
    }

    fn to_screen(&self, rect: Rect, cm: Vec2) -> Pos2 {
        self.origin(rect) + Vec2::new(cm.x * self.zoom, -cm.y * self.zoom)
    }

    fn draw_grid(&self, painter: &egui::Painter, rect: Rect) {
        // Feinster Abstand, der noch mindestens 8 px breit ist
        let steps = [0.5f32, 1.0, 5.0, 10.0, 50.0, 100.0];
        let minor = steps.iter().copied().find(|s| s * self.zoom >= 8.0).unwrap_or(100.0);
        let major = minor * if minor == 0.5 || minor == 5.0 || minor == 50.0 { 2.0 } else { 10.0 };

        let a = self.to_cm(rect, rect.left_bottom());
        let b = self.to_cm(rect, rect.right_top());

        let line = |cm: f32, vertical: bool| {
            let is_axis = cm.abs() < minor * 0.01;
            let is_major = ((cm / major).round() * major - cm).abs() < minor * 0.01;
            let color = if is_axis {
                Color32::from_gray(120)
            } else if is_major {
                Color32::from_gray(58)
            } else {
                Color32::from_gray(36)
            };
            let stroke = Stroke::new(1.0, color);
            if vertical {
                let x = self.to_screen(rect, Vec2::new(cm, 0.0)).x;
                painter.line_segment([Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())], stroke);
            } else {
                let y = self.to_screen(rect, Vec2::new(0.0, cm)).y;
                painter.line_segment([Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)], stroke);
            }
        };

        let mut x = (a.x / minor).floor() * minor;
        while x <= b.x {
            line(x, true);
            x += minor;
        }
        let mut y = (a.y / minor).floor() * minor;
        while y <= b.y {
            line(y, false);
            y += minor;
        }

        painter.text(
            rect.right_top() + Vec2::new(-8.0, 6.0),
            egui::Align2::RIGHT_TOP,
            format!("Raster {minor} cm"),
            egui::FontId::proportional(12.0),
            Color32::from_gray(120),
        );
    }
}
