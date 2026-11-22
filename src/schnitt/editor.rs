//! 2D-Schnittfenster: Werkzeuge, Zeichnen, Bearbeiten, Nähen, Rechtsklick-Menü, Rückgängig.

use super::icons::{self, Icon};
use super::model::*;
use crate::import::AvatarMesh;
use eframe::egui::{self, Color32, Key, Pos2, Rect, Sense, Stroke, Vec2 as EVec2};
use glam::Vec2;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
    Select,
    EditPoints,
    AddPoint,
    Polygon,
    Rect,
    Ellipse,
    InternalLine,
    Seam,
}

impl Tool {
    const ALL: [Tool; 8] = [
        Tool::Select,
        Tool::EditPoints,
        Tool::AddPoint,
        Tool::Polygon,
        Tool::Rect,
        Tool::Ellipse,
        Tool::InternalLine,
        Tool::Seam,
    ];

    fn label(self) -> &'static str {
        match self {
            Tool::Select => "Auswählen",
            Tool::EditPoints => "Punkte",
            Tool::AddPoint => "+ Punkt",
            Tool::Polygon => "Polygon",
            Tool::Rect => "Rechteck",
            Tool::Ellipse => "Kreis",
            Tool::InternalLine => "Innenlinie",
            Tool::Seam => "Naht",
        }
    }

    fn icon(self) -> Icon {
        match self {
            Tool::Select => Icon::Select,
            Tool::EditPoints => Icon::EditPoints,
            Tool::AddPoint => Icon::AddPoint,
            Tool::Polygon => Icon::Polygon,
            Tool::Rect => Icon::Rect,
            Tool::Ellipse => Icon::Ellipse,
            Tool::InternalLine => Icon::InternalLine,
            Tool::Seam => Icon::Seam,
        }
    }

    fn key(self) -> Key {
        match self {
            Tool::Select => Key::A,
            Tool::EditPoints => Key::Z,
            Tool::AddPoint => Key::X,
            Tool::Polygon => Key::H,
            Tool::Rect => Key::S,
            Tool::Ellipse => Key::E,
            Tool::InternalLine => Key::I,
            Tool::Seam => Key::N,
        }
    }

    fn help(self) -> &'static str {
        match self {
            Tool::Select => "Schnittteile auswählen und verschieben (A)",
            Tool::EditPoints => "Punkte und Kanten ziehen, Rechtsklick für mehr (Z)",
            Tool::AddPoint => "Klick auf eine Kante setzt dort einen Punkt (X)",
            Tool::Polygon => "Punkte klicken, Umschalt = Kurvenpunkt; auf den ersten Punkt oder Enter schließt (H)",
            Tool::Rect => "Rechteck aufziehen (S)",
            Tool::Ellipse => "Kreis/Ellipse aufziehen, Umschalt = exakter Kreis (E)",
            Tool::InternalLine => "Innenlinie in ein Teil klicken, Enter oder Doppelklick beendet (I)",
            Tool::Seam => "Erst eine Kante, dann die Gegenkante anklicken – jeweils nahe dem Ende, das zusammengehört (N)",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Sel {
    None,
    Piece(u64),
    Point { piece: u64, pt: u64 },
    Segment(SegRef),
    Seam(u64),
}

#[derive(Clone, Copy)]
struct View {
    /// Nullpunkt relativ zur Mitte der Fläche, in Bildpunkten
    offset: EVec2,
    /// Bildpunkte pro Zentimeter
    zoom: f32,
}

impl Default for View {
    fn default() -> Self {
        // Nullpunkt (Füße des Avatars) unten, damit ein 1,75-m-Avatar ins Bild passt
        Self { offset: EVec2::new(0.0, 300.0), zoom: 3.0 }
    }
}

impl View {
    fn origin(&self, r: Rect) -> Pos2 {
        r.center() + self.offset
    }
    fn to_cm(&self, r: Rect, p: Pos2) -> Vec2 {
        let d = p - self.origin(r);
        Vec2::new(d.x / self.zoom, -d.y / self.zoom)
    }
    fn to_px(&self, r: Rect, cm: Vec2) -> Pos2 {
        self.origin(r) + EVec2::new(cm.x * self.zoom, -cm.y * self.zoom)
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Hit {
    None,
    Point { piece: u64, pt: u64 },
    Segment { seg: SegRef, at: Vec2, near_start: bool },
    Piece(u64),
}

#[derive(Clone, Copy)]
enum Drag {
    Pan,
    Piece { id: u64, last: Vec2 },
    Point { piece: u64, pt: u64 },
    Segment { seg: SegRef, last: Vec2 },
    Box { from: Vec2 },
}

struct DartDialog {
    /// Wo das Fenster aufgeht: neben der Stelle, an der geklickt wurde
    at: Option<Pos2>,
    piece: u64,
    pt: u64,
    width: f32,
    length: f32,
}

pub struct Editor {
    pub tool: Tool,
    pub sel: Sel,
    pub split: bool,
    pub snap: bool,
    views: [View; 2],
    drawing: Vec<(Vec2, bool)>,
    drag: Option<Drag>,
    seam_first: Option<(SegRef, bool)>,
    undo: Vec<Doc>,
    redo: Vec<Doc>,
    dart: Option<DartDialog>,
    ctx_hit: Hit,
    silhouette: Option<(egui::TextureHandle, Vec2, Vec2)>,
    /// Wird bei jeder Änderung gesetzt; die App zeigt dann „ungespeichert“
    pub dirty: bool,
}

const SEAM_COLORS: [Color32; 8] = [
    Color32::from_rgb(255, 120, 80),
    Color32::from_rgb(80, 200, 120),
    Color32::from_rgb(90, 160, 255),
    Color32::from_rgb(240, 200, 60),
    Color32::from_rgb(220, 100, 220),
    Color32::from_rgb(60, 210, 210),
    Color32::from_rgb(255, 160, 200),
    Color32::from_rgb(170, 140, 255),
];

fn seam_color(i: usize) -> Color32 {
    SEAM_COLORS[i % SEAM_COLORS.len()]
}

impl Default for Editor {
    fn default() -> Self {
        Self {
            tool: Tool::Select,
            sel: Sel::None,
            split: false,
            snap: true,
            views: [View::default(), View { offset: EVec2::new(0.0, 500.0), zoom: 6.0 }],
            drawing: Vec::new(),
            drag: None,
            seam_first: None,
            undo: Vec::new(),
            redo: Vec::new(),
            dart: None,
            ctx_hit: Hit::None,
            silhouette: None,
            dirty: false,
        }
    }
}

impl Editor {
    pub fn reset_views(&mut self) {
        self.views = Editor::default().views;
    }

    pub fn clear_history(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.sel = Sel::None;
        self.drawing.clear();
        self.seam_first = None;
        self.dirty = false;
    }

    /// Zustand vor einer Änderung merken
    fn checkpoint(&mut self, doc: &Doc) {
        self.undo.push(doc.clone());
        if self.undo.len() > 200 {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.dirty = true;
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self, doc: &mut Doc) {
        if let Some(prev) = self.undo.pop() {
            self.redo.push(std::mem::replace(doc, prev));
            self.after_history(doc);
        }
    }

    pub fn redo(&mut self, doc: &mut Doc) {
        if let Some(next) = self.redo.pop() {
            self.undo.push(std::mem::replace(doc, next));
            self.after_history(doc);
        }
    }

    fn after_history(&mut self, doc: &Doc) {
        self.dirty = true;
        self.drag = None;
        let valid = match self.sel {
            Sel::Piece(id) => doc.piece(id).is_some(),
            Sel::Point { piece, pt } => doc.piece(piece).is_some_and(|p| p.index_of(pt).is_some()),
            Sel::Segment(s) => doc.segment(s).is_some(),
            Sel::Seam(id) => doc.seams.iter().any(|s| s.id == id),
            Sel::None => true,
        };
        if !valid {
            self.sel = Sel::None;
        }
    }

    /// Silhouette des Avatars von vorn, als Hintergrund im Schnittfenster
    pub fn set_avatar(&mut self, ctx: &egui::Context, avatar: Option<&AvatarMesh>) {
        self.silhouette = avatar.map(|a| {
            let res = 3.0; // Bildpunkte pro cm
            let min = Vec2::new(a.min.x, a.min.y) * 100.0;
            let max = Vec2::new(a.max.x, a.max.y) * 100.0;
            let w = (((max.x - min.x) * res) as usize).clamp(1, 4096);
            let h = (((max.y - min.y) * res) as usize).clamp(1, 4096);
            let mut mask = vec![false; w * h];
            let to_px = |p: glam::Vec3| {
                Vec2::new((p.x * 100.0 - min.x) * res, (max.y - p.y * 100.0) * res)
            };
            for t in a.indices.chunks_exact(3) {
                let [p0, p1, p2] = [t[0], t[1], t[2]].map(|i| to_px(a.positions[i as usize]));
                fill_triangle(&mut mask, w, h, p0, p1, p2);
            }
            let pixels = mask
                .iter()
                .map(|&m| if m { Color32::from_rgba_unmultiplied(120, 124, 132, 255) } else { Color32::TRANSPARENT })
                .collect();
            let img = egui::ColorImage::new([w, h], pixels);
            let tex = ctx.load_texture("silhouette", img, egui::TextureOptions::LINEAR);
            (tex, min, max)
        });
    }

    fn snap_pt(&self, p: Vec2, ctx: &egui::Context) -> Vec2 {
        // Strg schaltet das Einrasten vorübergehend um
        let on = self.snap != ctx.input(|i| i.modifiers.ctrl);
        if on { (p * 2.0).round() * 0.5 } else { p }
    }

    // ---------------------------------------------------------------- Werkzeugleiste

    pub fn toolbar(&mut self, ui: &mut egui::Ui, doc: &mut Doc) {
        ui.spacing_mut().item_spacing.x = 2.0;
        for t in Tool::ALL {
            let tip = format!("{}\n{}", t.label(), t.help());
            if icons::button(ui, t.icon(), self.tool == t, true, &tip).clicked() {
                self.set_tool(t);
            }
        }
        ui.separator();
        if icons::button(ui, Icon::Grid, self.snap, true, "Raster\nPunkte rasten auf 0,5 cm ein (Strg hält es kurz aus)").clicked() {
            self.snap = !self.snap;
        }
        if icons::button(ui, Icon::Split, self.split, true, "Ansicht teilen\nZwei Ansichten übereinander: Übersicht und Ausschnitt").clicked() {
            self.split = !self.split;
        }
        ui.separator();
        if icons::button(ui, Icon::Undo, false, self.can_undo(), "Rückgängig (Strg+Z)").clicked() {
            self.undo(doc);
        }
        if icons::button(ui, Icon::Redo, false, self.can_redo(), "Wiederholen (Strg+Y)").clicked() {
            self.redo(doc);
        }
    }

    fn set_tool(&mut self, t: Tool) {
        self.tool = t;
        self.drawing.clear();
        self.seam_first = None;
        self.drag = None;
    }

    // ---------------------------------------------------------------- Ansichten

    pub fn ui(&mut self, ui: &mut egui::Ui, doc: &mut Doc) {
        self.shortcuts(ui, doc);
        let full = ui.available_rect_before_wrap();
        ui.allocate_rect(full, Sense::hover());
        if self.split {
            let mid = full.center().y;
            let top = Rect::from_min_max(full.min, Pos2::new(full.max.x, mid - 1.0));
            let bottom = Rect::from_min_max(Pos2::new(full.min.x, mid + 1.0), full.max);
            self.view_ui(ui, doc, top, 0);
            self.view_ui(ui, doc, bottom, 1);
            ui.painter().hline(full.x_range(), mid, Stroke::new(2.0, Color32::from_gray(70)));
        } else {
            self.view_ui(ui, doc, full, 0);
        }
        self.dart_dialog(ui.ctx(), doc);
    }

    fn shortcuts(&mut self, ui: &egui::Ui, doc: &mut Doc) {
        if ui.ctx().egui_wants_keyboard_input() {
            return;
        }
        let (undo, redo, del, enter, esc, back, tool) = ui.input(|i| {
            let cmd = i.modifiers.command;
            let tool = if cmd || i.modifiers.alt { None } else { Tool::ALL.into_iter().find(|t| i.key_pressed(t.key())) };
            (
                cmd && i.key_pressed(Key::Z) && !i.modifiers.shift,
                cmd && (i.key_pressed(Key::Y) || (i.key_pressed(Key::Z) && i.modifiers.shift)),
                i.key_pressed(Key::Delete),
                i.key_pressed(Key::Enter),
                i.key_pressed(Key::Escape),
                i.key_pressed(Key::Backspace),
                tool,
            )
        });
        if undo {
            self.undo(doc);
        } else if redo {
            self.redo(doc);
        }
        if let Some(t) = tool {
            self.set_tool(t);
        }
        if esc {
            if self.drawing.is_empty() && self.seam_first.is_none() {
                self.sel = Sel::None;
            }
            self.drawing.clear();
            self.seam_first = None;
        }
        if back && !self.drawing.is_empty() {
            self.drawing.pop();
        }
        if enter {
            self.finish_drawing(doc);
        }
        if del || (back && self.drawing.is_empty()) {
            self.delete_selection(doc);
        }
    }

    pub fn delete_selection(&mut self, doc: &mut Doc) {
        match self.sel {
            Sel::Piece(id) => {
                self.checkpoint(doc);
                doc.delete_piece(id);
                self.sel = Sel::None;
            }
            Sel::Point { piece, pt } => self.delete_point(doc, piece, pt),
            Sel::Seam(id) => {
                self.checkpoint(doc);
                doc.seams.retain(|s| s.id != id);
                self.sel = Sel::None;
            }
            _ => {}
        }
    }

    fn delete_point(&mut self, doc: &mut Doc, piece: u64, pt: u64) {
        let Some(p) = doc.piece(piece) else { return };
        if p.pts.len() <= 3 {
            return;
        }
        self.checkpoint(doc);
        if let Some(p) = doc.piece_mut(piece) {
            p.pts.retain(|x| x.id != pt);
        }
        doc.sync_partner(piece);
        doc.cleanup_seams();
        self.sel = Sel::Piece(piece);
    }

    fn finish_drawing(&mut self, doc: &mut Doc) {
        match self.tool {
            Tool::Polygon if self.drawing.len() >= 3 => {
                self.checkpoint(doc);
                let name = doc.next_name();
                let pts = std::mem::take(&mut self.drawing);
                let id = doc.add_piece(&name, &pts);
                self.sel = Sel::Piece(id);
            }
            Tool::InternalLine if self.drawing.len() >= 2 => {
                let pts: Vec<Vec2> = self.drawing.iter().map(|d| d.0).collect();
                let owner = doc.pieces.iter().rev().find(|p| p.contains(pts[0])).map(|p| p.id);
                if let Some(owner) = owner {
                    self.checkpoint(doc);
                    let id = doc.new_id();
                    if let Some(p) = doc.piece_mut(owner) {
                        p.lines.push(InternalLine { id, pts });
                    }
                    doc.sync_partner(owner);
                }
                self.drawing.clear();
            }
            _ => {}
        }
    }

    fn hit(&self, doc: &Doc, p: Vec2, tol: f32, points_everywhere: bool) -> Hit {
        let selected_piece = match self.sel {
            Sel::Piece(id) | Sel::Point { piece: id, .. } => Some(id),
            Sel::Segment(s) => Some(s.piece),
            _ => None,
        };
        // Punkte: in Punkt-Werkzeugen überall, sonst nur am ausgewählten Teil
        let mut best: Option<(f32, Hit)> = None;
        for piece in doc.pieces.iter().rev() {
            if points_everywhere || selected_piece == Some(piece.id) {
                for pt in &piece.pts {
                    let d = pt.p.distance(p);
                    if d < tol && best.is_none_or(|b| d < b.0) {
                        best = Some((d, Hit::Point { piece: piece.id, pt: pt.id }));
                    }
                }
            }
        }
        if let Some((_, h)) = best {
            return h;
        }
        for piece in doc.pieces.iter().rev() {
            for s in piece.segments() {
                let (d, i, t) = dist_to_poly(p, &s.poly);
                if d < tol {
                    let along: f32 = s.poly.windows(2).take(i).map(|w| w[0].distance(w[1])).sum::<f32>()
                        + s.poly.get(i + 1).map(|b| s.poly[i].distance(*b) * t).unwrap_or(0.0);
                    let total = poly_len(&s.poly);
                    return Hit::Segment {
                        seg: SegRef { piece: piece.id, start: s.start_id },
                        at: s.poly[i].lerp(*s.poly.get(i + 1).unwrap_or(&s.poly[i]), t),
                        near_start: along < total * 0.5,
                    };
                }
            }
        }
        for piece in doc.pieces.iter().rev() {
            if piece.contains(p) {
                return Hit::Piece(piece.id);
            }
        }
        Hit::None
    }

    fn view_ui(&mut self, ui: &mut egui::Ui, doc: &mut Doc, rect: Rect, vi: usize) {
        let id = egui::Id::unique(("schnitt-ansicht", vi));
        let response = ui.interact(rect, id, Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        let view = self.views[vi];
        let tol = 8.0 / view.zoom;
        let hover_cm = response.hover_pos().map(|p| view.to_cm(rect, p));
        let ctx = ui.ctx().clone();

        // Zoomen zur Maus hin
        if let Some(hp) = response.hover_pos() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                let before = view.to_cm(rect, hp);
                let v = &mut self.views[vi];
                v.zoom = (v.zoom * (scroll * 0.002).exp()).clamp(0.3, 80.0);
                let after = v.to_px(rect, before);
                v.offset += hp - after;
            }
        }

        let space = ui.input(|i| i.key_down(Key::Space));
        let points_everywhere = matches!(self.tool, Tool::EditPoints | Tool::AddPoint);

        // --- Ziehen beginnt
        if response.drag_started() {
            // Ab dem Klickpunkt, nicht ab der Stelle, an der egui das Ziehen erkannt hat
            let press = ui
                .input(|i| i.pointer.press_origin())
                .or(response.interact_pointer_pos())
                .map(|p| self.views[vi].to_cm(rect, p));
            let button_pan = ui.input(|i| i.pointer.middle_down() || i.pointer.secondary_down());
            if button_pan || space {
                self.drag = Some(Drag::Pan);
            } else if let Some(pc) = press {
                let hit = self.hit(doc, pc, tol, points_everywhere);
                self.drag = match self.tool {
                    Tool::Select => match hit {
                        Hit::Piece(id) | Hit::Segment { seg: SegRef { piece: id, .. }, .. } | Hit::Point { piece: id, .. } => {
                            self.sel = Sel::Piece(id);
                            self.checkpoint(doc);
                            Some(Drag::Piece { id, last: pc })
                        }
                        Hit::None => None,
                    },
                    Tool::EditPoints => match hit {
                        Hit::Point { piece, pt } => {
                            self.sel = Sel::Point { piece, pt };
                            self.checkpoint(doc);
                            Some(Drag::Point { piece, pt })
                        }
                        Hit::Segment { seg, .. } => {
                            self.sel = Sel::Segment(seg);
                            self.checkpoint(doc);
                            Some(Drag::Segment { seg, last: pc })
                        }
                        Hit::Piece(id) => {
                            self.sel = Sel::Piece(id);
                            self.checkpoint(doc);
                            Some(Drag::Piece { id, last: pc })
                        }
                        Hit::None => None,
                    },
                    Tool::Rect | Tool::Ellipse => Some(Drag::Box { from: self.snap_pt(pc, &ctx) }),
                    _ => None,
                };
            }
        }

        // --- Ziehen läuft
        if response.dragged() {
            let delta = response.drag_delta();
            let cur = response.interact_pointer_pos().map(|p| self.views[vi].to_cm(rect, p));
            match self.drag {
                Some(Drag::Pan) => self.views[vi].offset += delta,
                Some(Drag::Piece { id, last }) => {
                    if let Some(c) = cur {
                        let d = c - last;
                        if let Some(p) = doc.piece_mut(id) {
                            p.translate(d);
                        }
                        doc.sync_partner(id);
                        self.drag = Some(Drag::Piece { id, last: c });
                    }
                }
                Some(Drag::Point { piece, pt }) => {
                    if let Some(c) = cur {
                        let c = self.snap_pt(c, &ctx);
                        if let Some(p) = doc.piece_mut(piece) {
                            if let Some(i) = p.index_of(pt) {
                                p.pts[i].p = c;
                            }
                        }
                        doc.sync_partner(piece);
                    }
                }
                Some(Drag::Segment { seg, last }) => {
                    if let Some(c) = cur {
                        let d = c - last;
                        if let Some(p) = doc.piece_mut(seg.piece) {
                            if let Some(i) = p.index_of(seg.start) {
                                let n = p.pts.len();
                                // Anfangsecke, Kurvenpunkte, Endecke
                                let mut j = i;
                                loop {
                                    p.pts[j].p += d;
                                    if j != i && !p.pts[j].curve {
                                        break;
                                    }
                                    j = (j + 1) % n;
                                    if j == i {
                                        break;
                                    }
                                }
                            }
                        }
                        doc.sync_partner(seg.piece);
                        self.drag = Some(Drag::Segment { seg, last: c });
                    }
                }
                _ => {}
            }
        }

        // --- Ziehen endet
        if response.drag_stopped() {
            if let (Some(Drag::Box { from }), Some(to)) =
                (self.drag, response.interact_pointer_pos().map(|p| self.views[vi].to_cm(rect, p)))
            {
                let mut to = self.snap_pt(to, &ctx);
                let shift = ui.input(|i| i.modifiers.shift);
                if (to - from).abs().min_element() >= 1.0 {
                    self.checkpoint(doc);
                    let name = doc.next_name();
                    let id = if self.tool == Tool::Rect {
                        doc.add_piece(
                            &name,
                            &[
                                (from, false),
                                (Vec2::new(to.x, from.y), false),
                                (to, false),
                                (Vec2::new(from.x, to.y), false),
                            ],
                        )
                    } else {
                        if shift {
                            let s = (to - from).abs().max_element();
                            to = from + (to - from).signum() * s;
                        }
                        doc.add_piece(&name, &circle_points((from + to) * 0.5, (to - from).abs() * 0.5))
                    };
                    self.sel = Sel::Piece(id);
                }
            }
            self.drag = None;
        }

        // --- Klicks
        if response.clicked() {
            if let Some(pc) = response.interact_pointer_pos().map(|p| self.views[vi].to_cm(rect, p)) {
                self.click(doc, pc, tol, &ctx, response.double_clicked());
            }
        }
        if response.secondary_clicked() {
            if let Some(pc) = response.interact_pointer_pos().map(|p| self.views[vi].to_cm(rect, p)) {
                self.ctx_hit = self.hit(doc, pc, tol, true);
                match self.ctx_hit {
                    Hit::Point { piece, pt } => self.sel = Sel::Point { piece, pt },
                    Hit::Segment { seg, .. } => self.sel = Sel::Segment(seg),
                    Hit::Piece(id) => self.sel = Sel::Piece(id),
                    Hit::None => {}
                }
            }
        }
        response.context_menu(|ui| self.context_menu(ui, doc));

        // ---------------------------------------------------------- Zeichnen
        let view = self.views[vi];
        painter.rect_filled(rect, 0.0, Color32::from_rgb(26, 28, 32));
        if let Some((tex, min, max)) = &self.silhouette {
            let r = Rect::from_min_max(view.to_px(rect, Vec2::new(min.x, max.y)), view.to_px(rect, Vec2::new(max.x, min.y)));
            painter.image(
                tex.id(),
                r,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::from_white_alpha(70),
            );
        }
        draw_grid(&painter, rect, &view);

        let hover_hit = hover_cm.map(|c| self.hit(doc, c, tol, points_everywhere)).unwrap_or(Hit::None);
        self.draw_pieces(&painter, rect, &view, doc, hover_hit);
        self.draw_preview(&painter, rect, &view, hover_cm, &ctx);

        // Mausposition und Werkzeughinweis
        if let Some(c) = hover_cm {
            painter.text(
                rect.left_top() + EVec2::new(8.0, 6.0),
                egui::Align2::LEFT_TOP,
                format!("x {:.1}  y {:.1} cm", c.x, c.y),
                egui::FontId::monospace(11.0),
                Color32::from_gray(150),
            );
        }
        if vi == 0 || !self.split {
            painter.text(
                rect.center_bottom() - EVec2::new(0.0, 8.0),
                egui::Align2::CENTER_BOTTOM,
                self.tool.help(),
                egui::FontId::proportional(12.0),
                Color32::from_gray(120),
            );
        }
        // Länge der Kante unter der Maus
        if let Hit::Segment { seg, .. } = hover_hit {
            if let (Some(len), Some(hp)) = (doc.segment_length(seg), response.hover_pos()) {
                painter.text(
                    hp + EVec2::new(14.0, -14.0),
                    egui::Align2::LEFT_BOTTOM,
                    format!("{len:.1} cm"),
                    egui::FontId::proportional(12.0),
                    Color32::from_rgb(255, 230, 150),
                );
            }
        }
    }

    fn click(&mut self, doc: &mut Doc, pc: Vec2, tol: f32, ctx: &egui::Context, double: bool) {
        let shift = ctx.input(|i| i.modifiers.shift);
        match self.tool {
            Tool::Select => {
                self.sel = match self.hit(doc, pc, tol, false) {
                    Hit::Piece(id) | Hit::Segment { seg: SegRef { piece: id, .. }, .. } | Hit::Point { piece: id, .. } => {
                        Sel::Piece(id)
                    }
                    Hit::None => Sel::None,
                };
            }
            Tool::EditPoints => {
                self.sel = match self.hit(doc, pc, tol, true) {
                    Hit::Point { piece, pt } => Sel::Point { piece, pt },
                    Hit::Segment { seg, .. } => Sel::Segment(seg),
                    Hit::Piece(id) => Sel::Piece(id),
                    Hit::None => Sel::None,
                };
            }
            Tool::AddPoint => {
                if let Hit::Segment { seg, at, .. } = self.hit(doc, pc, tol, false) {
                    self.add_point_on_segment(doc, seg, at);
                }
            }
            Tool::Polygon => {
                let p = self.snap_pt(pc, ctx);
                if self.drawing.len() >= 3 && self.drawing[0].0.distance(p) < tol * 1.5 {
                    self.finish_drawing(doc);
                } else if double && self.drawing.len() >= 3 {
                    self.finish_drawing(doc);
                } else {
                    self.drawing.push((p, shift));
                }
            }
            Tool::InternalLine => {
                let p = self.snap_pt(pc, ctx);
                if double && self.drawing.len() >= 2 {
                    self.finish_drawing(doc);
                } else if self.drawing.is_empty() && !doc.pieces.iter().any(|x| x.contains(p)) {
                    // Innenlinien gehören in ein Teil
                } else {
                    self.drawing.push((p, false));
                }
            }
            Tool::Seam => {
                if let Hit::Segment { seg, near_start, .. } = self.hit(doc, pc, tol, false) {
                    match self.seam_first {
                        None => self.seam_first = Some((seg, near_start)),
                        Some((first, _)) if first == seg => self.seam_first = None,
                        Some((first, first_start)) => {
                            self.checkpoint(doc);
                            let id = doc.new_id();
                            // Beide nahe am Anfang angeklickt → Anfang an Anfang
                            doc.seams.push(Seam { id, a: first, b: seg, flip: first_start != near_start });
                            self.seam_first = None;
                            self.sel = Sel::Seam(id);
                        }
                    }
                } else {
                    self.seam_first = None;
                }
            }
            Tool::Rect | Tool::Ellipse => {}
        }
    }

    fn add_point_on_segment(&mut self, doc: &mut Doc, seg: SegRef, at: Vec2) {
        self.checkpoint(doc);
        let pid = doc.new_id();
        if let Some(p) = doc.piece_mut(seg.piece) {
            if let Some(i) = p.index_of(seg.start) {
                // Hinter die Kurvenpunkte, die vor der Stelle liegen
                let n = p.pts.len();
                let mut after = i;
                let mut j = (i + 1) % n;
                while p.pts[j].curve && p.pts[j].p.distance(p.pts[i].p) < at.distance(p.pts[i].p) {
                    after = j;
                    j = (j + 1) % n;
                }
                p.insert_point(after, at, pid);
            }
        }
        doc.sync_partner(seg.piece);
        self.sel = Sel::Point { piece: seg.piece, pt: pid };
    }

    fn context_menu(&mut self, ui: &mut egui::Ui, doc: &mut Doc) {
        match self.ctx_hit {
            Hit::Point { piece, pt } => {
                let p = doc.piece(piece);
                let curve = p.and_then(|p| p.index_of(pt).map(|i| p.pts[i].curve)).unwrap_or(false);
                let can_delete = p.is_some_and(|p| p.pts.len() > 3);
                if ui.add_enabled(can_delete, egui::Button::new("Punkt löschen")).clicked() {
                    self.delete_point(doc, piece, pt);
                    ui.close();
                }
                if ui.add_enabled(!curve, egui::Button::new("Abnäher einfügen …")).clicked() {
                    self.dart = Some(DartDialog { at: ui.ctx().pointer_latest_pos(), piece, pt, width: 2.0, length: 7.0 });
                    ui.close();
                }
                let label = if curve { "In Eckpunkt umwandeln" } else { "In Kurvenpunkt umwandeln" };
                if ui.button(label).clicked() {
                    self.checkpoint(doc);
                    if let Some(p) = doc.piece_mut(piece) {
                        if let Some(i) = p.index_of(pt) {
                            p.pts[i].curve = !curve;
                        }
                    }
                    doc.sync_partner(piece);
                    doc.cleanup_seams();
                    ui.close();
                }
            }
            Hit::Segment { seg, at, .. } => {
                if ui.button("Punkt hier einfügen").clicked() {
                    self.add_point_on_segment(doc, seg, at);
                    ui.close();
                }
                if ui.button("Naht von dieser Kante …").clicked() {
                    self.set_tool(Tool::Seam);
                    self.seam_first = Some((seg, true));
                    ui.close();
                }
                self.piece_menu(ui, doc, seg.piece);
            }
            Hit::Piece(id) => self.piece_menu(ui, doc, id),
            Hit::None => {
                ui.label(egui::RichText::new("Hier ist nichts").weak());
            }
        }
    }

    fn piece_menu(&mut self, ui: &mut egui::Ui, doc: &mut Doc, id: u64) {
        ui.separator();
        let linked = doc.piece(id).is_some_and(|p| p.link.is_some());
        let is_mirror = doc.piece(id).is_some_and(|p| p.pts.iter().any(|x| x.id & MIRROR_BIT != 0));
        if ui.button("Duplizieren").clicked() {
            self.checkpoint(doc);
            if let Some(n) = doc.duplicate(id) {
                self.sel = Sel::Piece(n);
            }
            ui.close();
        }
        if ui
            .add_enabled(!linked && !is_mirror, egui::Button::new("Symmetrisch kopieren (verknüpft)"))
            .on_hover_text("Gespiegelte Kopie daneben; Änderungen an einem Teil gelten für beide")
            .clicked()
        {
            self.checkpoint(doc);
            if let Some(n) = doc.symmetric_copy(id) {
                self.sel = Sel::Piece(n);
            }
            ui.close();
        }
        if linked && ui.button("Verknüpfung lösen").clicked() {
            self.checkpoint(doc);
            doc.unlink(id);
            ui.close();
        }
        if ui.add_enabled(!linked, egui::Button::new("Horizontal spiegeln")).clicked() {
            self.checkpoint(doc);
            if let Some(p) = doc.piece_mut(id) {
                p.mirror_horizontal();
            }
            doc.cleanup_seams();
            ui.close();
        }
        if ui.add_enabled(!linked, egui::Button::new("90° drehen")).clicked() {
            self.checkpoint(doc);
            if let Some(p) = doc.piece_mut(id) {
                p.rotate(std::f32::consts::FRAC_PI_2);
            }
            ui.close();
        }
        ui.separator();
        if ui.button("Schnittteil löschen").clicked() {
            self.checkpoint(doc);
            doc.delete_piece(id);
            self.sel = Sel::None;
            ui.close();
        }
    }

    fn dart_dialog(&mut self, ctx: &egui::Context, doc: &mut Doc) {
        let Some(d) = &mut self.dart else { return };
        let mut open = true;
        let mut apply = false;
        let mut cancel = false;
        let mut win = egui::Window::new("Abnäher");
        if let Some(at) = d.at {
            win = win.default_pos(at + EVec2::new(16.0, 16.0));
        }
        win.collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                egui::Grid::new("dart").num_columns(2).show(ui, |ui| {
                    ui.label("Gesamtbreite");
                    ui.add(egui::DragValue::new(&mut d.width).range(0.1..=50.0).speed(0.1).suffix(" cm"));
                    ui.end_row();
                    ui.label("Länge");
                    ui.add(egui::DragValue::new(&mut d.length).range(0.1..=100.0).speed(0.1).suffix(" cm"));
                    ui.end_row();
                });
                ui.label(egui::RichText::new("Der Punkt wird zur Mitte des Abnähers, die Spitze zeigt ins Teil.").weak().small());
                ui.horizontal(|ui| {
                    apply = ui.button("OK").clicked();
                    cancel = ui.button("Abbrechen").clicked();
                });
            });
        if apply {
            let (piece, pt, w, l) = (d.piece, d.pt, d.width, d.length);
            self.checkpoint(doc);
            let ids = [doc.new_id(), doc.new_id(), doc.new_id()];
            if let Some(p) = doc.piece_mut(piece) {
                if let Some(i) = p.index_of(pt) {
                    p.insert_dart(i, w, l, ids);
                }
            }
            doc.sync_partner(piece);
            doc.cleanup_seams();
            self.sel = Sel::Point { piece, pt: ids[1] };
        }
        if apply || cancel || !open {
            self.dart = None;
        }
    }

    // ---------------------------------------------------------------- Darstellung

    fn draw_pieces(&self, painter: &egui::Painter, rect: Rect, view: &View, doc: &Doc, hover: Hit) {
        let px = |p: Vec2| view.to_px(rect, p);
        let sel_piece = match self.sel {
            Sel::Piece(id) | Sel::Point { piece: id, .. } => Some(id),
            Sel::Segment(s) => Some(s.piece),
            _ => None,
        };
        let show_all_points = matches!(self.tool, Tool::EditPoints | Tool::AddPoint);

        for piece in &doc.pieces {
            let outline: Vec<Pos2> = piece.outline().into_iter().map(px).collect();
            if outline.len() < 3 {
                continue;
            }
            let selected = sel_piece == Some(piece.id);
            let hovered = matches!(hover, Hit::Piece(id) if id == piece.id);
            let fill = if selected {
                Color32::from_rgb(214, 222, 236)
            } else if hovered {
                Color32::from_rgb(205, 208, 214)
            } else {
                Color32::from_rgb(190, 193, 198)
            };
            fill_polygon(painter, &outline, fill);

            for s in piece.segments() {
                let pts: Vec<Pos2> = s.poly.iter().map(|&p| px(p)).collect();
                let sref = SegRef { piece: piece.id, start: s.start_id };
                let is_sel = self.sel == Sel::Segment(sref) || self.seam_first.map(|f| f.0) == Some(sref);
                let is_hover = matches!(hover, Hit::Segment { seg, .. } if seg == sref);
                let stroke = if is_sel {
                    Stroke::new(3.0, Color32::from_rgb(255, 210, 60))
                } else if is_hover {
                    Stroke::new(2.5, Color32::from_rgb(255, 230, 140))
                } else {
                    Stroke::new(1.2, Color32::from_rgb(70, 90, 140))
                };
                painter.add(egui::Shape::line(pts, stroke));
            }

            for l in &piece.lines {
                let pts: Vec<Pos2> = l.pts.iter().map(|&p| px(p)).collect();
                painter.add(egui::Shape::line(pts, Stroke::new(1.5, Color32::from_rgb(60, 110, 220))));
            }

            // Name in der Mitte
            let (min, max) = piece.bounds();
            let label = if piece.link.is_some() { format!("{} (sym.)", piece.name) } else { piece.name.clone() };
            painter.text(
                px((min + max) * 0.5),
                egui::Align2::CENTER_CENTER,
                label,
                egui::FontId::proportional(12.0),
                Color32::from_gray(70),
            );

            if selected || show_all_points {
                for pt in &piece.pts {
                    let c = px(pt.p);
                    let is_sel = self.sel == (Sel::Point { piece: piece.id, pt: pt.id });
                    let is_hover = hover == (Hit::Point { piece: piece.id, pt: pt.id });
                    let color = if is_sel {
                        Color32::from_rgb(255, 120, 40)
                    } else if is_hover {
                        Color32::from_rgb(255, 200, 120)
                    } else {
                        Color32::from_rgb(40, 60, 120)
                    };
                    if pt.curve {
                        painter.circle(c, 3.5, Color32::WHITE, Stroke::new(1.5, color));
                    } else {
                        painter.rect_filled(Rect::from_center_size(c, EVec2::splat(6.0)), 0.0, color);
                    }
                }
                // Hilfslinien zu Kurvenpunkten
                let n = piece.pts.len();
                for i in 0..n {
                    let (a, b) = (piece.pts[i], piece.pts[(i + 1) % n]);
                    if a.curve || b.curve {
                        painter.line_segment([px(a.p), px(b.p)], Stroke::new(0.8, Color32::from_rgba_unmultiplied(40, 60, 120, 90)));
                    }
                }
            }
        }

        // Nähte: beide Kanten in derselben Farbe, Striche zeigen, welche Stellen zusammenkommen
        for (i, seam) in doc.seams.iter().enumerate() {
            let (Some(a), Some(b)) = (doc.segment(seam.a), doc.segment(seam.b)) else { continue };
            let color = seam_color(i);
            let width = if self.sel == Sel::Seam(seam.id) { 4.0 } else { 2.5 };
            for poly in [&a.poly, &b.poly] {
                painter.add(egui::Shape::line(poly.iter().map(|&p| px(p)).collect(), Stroke::new(width, color)));
            }
            for k in 1..4 {
                let t = k as f32 / 4.0;
                let pa = point_at(&a.poly, t);
                let pb = point_at(&b.poly, if seam.flip { 1.0 - t } else { t });
                painter.line_segment(
                    [px(pa), px(pb)],
                    Stroke::new(1.0, Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 70)),
                );
            }
        }
    }

    fn draw_preview(&self, painter: &egui::Painter, rect: Rect, view: &View, hover: Option<Vec2>, ctx: &egui::Context) {
        let px = |p: Vec2| view.to_px(rect, p);
        let pen = Stroke::new(1.5, Color32::from_rgb(255, 170, 60));
        if !self.drawing.is_empty() {
            let mut pts: Vec<Pos2> = self.drawing.iter().map(|d| px(d.0)).collect();
            let cursor = hover.map(|h| self.snap_pt(h, ctx));
            if let Some(c) = cursor {
                pts.push(px(c));
                let last = self.drawing.last().unwrap().0;
                painter.text(
                    px(c) + EVec2::new(12.0, -12.0),
                    egui::Align2::LEFT_BOTTOM,
                    format!("{:.1} cm", last.distance(c)),
                    egui::FontId::proportional(12.0),
                    Color32::from_rgb(255, 230, 150),
                );
            }
            painter.add(egui::Shape::line(pts, pen));
            for d in &self.drawing {
                if d.1 {
                    painter.circle_stroke(px(d.0), 3.5, pen);
                } else {
                    painter.rect_filled(Rect::from_center_size(px(d.0), EVec2::splat(6.0)), 0.0, pen.color);
                }
            }
        }
        if let (Some(Drag::Box { from }), Some(h)) = (self.drag, hover) {
            let to = self.snap_pt(h, ctx);
            let r = Rect::from_two_pos(px(from), px(to));
            if self.tool == Tool::Ellipse {
                let c = r.center();
                let rad = r.size() * 0.5;
                let pts: Vec<Pos2> = (0..=48)
                    .map(|i| {
                        let a = i as f32 / 48.0 * std::f32::consts::TAU;
                        c + EVec2::new(a.cos() * rad.x, a.sin() * rad.y)
                    })
                    .collect();
                painter.add(egui::Shape::line(pts, pen));
            } else {
                painter.rect_stroke(r, 0.0, pen, egui::StrokeKind::Middle);
            }
            painter.text(
                r.right_bottom() + EVec2::new(8.0, 4.0),
                egui::Align2::LEFT_TOP,
                format!("{:.1} × {:.1} cm", (to.x - from.x).abs(), (to.y - from.y).abs()),
                egui::FontId::proportional(12.0),
                Color32::from_rgb(255, 230, 150),
            );
        }
    }

    // ---------------------------------------------------------------- Seitenleiste

    /// Liste der Schnittteile und Nähte (rechts unter „Objekte“)
    pub fn objects_ui(&mut self, ui: &mut egui::Ui, doc: &Doc) {
        ui.label(egui::RichText::new("Schnittteile").weak().small());
        if doc.pieces.is_empty() {
            ui.label(egui::RichText::new("noch keine – Rechteck, Kreis oder Polygon zeichnen").weak());
        }
        for p in &doc.pieces {
            let selected = matches!(self.sel, Sel::Piece(id) | Sel::Point { piece: id, .. } if id == p.id);
            if ui.selectable_label(selected, &p.name).clicked() {
                self.sel = Sel::Piece(p.id);
            }
        }
        if !doc.seams.is_empty() {
            ui.add_space(6.0);
            ui.label(egui::RichText::new("Nähte").weak().small());
            for (i, s) in doc.seams.iter().enumerate() {
                let name = |r: SegRef| doc.piece(r.piece).map(|p| p.name.clone()).unwrap_or_default();
                let text = egui::RichText::new(format!("{} – {}", name(s.a), name(s.b))).color(seam_color(i));
                if ui.selectable_label(self.sel == Sel::Seam(s.id), text).clicked() {
                    self.sel = Sel::Seam(s.id);
                }
            }
        }
    }

    /// Eigenschaften der Auswahl; gibt false zurück, wenn nichts aus dem Schnitt ausgewählt ist
    pub fn properties_ui(&mut self, ui: &mut egui::Ui, doc: &mut Doc) -> bool {
        match self.sel {
            Sel::Piece(id) => {
                let Some(p) = doc.piece(id).cloned() else { return false };
                let mut name = p.name.clone();
                ui.horizontal(|ui| {
                    ui.label("Name");
                    if ui.text_edit_singleline(&mut name).changed() {
                        if let Some(pm) = doc.piece_mut(id) {
                            pm.name = name.clone();
                        }
                        self.dirty = true;
                    }
                });
                let (min, max) = p.bounds();
                egui::Grid::new("piece-props").num_columns(2).spacing([12.0, 4.0]).show(ui, |ui| {
                    ui.label("Breite");
                    ui.label(format!("{:.1} cm", max.x - min.x));
                    ui.end_row();
                    ui.label("Höhe");
                    ui.label(format!("{:.1} cm", max.y - min.y));
                    ui.end_row();
                    ui.label("Umfang");
                    ui.label(format!("{:.1} cm", p.perimeter()));
                    ui.end_row();
                    ui.label("Fläche");
                    ui.label(format!("{:.0} cm²", p.area()));
                    ui.end_row();
                    ui.label("Punkte");
                    ui.label(p.pts.len().to_string());
                    ui.end_row();
                });
                if let Some(link) = p.link {
                    let partner = doc.piece(link.partner).map(|x| x.name.clone()).unwrap_or_default();
                    ui.label(egui::RichText::new(format!("Symmetrisch verknüpft mit {partner}")).small());
                }
                ui.add_space(6.0);
                ui.label(egui::RichText::new("Kanten").weak().small());
                for (k, s) in p.segments().iter().enumerate() {
                    let r = SegRef { piece: id, start: s.start_id };
                    let sewn = doc.seams.iter().position(|x| x.a == r || x.b == r);
                    let mut text = egui::RichText::new(format!("Kante {}: {:.1} cm", k + 1, poly_len(&s.poly)));
                    if let Some(i) = sewn {
                        text = text.color(seam_color(i));
                    }
                    if ui.selectable_label(self.sel == Sel::Segment(r), text).clicked() {
                        self.sel = Sel::Segment(r);
                    }
                }
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    if ui.small_button("Duplizieren").clicked() {
                        self.checkpoint(doc);
                        if let Some(n) = doc.duplicate(id) {
                            self.sel = Sel::Piece(n);
                        }
                    }
                    if p.link.is_none() && ui.small_button("Symmetrisch kopieren").clicked() {
                        self.checkpoint(doc);
                        if let Some(n) = doc.symmetric_copy(id) {
                            self.sel = Sel::Piece(n);
                        }
                    }
                    if ui.small_button("Löschen").clicked() {
                        self.checkpoint(doc);
                        doc.delete_piece(id);
                        self.sel = Sel::None;
                    }
                });
                true
            }
            Sel::Point { piece, pt } => {
                let Some(p) = doc.piece(piece) else { return false };
                let Some(i) = p.index_of(pt) else { return false };
                let mut pos = p.pts[i].p;
                let mut curve = p.pts[i].curve;
                let name = p.name.clone();
                ui.label(format!("Punkt an {name}"));
                let mut changed = false;
                let mut started = false;
                egui::Grid::new("pt-props").num_columns(2).show(ui, |ui| {
                    ui.label("X");
                    let r = ui.add(egui::DragValue::new(&mut pos.x).speed(0.1).suffix(" cm"));
                    started |= r.drag_started() || r.gained_focus();
                    changed |= r.changed();
                    ui.end_row();
                    ui.label("Y");
                    let r = ui.add(egui::DragValue::new(&mut pos.y).speed(0.1).suffix(" cm"));
                    started |= r.drag_started() || r.gained_focus();
                    changed |= r.changed();
                    ui.end_row();
                });
                let curve_changed = ui.checkbox(&mut curve, "Kurvenpunkt").changed();
                if started || curve_changed {
                    self.checkpoint(doc);
                }
                if changed || curve_changed {
                    if let Some(pm) = doc.piece_mut(piece) {
                        pm.pts[i].p = pos;
                        pm.pts[i].curve = curve;
                    }
                    doc.sync_partner(piece);
                    doc.cleanup_seams();
                    self.dirty = true;
                }
                ui.horizontal(|ui| {
                    if !curve && ui.small_button("Abnäher …").clicked() {
                        self.dart = Some(DartDialog { at: ui.ctx().pointer_latest_pos(), piece, pt, width: 2.0, length: 7.0 });
                    }
                    if ui.small_button("Löschen").clicked() {
                        self.delete_point(doc, piece, pt);
                    }
                });
                true
            }
            Sel::Segment(s) => {
                let Some(len) = doc.segment_length(s) else { return false };
                let name = doc.piece(s.piece).map(|p| p.name.clone()).unwrap_or_default();
                ui.label(format!("Kante an {name}"));
                ui.label(format!("Länge {len:.1} cm"));
                if ui.small_button("Naht von dieser Kante …").clicked() {
                    self.set_tool(Tool::Seam);
                    self.seam_first = Some((s, true));
                }
                true
            }
            Sel::Seam(id) => {
                let Some(seam) = doc.seams.iter().find(|x| x.id == id).copied() else { return false };
                let la = doc.segment_length(seam.a).unwrap_or(0.0);
                let lb = doc.segment_length(seam.b).unwrap_or(0.0);
                let name = |r: SegRef| doc.piece(r.piece).map(|p| p.name.clone()).unwrap_or_default();
                ui.label(format!("Naht {} – {}", name(seam.a), name(seam.b)));
                egui::Grid::new("seam-props").num_columns(2).show(ui, |ui| {
                    ui.label(name(seam.a));
                    ui.label(format!("{la:.1} cm"));
                    ui.end_row();
                    ui.label(name(seam.b));
                    ui.label(format!("{lb:.1} cm"));
                    ui.end_row();
                });
                let diff = (la - lb).abs();
                if diff > 0.5 {
                    ui.label(
                        egui::RichText::new(format!("Längen weichen um {diff:.1} cm ab – der Stoff wird dort gerafft"))
                            .color(Color32::from_rgb(230, 170, 80))
                            .small(),
                    );
                }
                ui.horizontal(|ui| {
                    if ui.small_button("Richtung umkehren").clicked() {
                        self.checkpoint(doc);
                        if let Some(sm) = doc.seams.iter_mut().find(|x| x.id == id) {
                            sm.flip = !sm.flip;
                        }
                    }
                    if ui.small_button("Löschen").clicked() {
                        self.checkpoint(doc);
                        doc.seams.retain(|x| x.id != id);
                        self.sel = Sel::None;
                    }
                });
                true
            }
            Sel::None => false,
        }
    }
}

fn point_at(poly: &[Vec2], t: f32) -> Vec2 {
    let total = poly_len(poly);
    let mut rest = total * t;
    for w in poly.windows(2) {
        let l = w[0].distance(w[1]);
        if rest <= l && l > 0.0 {
            return w[0].lerp(w[1], rest / l);
        }
        rest -= l;
    }
    *poly.last().unwrap_or(&Vec2::ZERO)
}

/// Füllt ein beliebiges (auch nicht konvexes) Polygon über Zerlegung in Dreiecke
fn fill_polygon(painter: &egui::Painter, pts: &[Pos2], color: Color32) {
    let tris = triangulate(pts);
    let mut mesh = egui::Mesh::default();
    for p in pts {
        mesh.colored_vertex(*p, color);
    }
    for t in tris.chunks_exact(3) {
        mesh.add_triangle(t[0], t[1], t[2]);
    }
    painter.add(egui::Shape::mesh(mesh));
}

/// Ohrenschneiden: genügt für Schnittteile (einfache Polygone ohne Löcher)
pub fn triangulate(pts: &[Pos2]) -> Vec<u32> {
    let n = pts.len();
    if n < 3 {
        return Vec::new();
    }
    let area: f32 = (0..n).map(|i| {
        let (a, b) = (pts[i], pts[(i + 1) % n]);
        a.x * b.y - b.x * a.y
    }).sum();
    let mut idx: Vec<usize> = (0..n).collect();
    if area < 0.0 {
        idx.reverse();
    }
    let cross = |a: Pos2, b: Pos2, c: Pos2| (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
    let mut out = Vec::new();
    let mut guard = 0;
    while idx.len() > 3 && guard < n * n {
        guard += 1;
        let m = idx.len();
        let mut cut = None;
        for k in 0..m {
            let (ia, ib, ic) = (idx[(k + m - 1) % m], idx[k], idx[(k + 1) % m]);
            let (a, b, c) = (pts[ia], pts[ib], pts[ic]);
            if cross(a, b, c) <= 0.0 {
                continue;
            }
            let inside = idx.iter().any(|&j| {
                if j == ia || j == ib || j == ic {
                    return false;
                }
                let p = pts[j];
                cross(a, b, p) > 0.0 && cross(b, c, p) > 0.0 && cross(c, a, p) > 0.0
            });
            if !inside {
                cut = Some(k);
                break;
            }
        }
        let k = cut.unwrap_or(0);
        let m = idx.len();
        out.extend([idx[(k + m - 1) % m] as u32, idx[k] as u32, idx[(k + 1) % m] as u32]);
        idx.remove(k);
    }
    if idx.len() == 3 {
        out.extend(idx.iter().map(|&i| i as u32));
    }
    out
}

fn fill_triangle(mask: &mut [bool], w: usize, h: usize, a: Vec2, b: Vec2, c: Vec2) {
    let min = a.min(b).min(c).max(Vec2::ZERO);
    let max = a.max(b).max(c).min(Vec2::new(w as f32 - 1.0, h as f32 - 1.0));
    if min.x > max.x || min.y > max.y {
        return;
    }
    let area = (b - a).perp_dot(c - a);
    if area.abs() < 1e-6 {
        return;
    }
    for y in min.y as usize..=max.y as usize {
        for x in min.x as usize..=max.x as usize {
            let p = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
            let w0 = (b - p).perp_dot(c - p) / area;
            let w1 = (c - p).perp_dot(a - p) / area;
            let w2 = 1.0 - w0 - w1;
            if w0 >= -1e-4 && w1 >= -1e-4 && w2 >= -1e-4 {
                mask[y * w + x] = true;
            }
        }
    }
}

fn draw_grid(painter: &egui::Painter, rect: Rect, view: &View) {
    let steps = [0.5f32, 1.0, 5.0, 10.0, 50.0, 100.0];
    let minor = steps.iter().copied().find(|s| s * view.zoom >= 8.0).unwrap_or(100.0);
    let major = minor * if minor == 0.5 || minor == 5.0 || minor == 50.0 { 2.0 } else { 10.0 };
    let a = view.to_cm(rect, rect.left_bottom());
    let b = view.to_cm(rect, rect.right_top());
    let line = |cm: f32, vertical: bool| {
        let is_axis = cm.abs() < minor * 0.01;
        let is_major = ((cm / major).round() * major - cm).abs() < minor * 0.01;
        let color = if is_axis {
            Color32::from_gray(95)
        } else if is_major {
            Color32::from_gray(50)
        } else {
            Color32::from_gray(35)
        };
        let s = Stroke::new(1.0, color);
        if vertical {
            let x = view.to_px(rect, Vec2::new(cm, 0.0)).x;
            painter.vline(x, rect.y_range(), s);
        } else {
            let y = view.to_px(rect, Vec2::new(0.0, cm)).y;
            painter.hline(rect.x_range(), y, s);
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
        rect.right_top() + EVec2::new(-8.0, 6.0),
        egui::Align2::RIGHT_TOP,
        format!("Raster {minor} cm"),
        egui::FontId::proportional(11.0),
        Color32::from_gray(110),
    );
}
