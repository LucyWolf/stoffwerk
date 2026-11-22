//! Stoffsimulation: Schnittteile vernetzen, am Avatar anlegen, mit XPBD simulieren.
//!
//! XPBD (extended position based dynamics) löst Dehnung, Biegung und Nähte als Abstandsbedingungen in
//! kleinen Zeitschritten. Das bleibt auch bei großen Kräften (Nähte, die 30 cm zusammenziehen) stabil.
//! Längen in Metern, Zeit in Sekunden.

mod collide;
mod mesh;

pub use collide::Collider;
use glam::{Vec2, Vec3};
use serde::{Deserialize, Serialize};

use crate::schnitt::model::Doc;

/// Wo ein Schnittteil zu Beginn liegt
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Placement {
    /// Senkrecht vor dem Avatar, so wie gezeichnet
    #[default]
    Front,
    /// Senkrecht hinter dem Avatar, von hinten gesehen so wie gezeichnet
    Back,
    /// Waagerecht über dem Avatar, zum Fallenlassen
    Above,
}

impl Placement {
    pub const ALL: [Placement; 3] = [Placement::Front, Placement::Back, Placement::Above];
    pub fn label(self) -> &'static str {
        match self {
            Placement::Front => "Vorne",
            Placement::Back => "Hinten",
            Placement::Above => "Oben (fallen lassen)",
        }
    }
}

/// Stoffart: Gewicht, Steifigkeit, Reibung
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Fabric {
    #[default]
    Cotton,
    Silk,
    Denim,
    Knit,
    Leather,
}

pub struct FabricParams {
    /// Flächengewicht in kg/m²
    pub density: f32,
    /// Nachgiebigkeit gegen Dehnung (XPBD-Compliance, m/N)
    pub stretch: f32,
    /// Nachgiebigkeit gegen Biegung
    pub bend: f32,
    pub friction: f32,
    pub color: [f32; 3],
}

impl Fabric {
    pub const ALL: [Fabric; 5] = [Fabric::Cotton, Fabric::Silk, Fabric::Denim, Fabric::Knit, Fabric::Leather];

    pub fn label(self) -> &'static str {
        match self {
            Fabric::Cotton => "Baumwolle",
            Fabric::Silk => "Seide",
            Fabric::Denim => "Jeans",
            Fabric::Knit => "Strick (dehnbar)",
            Fabric::Leather => "Leder",
        }
    }

    pub fn params(self) -> FabricParams {
        match self {
            Fabric::Cotton => FabricParams { density: 0.15, stretch: 1e-7, bend: 1e-3, friction: 0.3, color: [0.95, 0.95, 0.96] },
            Fabric::Silk => FabricParams { density: 0.07, stretch: 1e-7, bend: 6e-2, friction: 0.15, color: [0.97, 0.94, 0.9] },
            Fabric::Denim => FabricParams { density: 0.35, stretch: 2e-8, bend: 5e-4, friction: 0.5, color: [0.55, 0.65, 0.82] },
            Fabric::Knit => FabricParams { density: 0.2, stretch: 4e-5, bend: 2e-2, friction: 0.5, color: [0.92, 0.9, 0.95] },
            Fabric::Leather => FabricParams { density: 0.8, stretch: 1e-8, bend: 1e-4, friction: 0.6, color: [0.45, 0.32, 0.24] },
        }
    }
}

/// Lage des Avatars, an der sich das Anlegen orientiert
#[derive(Clone, Copy)]
pub struct Body {
    pub min: Vec3,
    pub max: Vec3,
}

impl Default for Body {
    fn default() -> Self {
        // Ohne Avatar: gedachter Mensch von 1,75 m
        Self { min: Vec3::new(-0.3, 0.0, -0.15), max: Vec3::new(0.3, 1.75, 0.15) }
    }
}

struct Dist {
    a: u32,
    b: u32,
    rest: f32,
    compliance: f32,
}

/// Nahtpunkt: Punkt a wird zu einer Stelle zwischen b0 und b1 gezogen (Anteil w bei b1)
struct Stitch {
    a: u32,
    b0: u32,
    b1: u32,
    w: f32,
    rest: f32,
}

/// Ein Schnittteil in der Simulation
pub struct ClothPiece {
    pub piece_id: u64,
    pub verts: std::ops::Range<u32>,
    /// Bereich in `Cloth::indices` (in Indizes, nicht in Dreiecken)
    pub index_range: std::ops::Range<u32>,
    pub color: [f32; 3],
}

pub struct Cloth {
    pub pos: Vec<Vec3>,
    prev: Vec<Vec3>,
    vel: Vec<Vec3>,
    inv_mass: Vec<f32>,
    friction: Vec<f32>,
    /// Lage im Schnitt (cm), für UVs und spätere Ausgabe
    pub uv: Vec<Vec2>,
    pub indices: Vec<u32>,
    pub pieces: Vec<ClothPiece>,
    stretch: Vec<Dist>,
    bend: Vec<Dist>,
    stitches: Vec<Stitch>,
    /// Zu welchem Teil ein Punkt gehört und ob er an einer Naht liegt (für die Selbstkollision)
    piece_of: Vec<u32>,
    in_seam: Vec<bool>,
    /// Punktabstand in Metern
    spacing: f32,
    pub time: f32,
}

/// Zeitschritte pro Bild; kleine Schritte machen XPBD steif und stabil
const SUBSTEPS: usize = 15;
const THICKNESS: f32 = 0.004;
const SEAM_SPEED: f32 = 0.6; // m/s, so schnell ziehen sich Nähte zusammen

impl Cloth {
    /// Vernetzt alle Schnittteile mit dem Partikelabstand `spacing_cm` und legt sie am Avatar an
    pub fn build(doc: &Doc, body: Body, spacing_cm: f32) -> Cloth {
        let spacing = spacing_cm.clamp(0.5, 10.0);
        let mut c = Cloth {
            pos: Vec::new(),
            prev: Vec::new(),
            vel: Vec::new(),
            inv_mass: Vec::new(),
            friction: Vec::new(),
            uv: Vec::new(),
            indices: Vec::new(),
            pieces: Vec::new(),
            stretch: Vec::new(),
            bend: Vec::new(),
            stitches: Vec::new(),
            piece_of: Vec::new(),
            in_seam: Vec::new(),
            spacing: spacing * 0.01,
            time: 0.0,
        };
        let mut meshes = std::collections::HashMap::new();

        for piece in &doc.pieces {
            let Some(m) = mesh::mesh_piece(piece, spacing) else { continue };
            let base = c.pos.len() as u32;
            let tri0 = c.indices.len() as u32;
            let fp = piece.fabric.params();
            let place = placer(piece, body);
            for &p in &m.verts {
                let p3 = place(p);
                c.pos.push(p3);
                c.prev.push(p3);
                c.vel.push(Vec3::ZERO);
                c.inv_mass.push(0.0);
                c.friction.push(fp.friction);
                c.uv.push(p);
                c.piece_of.push(c.pieces.len() as u32);
                c.in_seam.push(false);
            }
            // Masse aus der Fläche der anliegenden Dreiecke
            let mut mass = vec![0.0f32; m.verts.len()];
            for t in m.tris.chunks_exact(3) {
                let (a, b, d) = (m.verts[t[0] as usize], m.verts[t[1] as usize], m.verts[t[2] as usize]);
                let area = ((b - a).perp_dot(d - a)).abs() * 0.5 * 1e-4; // cm² → m²
                for &i in t {
                    mass[i as usize] += area * fp.density / 3.0;
                }
            }
            for (i, &mm) in mass.iter().enumerate() {
                c.inv_mass[base as usize + i] = if mm > 0.0 { 1.0 / mm } else { 0.0 };
            }
            c.indices.extend(m.tris.iter().map(|&i| i + base));

            // Dehnung: jede Kante; Biegung: gegenüberliegende Punkte zweier Nachbardreiecke
            let mut edges: std::collections::HashMap<(u32, u32), u32> = std::collections::HashMap::new();
            for t in m.tris.chunks_exact(3) {
                for k in 0..3 {
                    let (a, b) = (t[k], t[(k + 1) % 3]);
                    let opp = t[(k + 2) % 3];
                    let key = (a.min(b), a.max(b));
                    match edges.get(&key) {
                        None => {
                            edges.insert(key, opp);
                            let rest = (m.verts[a as usize] - m.verts[b as usize]).length() * 0.01;
                            c.stretch.push(Dist { a: a + base, b: b + base, rest, compliance: fp.stretch });
                        }
                        Some(&other) => {
                            let rest = (m.verts[opp as usize] - m.verts[other as usize]).length() * 0.01;
                            c.bend.push(Dist { a: opp + base, b: other + base, rest, compliance: fp.bend });
                        }
                    }
                }
            }
            c.pieces.push(ClothPiece {
                piece_id: piece.id,
                verts: base..c.pos.len() as u32,
                index_range: tri0..c.indices.len() as u32,
                color: fp.color,
            });
            meshes.insert(piece.id, (base, m));
        }

        // Nähte: Randpunkte beider Kanten nach ihrem Anteil an der Kantenlänge einander zuordnen
        for seam in &doc.seams {
            let (Some(sa), Some(sb)) = (doc.segment(seam.a), doc.segment(seam.b)) else { continue };
            let (Some((ba, ma)), Some((bb, mb))) = (meshes.get(&seam.a.piece), meshes.get(&seam.b.piece)) else { continue };
            let ra = mesh::boundary_on(ma, &sa.poly, spacing * 0.2);
            let rb = mesh::boundary_on(mb, &sb.poly, spacing * 0.2);
            if ra.len() < 2 || rb.len() < 2 {
                continue;
            }
            let map_b = |t: f32| if seam.flip { 1.0 - t } else { t };
            for &(t, va) in &ra {
                let (b0, b1, w) = locate(&rb, map_b(t));
                c.stitches.push(Stitch { a: va + ba, b0: b0 + bb, b1: b1 + bb, w, rest: 0.0 });
            }
            for &(t, vb) in &rb {
                let (a0, a1, w) = locate(&ra, map_b(t));
                c.stitches.push(Stitch { a: vb + bb, b0: a0 + ba, b1: a1 + ba, w, rest: 0.0 });
            }
        }
        for s in &c.stitches {
            for i in [s.a, s.b0, s.b1] {
                c.in_seam[i as usize] = true;
            }
        }
        // Anfangslänge merken: die Nähte ziehen sich von dort langsam zusammen
        for s in &mut c.stitches {
            let target = c.pos[s.b0 as usize].lerp(c.pos[s.b1 as usize], s.w);
            s.rest = c.pos[s.a as usize].distance(target);
        }
        c
    }

    pub fn particle_count(&self) -> usize {
        self.pos.len()
    }

    /// Ein Bild weiter (1/60 s)
    pub fn step(&mut self, collider: Option<&Collider>) {
        let frame = 1.0 / 60.0;
        let h = frame / SUBSTEPS as f32;
        let gravity = Vec3::new(0.0, -9.81, 0.0);
        for _ in 0..SUBSTEPS {
            // Vorhersage
            for i in 0..self.pos.len() {
                if self.inv_mass[i] == 0.0 {
                    continue;
                }
                self.vel[i] += gravity * h;
                self.prev[i] = self.pos[i];
                self.pos[i] += self.vel[i] * h;
            }
            // Nähte ziehen sich mit begrenzter Geschwindigkeit zusammen
            for s in &mut self.stitches {
                s.rest = (s.rest - SEAM_SPEED * h).max(0.0);
            }
            let h2 = h * h;
            solve(&mut self.pos, &self.inv_mass, &self.stretch, h2);
            solve(&mut self.pos, &self.inv_mass, &self.bend, h2);
            self.solve_stitches(h2);
            self.self_collide();
            let hit = self.collide(collider);
            // Geschwindigkeit aus der Bewegung, Reibung an Kontaktstellen, etwas Luftwiderstand
            for i in 0..self.pos.len() {
                let mut v = (self.pos[i] - self.prev[i]) / h;
                if let Some(n) = hit[i] {
                    // Am Kontakt: in die Fläche hinein keine Geschwindigkeit, entlang der Fläche Reibung
                    let along_n = v.dot(n);
                    let tangential = v - n * along_n;
                    v = tangential * (1.0 - self.friction[i]).max(0.0) + n * along_n.max(0.0);
                }
                // Luftwiderstand: der Stoff kommt zur Ruhe, statt nachzuzappeln
                self.vel[i] = v * (1.0 - 2.0 * h);
                // Ausreißer abfangen, falls etwas explodiert
                if !self.vel[i].is_finite() || self.vel[i].length() > 20.0 {
                    self.vel[i] = Vec3::ZERO;
                    if !self.pos[i].is_finite() {
                        self.pos[i] = self.prev[i];
                    }
                }
            }
        }
        self.time += frame;
    }

    fn solve_stitches(&mut self, h2: f32) {
        let alpha = 1e-7 / h2;
        for s in &self.stitches {
            let (a, b0, b1) = (s.a as usize, s.b0 as usize, s.b1 as usize);
            let target = self.pos[b0].lerp(self.pos[b1], s.w);
            let d = self.pos[a] - target;
            let len = d.length();
            if len <= s.rest || len < 1e-9 {
                continue;
            }
            let n = d / len;
            let (wa, w0, w1) = (self.inv_mass[a], self.inv_mass[b0] * (1.0 - s.w) * (1.0 - s.w), self.inv_mass[b1] * s.w * s.w);
            let wsum = wa + w0 + w1;
            if wsum == 0.0 {
                continue;
            }
            let lambda = (len - s.rest) / (wsum + alpha);
            self.pos[a] -= n * lambda * wa;
            self.pos[b0] += n * lambda * self.inv_mass[b0] * (1.0 - s.w);
            self.pos[b1] += n * lambda * self.inv_mass[b1] * s.w;
        }
    }

    /// Selbstkollision: Stoffpunkte halten Abstand voneinander, außer sie liegen auch im Schnitt nah beieinander.
    /// Jacobi-Schritt (erst alle Korrekturen berechnen, dann anwenden), damit es parallel laufen kann.
    fn self_collide(&mut self) {
        use rayon::prelude::*;
        use std::collections::HashMap;
        let r = self.spacing * 0.45; // Mindestabstand halbe Punktdichte: Falten liegen aufeinander, nicht ineinander
        let cell = 2.0 * r;
        let key = |p: Vec3| ((p.x / cell).floor() as i32, (p.y / cell).floor() as i32, (p.z / cell).floor() as i32);
        let mut grid: HashMap<(i32, i32, i32), Vec<u32>> = HashMap::with_capacity(self.pos.len());
        for (i, p) in self.pos.iter().enumerate() {
            grid.entry(key(*p)).or_default().push(i as u32);
        }
        let near_cm = 2.0 * r * 100.0 * 1.6;
        let (pos, uv, piece_of, in_seam, inv_mass) = (&self.pos, &self.uv, &self.piece_of, &self.in_seam, &self.inv_mass);
        let corr: Vec<Vec3> = (0..pos.len())
            .into_par_iter()
            .map(|i| {
                if inv_mass[i] == 0.0 {
                    return Vec3::ZERO;
                }
                let (cx, cy, cz) = key(pos[i]);
                let mut c = Vec3::ZERO;
                for dx in -1..=1 {
                    for dy in -1..=1 {
                        for dz in -1..=1 {
                            let Some(list) = grid.get(&(cx + dx, cy + dy, cz + dz)) else { continue };
                            for &j in list {
                                let j = j as usize;
                                if j == i {
                                    continue;
                                }
                                // Nachbarn im Schnitt dürfen nah sein; genähte Ränder verschiedener Teile auch
                                if piece_of[i] == piece_of[j] {
                                    if uv[i].distance(uv[j]) < near_cm {
                                        continue;
                                    }
                                } else if in_seam[i] && in_seam[j] {
                                    continue;
                                }
                                let d = pos[i] - pos[j];
                                let len = d.length();
                                if len < 2.0 * r && len > 1e-9 {
                                    c += d / len * (2.0 * r - len) * 0.5;
                                }
                            }
                        }
                    }
                }
                c
            })
            .collect();
        for (p, c) in self.pos.iter_mut().zip(corr) {
            *p += c * 0.5;
        }
    }

    /// Kollision mit Avatar und Boden; liefert je Punkt die Kontaktnormale
    fn collide(&mut self, collider: Option<&Collider>) -> Vec<Option<Vec3>> {
        use rayon::prelude::*;
        let prev = &self.prev;
        self.pos
            .par_iter_mut()
            .zip(prev.par_iter())
            .map(|(p, prev)| {
                let mut normal = None;
                if let Some(col) = collider {
                    if let Some((q, n)) = col.push_out(*p, *prev, THICKNESS) {
                        *p = q;
                        normal = Some(n);
                    }
                }
                if p.y < THICKNESS {
                    p.y = THICKNESS;
                    normal = Some(Vec3::Y);
                }
                normal
            })
            .collect()
    }

    /// Normalen je Punkt für die Darstellung
    pub fn normals(&self) -> Vec<Vec3> {
        let mut n = vec![Vec3::ZERO; self.pos.len()];
        for t in self.indices.chunks_exact(3) {
            let (a, b, c) = (t[0] as usize, t[1] as usize, t[2] as usize);
            let f = (self.pos[b] - self.pos[a]).cross(self.pos[c] - self.pos[a]);
            n[a] += f;
            n[b] += f;
            n[c] += f;
        }
        n.iter_mut().for_each(|v| *v = v.normalize_or(Vec3::Y));
        n
    }

    /// Welches Schnittteil liegt unter dem Strahl? (ID, Abstand)
    pub fn pick(&self, origin: Vec3, dir: Vec3) -> Option<(u64, f32)> {
        let mut best: Option<(f32, u64)> = None;
        for p in &self.pieces {
            for t in self.indices[p.index_range.start as usize..p.index_range.end as usize].chunks_exact(3) {
                let (a, b, c) = (self.pos[t[0] as usize], self.pos[t[1] as usize], self.pos[t[2] as usize]);
                if let Some(d) = crate::import::ray_triangle(origin, dir, a, b, c) {
                    if best.is_none_or(|(bd, _)| d < bd) {
                        best = Some((d, p.piece_id));
                    }
                }
            }
        }
        best.map(|(d, id)| (id, d))
    }

    /// Größter Abstand zwischen genähten Punkten (zeigt, ob die Nähte geschlossen sind)
    #[cfg(test)]
    pub fn max_seam_gap(&self) -> f32 {
        self.stitches
            .iter()
            .map(|s| self.pos[s.a as usize].distance(self.pos[s.b0 as usize].lerp(self.pos[s.b1 as usize], s.w)))
            .fold(0.0, f32::max)
    }
}

/// Gauss-Seidel über Abstandsbedingungen (XPBD)
fn solve(pos: &mut [Vec3], inv_mass: &[f32], cons: &[Dist], h2: f32) {
    for c in cons {
        let (a, b) = (c.a as usize, c.b as usize);
        let w = inv_mass[a] + inv_mass[b];
        if w == 0.0 {
            continue;
        }
        let d = pos[a] - pos[b];
        let len = d.length();
        if len < 1e-9 {
            continue;
        }
        let alpha = c.compliance / h2;
        let lambda = (len - c.rest) / (w + alpha);
        let n = d / len;
        pos[a] -= n * lambda * inv_mass[a];
        pos[b] += n * lambda * inv_mass[b];
    }
}

/// Stelle t (0..1) auf einer nach Anteil sortierten Randpunktliste: (Punkt davor, Punkt danach, Anteil)
fn locate(list: &[(f32, u32)], t: f32) -> (u32, u32, f32) {
    let t = t.clamp(0.0, 1.0);
    for w in list.windows(2) {
        if t <= w[1].0 {
            let span = (w[1].0 - w[0].0).max(1e-6);
            return (w[0].1, w[1].1, ((t - w[0].0) / span).clamp(0.0, 1.0));
        }
    }
    let last = list[list.len() - 1].1;
    (last, last, 0.0)
}

/// Rechnet einen Schnittpunkt (cm, Vorderansicht) in die Anfangslage im Raum um
fn placer(piece: &crate::schnitt::model::Piece, body: Body) -> impl Fn(Vec2) -> Vec3 {
    let (min, max) = piece.bounds();
    let c = (min + max) * 0.5;
    let gap = 0.06;
    let center_z = (body.min.z + body.max.z) * 0.5;
    let placement = piece.placement;
    move |p: Vec2| match placement {
        Placement::Front => Vec3::new(p.x * 0.01, p.y * 0.01, body.max.z + gap),
        // Von hinten gesehen wie gezeichnet: um die eigene Mitte gespiegelt
        Placement::Back => Vec3::new((2.0 * c.x - p.x) * 0.01, p.y * 0.01, body.min.z - gap),
        Placement::Above => Vec3::new(p.x * 0.01, body.max.y + gap, center_z + (c.y - p.y) * 0.01),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tuch(doc: &mut Doc, placement: Placement) -> u64 {
        let id = doc.add_piece(
            "Tuch",
            &[
                (Vec2::new(-40.0, 100.0), false),
                (Vec2::new(40.0, 100.0), false),
                (Vec2::new(40.0, 180.0), false),
                (Vec2::new(-40.0, 180.0), false),
            ],
        );
        doc.piece_mut(id).unwrap().placement = placement;
        id
    }

    #[test]
    fn vernetzen_ergibt_sinnvolle_dichte() {
        let mut doc = Doc::default();
        tuch(&mut doc, Placement::Front);
        let c = Cloth::build(&doc, Body::default(), 2.0);
        // 80 × 80 cm bei 2 cm: grob 1600–4000 Punkte
        assert!((1200..5000).contains(&c.particle_count()), "{}", c.particle_count());
        assert!(c.pos.iter().all(|p| (p.z - 0.21).abs() < 1e-5));
    }

    #[test]
    fn tuch_faellt_auf_den_boden_und_bleibt_heil() {
        let mut doc = Doc::default();
        tuch(&mut doc, Placement::Above);
        let mut c = Cloth::build(&doc, Body::default(), 4.0);
        for _ in 0..240 {
            c.step(None);
        }
        let (lo, hi) = c.pos.iter().fold((f32::MAX, f32::MIN), |(lo, hi), p| (lo.min(p.y), hi.max(p.y)));
        assert!(lo >= THICKNESS * 0.99 && hi < 0.05, "Höhe {lo}..{hi}");
        // Kanten nicht über 10 % gedehnt
        let worst = c.stretch.iter().map(|d| c.pos[d.a as usize].distance(c.pos[d.b as usize]) / d.rest).fold(0.0, f32::max);
        assert!(worst < 1.1, "Dehnung {worst}");
    }

    #[test]
    fn tuch_bleibt_auf_dem_kopf_liegen() {
        let m = crate::import::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/dateien/figur.glb"),
            &Default::default(),
        )
        .unwrap();
        let col = Collider::new(&m, &[]);
        let mut doc = Doc::default();
        // 60 × 60 cm, mittig über dem Kopf (Kopf: x ±11 cm, Oberkante 1,75 m)
        let id = doc.add_piece(
            "Tuch",
            &[(Vec2::new(-30.0, 110.0), false), (Vec2::new(30.0, 110.0), false), (Vec2::new(30.0, 170.0), false), (Vec2::new(-30.0, 170.0), false)],
        );
        doc.piece_mut(id).unwrap().placement = Placement::Above;
        let mut c = Cloth::build(&doc, Body { min: m.min, max: m.max }, 3.0);
        for _ in 0..240 {
            c.step(Some(&col));
        }
        // Der höchste Punkt liegt auf dem Kopf, nicht darin und nicht am Boden
        let top = c.pos.iter().map(|p| p.y).fold(f32::MIN, f32::max);
        assert!(top > 1.74 && top < 1.80, "oben {top}");
        // Kein Punkt steckt im Kopf
        let inside = c.pos.iter().filter(|p| p.x.abs() < 0.10 && p.z.abs() < 0.10 && p.y > 1.53 && p.y < 1.74).count();
        assert_eq!(inside, 0);
    }

    /// Stoffpunkte, die im Schnitt weit auseinanderliegen, im Raum aber fast aufeinander: Stoff steckt in sich selbst
    fn durchdringungen(c: &Cloth, spacing_cm: f32) -> usize {
        let mut n = 0;
        for i in 0..c.pos.len() {
            for j in (i + 1)..c.pos.len() {
                if c.uv[i].distance(c.uv[j]) > 3.0 * spacing_cm && c.pos[i].distance(c.pos[j]) < 0.25 * spacing_cm * 0.01 {
                    n += 1;
                }
            }
        }
        n
    }

    #[test]
    fn tuch_knuellt_nicht_in_sich_selbst() {
        let m = crate::import::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/dateien/figur.glb"),
            &Default::default(),
        )
        .unwrap();
        let col = Collider::new(&m, &[]);
        let mut doc = Doc::default();
        let id = doc.add_piece(
            "Tuch",
            &[(Vec2::new(-45.0, 100.0), false), (Vec2::new(45.0, 100.0), false), (Vec2::new(45.0, 190.0), false), (Vec2::new(-45.0, 190.0), false)],
        );
        doc.piece_mut(id).unwrap().placement = Placement::Above;
        let mut c = Cloth::build(&doc, Body { min: m.min, max: m.max }, 3.0);
        for _ in 0..300 {
            c.step(Some(&col));
        }
        let n = durchdringungen(&c, 3.0);
        assert!(n < 5, "{n} Stellen, an denen der Stoff in sich selbst steckt");
    }

    #[test]
    fn tuch_bleibt_auf_katzenkopf_und_knuellt_nicht() {
        // Lucys Fall (0.1.8): Tuch fiel durch einen Kopf mit falsch herum zeigenden Flächen und knüllte am Boden
        let m = crate::import::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/dateien/figur_tpose_katze.obj"),
            &Default::default(),
        )
        .unwrap();
        let col = Collider::new(&m, &[]);
        let mut doc = Doc::default();
        let id = doc.add_piece(
            "Tuch",
            &[(Vec2::new(-30.0, 110.0), false), (Vec2::new(30.0, 110.0), false), (Vec2::new(30.0, 190.0), false), (Vec2::new(-30.0, 190.0), false)],
        );
        doc.piece_mut(id).unwrap().placement = Placement::Above;
        let mut c = Cloth::build(&doc, Body { min: m.min, max: m.max }, 3.0);
        for _ in 0..240 {
            c.step(Some(&col));
        }
        let top = c.pos.iter().map(|p| p.y).fold(f32::MIN, f32::max);
        let low = c.pos.iter().map(|p| p.y).fold(f32::MAX, f32::min);
        assert!(top > 1.70, "Tuch nicht mehr auf dem Kopf: oben {top}");
        assert!(low > 0.5, "Tuch hängt bis zum Boden: unten {low}");
        assert!(durchdringungen(&c, 3.0) < 5);
    }

    /// Sichtprüfung: Tuch über die T-Pose-Figur fallen lassen und Avatar + Stoff als OBJ schreiben.
    /// cargo test sichtpruefung -- --ignored, Ziel in STOFFWERK_OBJ_OUT
    #[test]
    #[ignore]
    fn sichtpruefung_tuch_auf_katze() {
        let out = std::env::var("STOFFWERK_OBJ_OUT").unwrap_or("/tmp/tuch.obj".into());
        let m = crate::import::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/dateien/figur_tpose_katze.obj"),
            &Default::default(),
        )
        .unwrap();
        let col = Collider::new(&m, &[]);
        let mut doc = Doc::default();
        let id = doc.add_piece(
            "Tuch",
            &[(Vec2::new(-30.0, 110.0), false), (Vec2::new(30.0, 110.0), false), (Vec2::new(30.0, 190.0), false), (Vec2::new(-30.0, 190.0), false)],
        );
        doc.piece_mut(id).unwrap().placement = Placement::Above;
        let mut c = Cloth::build(&doc, Body { min: m.min, max: m.max }, 2.0);
        for _ in 0..300 {
            c.step(Some(&col));
        }
        eprintln!("Durchdringungen: {}", durchdringungen(&c, 2.0));
        let mut s = String::new();
        for p in &m.positions {
            s += &format!("v {} {} {}\n", p.x, p.y, p.z);
        }
        for p in &c.pos {
            s += &format!("v {} {} {}\n", p.x, p.y, p.z);
        }
        s += "o avatar\n";
        for t in m.indices.chunks_exact(3) {
            s += &format!("f {} {} {}\n", t[0] + 1, t[1] + 1, t[2] + 1);
        }
        s += "o stoff\n";
        let o = m.positions.len() as u32 + 1;
        for t in c.indices.chunks_exact(3) {
            s += &format!("f {} {} {}\n", t[0] + o, t[1] + o, t[2] + o);
        }
        std::fs::write(out, s).unwrap();
    }

    #[test]
    fn naht_schliesst_sich() {
        // Vorder- und Rückenteil, an beiden Seiten vernäht: die Nähte müssen zugehen
        let mut doc = Doc::default();
        let a = tuch(&mut doc, Placement::Front);
        let b = tuch(&mut doc, Placement::Back);
        let pa = doc.piece(a).unwrap().pts.clone();
        let pb = doc.piece(b).unwrap().pts.clone();
        // Rechte Kante vorn (Punkt 1 → 2) an rechte Kante hinten. Hinten ist gespiegelt: dieselbe Kante im Schnitt
        let id1 = doc.new_id();
        doc.seams.push(crate::schnitt::model::Seam {
            id: id1,
            a: crate::schnitt::model::SegRef { piece: a, start: pa[1].id },
            b: crate::schnitt::model::SegRef { piece: b, start: pb[3].id },
            flip: true,
        });
        let mut c = Cloth::build(&doc, Body::default(), 4.0);
        let start = c.max_seam_gap();
        for _ in 0..180 {
            c.step(None);
        }
        let end = c.max_seam_gap();
        assert!(start > 0.2 && end < 0.02, "Naht {start} → {end}");
    }
}
