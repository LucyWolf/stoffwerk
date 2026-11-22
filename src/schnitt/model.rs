//! Datenmodell des Schnitts: Schnittteile aus Eck- und Kurvenpunkten, Innenlinien, Nähte.
//! Alle Maße in Zentimetern, y nach oben. Die Lage im 2D-Fenster entspricht der Vorderansicht des
//! Avatars (x = Breite, y = Höhe über dem Boden) – das hilft später beim Anlegen am Körper.

use glam::Vec2;
use serde::{Deserialize, Serialize};

/// Punkt-IDs eines symmetrischen Partners: gleiche Nummer mit gesetztem Bit. Damit findet jeder Punkt
/// seinen Spiegelpunkt, auch nachdem Punkte eingefügt oder gelöscht wurden.
pub const MIRROR_BIT: u64 = 1 << 40;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Pt {
    pub id: u64,
    pub p: Vec2,
    /// Kurvenpunkt: zieht die Kante an, liegt aber nicht auf ihr (Bézier-Stützpunkt)
    pub curve: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InternalLine {
    pub id: u64,
    pub pts: Vec<Vec2>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Link {
    pub partner: u64,
    /// Spiegelachse (x-Wert) zwischen den beiden Teilen
    pub axis: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Piece {
    pub id: u64,
    pub name: String,
    /// Umriss gegen den Uhrzeigersinn
    pub pts: Vec<Pt>,
    #[serde(default)]
    pub lines: Vec<InternalLine>,
    /// Symmetrisch verknüpfter Partner: Änderungen an einem Teil erscheinen gespiegelt am anderen
    #[serde(default)]
    pub link: Option<Link>,
    /// Anfangslage für die Simulation
    #[serde(default)]
    pub placement: crate::sim::Placement,
    #[serde(default)]
    pub fabric: crate::sim::Fabric,
}

/// Eine Kante zwischen zwei Eckpunkten, gekennzeichnet durch die ID ihres Anfangspunkts
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct SegRef {
    pub piece: u64,
    pub start: u64,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Seam {
    pub id: u64,
    pub a: SegRef,
    pub b: SegRef,
    /// Kanten laufen entgegengesetzt zusammen (Anfang von a an Ende von b)
    pub flip: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Doc {
    pub pieces: Vec<Piece>,
    pub seams: Vec<Seam>,
    pub next_id: u64,
    /// Punktabstand des Simulationsnetzes in mm (Marvelous Designer: „Particle Distance“); 0 = Vorgabe 20
    #[serde(default)]
    pub particle_mm: f32,
}

/// Eine Kante zum Zeichnen und Messen
pub struct Segment {
    pub start_id: u64,
    /// Abgetastete Linie vom Anfangs- zum Endeckpunkt
    pub poly: Vec<Vec2>,
}

impl Doc {
    pub fn particle_cm(&self) -> f32 {
        if self.particle_mm > 0.0 { self.particle_mm * 0.1 } else { 2.0 }
    }

    /// Ändert sich, sobald sich an Form, Lage, Stoff oder Nähten etwas ändert – dann wird der Stoff neu aufgebaut
    pub fn sim_hash(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for p in &self.pieces {
            p.id.hash(&mut h);
            for pt in &p.pts {
                pt.id.hash(&mut h);
                pt.p.x.to_bits().hash(&mut h);
                pt.p.y.to_bits().hash(&mut h);
                pt.curve.hash(&mut h);
            }
            (p.placement as u8).hash(&mut h);
            (p.fabric as u8).hash(&mut h);
        }
        for s in &self.seams {
            (s.a, s.b, s.flip).hash(&mut h);
        }
        self.particle_mm.to_bits().hash(&mut h);
        h.finish()
    }

    pub fn new_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    pub fn piece(&self, id: u64) -> Option<&Piece> {
        self.pieces.iter().find(|p| p.id == id)
    }

    pub fn piece_mut(&mut self, id: u64) -> Option<&mut Piece> {
        self.pieces.iter_mut().find(|p| p.id == id)
    }

    pub fn add_piece(&mut self, name: &str, corners: &[(Vec2, bool)]) -> u64 {
        let id = self.new_id();
        let pts = corners.iter().map(|&(p, curve)| Pt { id: self.new_id(), p, curve }).collect();
        let mut piece = Piece { id, name: name.to_string(), pts, lines: Vec::new(), link: None, placement: Default::default(), fabric: Default::default() };
        piece.make_ccw();
        self.pieces.push(piece);
        id
    }

    pub fn next_name(&self) -> String {
        let mut n = self.pieces.len() + 1;
        loop {
            let name = format!("Teil {n}");
            if !self.pieces.iter().any(|p| p.name == name) {
                return name;
            }
            n += 1;
        }
    }

    pub fn delete_piece(&mut self, id: u64) {
        if let Some(partner) = self.piece(id).and_then(|p| p.link).map(|l| l.partner) {
            if let Some(p) = self.piece_mut(partner) {
                p.link = None;
            }
        }
        self.pieces.retain(|p| p.id != id);
        self.seams.retain(|s| s.a.piece != id && s.b.piece != id);
    }

    /// Kopie daneben, unabhängig
    pub fn duplicate(&mut self, id: u64) -> Option<u64> {
        let src = self.piece(id)?.clone();
        let (min, max) = src.bounds();
        let dx = Vec2::new(max.x - min.x + 5.0, 0.0);
        let new_id = self.new_id();
        let mut copy = src;
        copy.id = new_id;
        copy.name = format!("{} Kopie", copy.name);
        copy.link = None;
        for pt in &mut copy.pts {
            pt.id = self.next_id + 1;
            self.next_id += 1;
            pt.p += dx;
        }
        for l in &mut copy.lines {
            l.id = self.new_id();
            l.pts.iter_mut().for_each(|p| *p += dx);
        }
        self.pieces.push(copy);
        Some(new_id)
    }

    /// Spiegelbildliche Kopie rechts daneben, verknüpft: Änderungen gelten für beide
    pub fn symmetric_copy(&mut self, id: u64) -> Option<u64> {
        let src = self.piece(id)?;
        if src.link.is_some() || src.pts.iter().any(|p| p.id & MIRROR_BIT != 0) {
            return None;
        }
        let (_, max) = src.bounds();
        let axis = max.x + 2.5;
        let name = format!("{} (gespiegelt)", src.name);
        let new_id = self.new_id();
        self.pieces.push(Piece { id: new_id, name, pts: Vec::new(), lines: Vec::new(), link: None, placement: Default::default(), fabric: Default::default() });
        self.piece_mut(id)?.link = Some(Link { partner: new_id, axis });
        self.piece_mut(new_id)?.link = Some(Link { partner: id, axis });
        self.sync_partner(id);
        Some(new_id)
    }

    pub fn unlink(&mut self, id: u64) {
        if let Some(partner) = self.piece(id).and_then(|p| p.link).map(|l| l.partner) {
            for pid in [id, partner] {
                if let Some(p) = self.piece_mut(pid) {
                    p.link = None;
                }
            }
        }
    }

    /// Überträgt die Form eines Teils gespiegelt auf seinen verknüpften Partner
    pub fn sync_partner(&mut self, id: u64) {
        let Some(src) = self.piece(id).cloned() else { return };
        let Some(link) = src.link else { return };
        let mirror = |p: Vec2| Vec2::new(2.0 * link.axis - p.x, p.y);
        let Some(dst) = self.piece_mut(link.partner) else { return };
        // Gespiegelt kehrt sich der Umlaufsinn um → Reihenfolge umdrehen, damit er wieder stimmt
        dst.pts = src
            .pts
            .iter()
            .rev()
            .map(|pt| Pt { id: pt.id ^ MIRROR_BIT, p: mirror(pt.p), curve: pt.curve })
            .collect();
        dst.lines = src
            .lines
            .iter()
            .map(|l| InternalLine { id: l.id ^ MIRROR_BIT, pts: l.pts.iter().map(|&p| mirror(p)).collect() })
            .collect();
        // Gleicher Stoff und gleiche Lage wie das Original
        dst.fabric = src.fabric;
        dst.placement = src.placement;
        self.cleanup_seams();
    }

    /// Nähte entfernen, deren Kanten es nicht mehr gibt
    pub fn cleanup_seams(&mut self) {
        let exists = |d: &Doc, s: SegRef| {
            d.piece(s.piece).is_some_and(|p| p.pts.iter().any(|pt| pt.id == s.start && !pt.curve))
        };
        let keep: Vec<bool> = self.seams.iter().map(|s| exists(self, s.a) && exists(self, s.b)).collect();
        let mut it = keep.into_iter();
        self.seams.retain(|_| it.next().unwrap_or(false));
    }

    pub fn segment_length(&self, s: SegRef) -> Option<f32> {
        let piece = self.piece(s.piece)?;
        piece.segments().into_iter().find(|g| g.start_id == s.start).map(|g| poly_len(&g.poly))
    }

    pub fn segment(&self, s: SegRef) -> Option<Segment> {
        self.piece(s.piece)?.segments().into_iter().find(|g| g.start_id == s.start)
    }
}

pub fn poly_len(p: &[Vec2]) -> f32 {
    p.windows(2).map(|w| w[0].distance(w[1])).sum()
}

impl Piece {
    pub fn bounds(&self) -> (Vec2, Vec2) {
        let mut min = Vec2::splat(f32::MAX);
        let mut max = Vec2::splat(f32::MIN);
        for p in self.outline() {
            min = min.min(p);
            max = max.max(p);
        }
        (min, max)
    }

    pub fn signed_area(&self) -> f32 {
        let o = self.outline();
        let n = o.len();
        (0..n).map(|i| o[i].perp_dot(o[(i + 1) % n])).sum::<f32>() * 0.5
    }

    pub fn area(&self) -> f32 {
        self.signed_area().abs()
    }

    pub fn make_ccw(&mut self) {
        if self.pts.len() >= 3 && self.signed_area() < 0.0 {
            self.pts.reverse();
        }
    }

    pub fn translate(&mut self, d: Vec2) {
        self.pts.iter_mut().for_each(|p| p.p += d);
        self.lines.iter_mut().for_each(|l| l.pts.iter_mut().for_each(|p| *p += d));
    }

    /// Indizes der Eckpunkte; gibt es keinen (nur Kurvenpunkte), gilt der erste als Ecke
    fn corners(&self) -> Vec<usize> {
        let c: Vec<usize> = (0..self.pts.len()).filter(|&i| !self.pts[i].curve).collect();
        if c.is_empty() && !self.pts.is_empty() { vec![0] } else { c }
    }

    /// Kanten von Ecke zu Ecke; Kurvenpunkte dazwischen sind Bézier-Stützpunkte
    pub fn segments(&self) -> Vec<Segment> {
        let n = self.pts.len();
        let corners = self.corners();
        let mut out = Vec::new();
        for (k, &ci) in corners.iter().enumerate() {
            let next = corners[(k + 1) % corners.len()];
            let mut ctrl = vec![self.pts[ci].p];
            let mut j = (ci + 1) % n;
            while j != next {
                ctrl.push(self.pts[j].p);
                j = (j + 1) % n;
            }
            ctrl.push(self.pts[next].p);
            let poly = if ctrl.len() == 2 {
                ctrl
            } else {
                let steps = 12 * (ctrl.len() - 1);
                (0..=steps).map(|s| bezier(&ctrl, s as f32 / steps as f32)).collect()
            };
            out.push(Segment { start_id: self.pts[ci].id, poly });
        }
        out
    }

    /// Geschlossener Umriss als Linienzug (ohne doppelten Endpunkt)
    pub fn outline(&self) -> Vec<Vec2> {
        let mut o = Vec::new();
        for s in self.segments() {
            o.extend_from_slice(&s.poly[..s.poly.len() - 1]);
        }
        o
    }

    pub fn perimeter(&self) -> f32 {
        self.segments().iter().map(|s| poly_len(&s.poly)).sum()
    }

    pub fn contains(&self, p: Vec2) -> bool {
        point_in_polygon(p, &self.outline())
    }

    pub fn index_of(&self, id: u64) -> Option<usize> {
        self.pts.iter().position(|p| p.id == id)
    }

    /// Richtung der Kante an einem Punkt und Normale nach innen
    fn frame_at(&self, i: usize) -> (Vec2, Vec2) {
        let n = self.pts.len();
        let prev = self.pts[(i + n - 1) % n].p;
        let next = self.pts[(i + 1) % n].p;
        let t = (next - prev).normalize_or(Vec2::X);
        // Gegen den Uhrzeigersinn liegt das Innere links der Laufrichtung
        (t, t.perp())
    }

    /// Ersetzt einen Punkt durch einen Abnäher: zwei Fußpunkte auf der Kante und eine Spitze nach innen
    pub fn insert_dart(&mut self, i: usize, width: f32, length: f32, ids: [u64; 3]) {
        if i >= self.pts.len() || width <= 0.0 || length <= 0.0 {
            return;
        }
        let (t, n) = self.frame_at(i);
        let p = self.pts[i].p;
        let a = Pt { id: ids[0], p: p - t * width * 0.5, curve: false };
        let tip = Pt { id: ids[1], p: p + n * length, curve: false };
        let b = Pt { id: ids[2], p: p + t * width * 0.5, curve: false };
        self.pts.splice(i..=i, [a, tip, b]);
    }

    /// Fügt auf der Kante nach Punkt `after` einen Punkt ein
    pub fn insert_point(&mut self, after: usize, p: Vec2, id: u64) {
        let i = (after + 1).min(self.pts.len());
        self.pts.insert(i, Pt { id, p, curve: false });
    }

    pub fn mirror_horizontal(&mut self) {
        let (min, max) = self.bounds();
        let cx = (min.x + max.x) * 0.5;
        self.pts.iter_mut().for_each(|p| p.p.x = 2.0 * cx - p.p.x);
        self.lines.iter_mut().for_each(|l| l.pts.iter_mut().for_each(|p| p.x = 2.0 * cx - p.x));
        self.pts.reverse();
    }

    pub fn rotate(&mut self, angle: f32) {
        let (min, max) = self.bounds();
        let c = (min + max) * 0.5;
        let r = Vec2::from_angle(angle);
        self.pts.iter_mut().for_each(|p| p.p = c + r.rotate(p.p - c));
        self.lines.iter_mut().for_each(|l| l.pts.iter_mut().for_each(|p| *p = c + r.rotate(*p - c)));
    }
}

fn bezier(ctrl: &[Vec2], t: f32) -> Vec2 {
    let mut v = ctrl.to_vec();
    for k in (1..v.len()).rev() {
        for i in 0..k {
            v[i] = v[i].lerp(v[i + 1], t);
        }
    }
    v[0]
}

pub fn point_in_polygon(p: Vec2, poly: &[Vec2]) -> bool {
    let mut inside = false;
    let n = poly.len();
    let mut j = n.wrapping_sub(1);
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Abstand eines Punkts zu einem Linienzug, dazu die Stelle (Index des Teilstücks, Anteil darauf)
pub fn dist_to_poly(p: Vec2, poly: &[Vec2]) -> (f32, usize, f32) {
    let mut best = (f32::MAX, 0, 0.0);
    for (i, w) in poly.windows(2).enumerate() {
        let d = w[1] - w[0];
        let t = if d.length_squared() > 0.0 { ((p - w[0]).dot(d) / d.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
        let dist = p.distance(w[0] + d * t);
        if dist < best.0 {
            best = (dist, i, t);
        }
    }
    best
}

/// Kreis aus vier kubischen Bézier-Bögen (je zwei Kurvenpunkte), Abweichung unter 0,03 %
pub fn circle_points(c: Vec2, r: Vec2) -> Vec<(Vec2, bool)> {
    let k = 0.552_284_8;
    let mut out = Vec::new();
    for q in 0..4 {
        let a0 = q as f32 * std::f32::consts::FRAC_PI_2;
        let a1 = a0 + std::f32::consts::FRAC_PI_2;
        let p0 = Vec2::new(a0.cos(), a0.sin());
        let p1 = Vec2::new(a1.cos(), a1.sin());
        out.push((c + p0 * r, false));
        out.push((c + (p0 + p0.perp() * k) * r, true));
        out.push((c + (p1 - p1.perp() * k) * r, true));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(doc: &mut Doc) -> u64 {
        doc.add_piece(
            "R",
            &[
                (Vec2::new(0.0, 0.0), false),
                (Vec2::new(40.0, 0.0), false),
                (Vec2::new(40.0, 60.0), false),
                (Vec2::new(0.0, 60.0), false),
            ],
        )
    }

    #[test]
    fn rechteck_masse() {
        let mut d = Doc::default();
        let id = rect(&mut d);
        let p = d.piece(id).unwrap();
        assert!((p.area() - 2400.0).abs() < 0.01);
        assert!((p.perimeter() - 200.0).abs() < 0.01);
        assert_eq!(p.segments().len(), 4);
    }

    #[test]
    fn im_uhrzeigersinn_wird_umgedreht() {
        let mut d = Doc::default();
        let id = d.add_piece(
            "R",
            &[(Vec2::ZERO, false), (Vec2::new(0.0, 10.0), false), (Vec2::new(10.0, 10.0), false), (Vec2::new(10.0, 0.0), false)],
        );
        assert!(d.piece(id).unwrap().signed_area() > 0.0);
    }

    #[test]
    fn kreis_umfang() {
        let mut d = Doc::default();
        let id = d.add_piece("K", &circle_points(Vec2::ZERO, Vec2::splat(10.0)));
        let u = d.piece(id).unwrap().perimeter();
        assert!((u - 2.0 * std::f32::consts::PI * 10.0).abs() < 0.1, "{u}");
        assert_eq!(d.piece(id).unwrap().segments().len(), 4);
    }

    #[test]
    fn abnaeher_zeigt_nach_innen() {
        let mut d = Doc::default();
        let id = rect(&mut d);
        // Punkt mitten auf die Unterkante, dann dort einen Abnäher
        let pid = d.new_id();
        let ids = [d.new_id(), d.new_id(), d.new_id()];
        let p = d.piece_mut(id).unwrap();
        p.insert_point(0, Vec2::new(20.0, 0.0), pid);
        let i = p.index_of(pid).unwrap();
        p.insert_dart(i, 4.0, 10.0, ids);
        let tip = p.pts[p.index_of(ids[1]).unwrap()].p;
        assert!((tip - Vec2::new(20.0, 10.0)).length() < 1e-4, "{tip}");
        assert!((p.pts[p.index_of(ids[0]).unwrap()].p - Vec2::new(18.0, 0.0)).length() < 1e-4);
        // Fläche verliert das Dreieck des Abnähers
        assert!((p.area() - (2400.0 - 20.0)).abs() < 0.01, "{}", p.area());
    }

    #[test]
    fn symmetrische_kopie_folgt() {
        let mut d = Doc::default();
        let id = rect(&mut d);
        let other = d.symmetric_copy(id).unwrap();
        assert!(d.piece(other).unwrap().signed_area() > 0.0);
        d.piece_mut(id).unwrap().pts[2].p.y = 70.0; // obere rechte Ecke hochziehen
        d.sync_partner(id);
        let o = d.piece(other).unwrap();
        let (_, max) = o.bounds();
        assert!((max.y - 70.0).abs() < 1e-4);
        // Spiegelpunkt über die ID wiederfinden
        let mp = o.pts.iter().find(|p| p.id == d.piece(id).unwrap().pts[2].id ^ MIRROR_BIT).unwrap();
        assert!((mp.p.y - 70.0).abs() < 1e-4);
    }

    #[test]
    fn naht_verschwindet_mit_der_kante() {
        let mut d = Doc::default();
        let a = rect(&mut d);
        let b = rect(&mut d);
        let sa = d.piece(a).unwrap().pts[1].id;
        let sb = d.piece(b).unwrap().pts[3].id;
        let sid = d.new_id();
        d.seams.push(Seam { id: sid, a: SegRef { piece: a, start: sa }, b: SegRef { piece: b, start: sb }, flip: true });
        assert!((d.segment_length(d.seams[0].a).unwrap() - 60.0).abs() < 1e-4);
        d.piece_mut(a).unwrap().pts.remove(1);
        d.cleanup_seams();
        assert!(d.seams.is_empty());
    }
}
