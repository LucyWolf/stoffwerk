//! Kollision Stoff ↔ Avatar: Dreiecke des Avatars in einem Raster, je Punkt die nächste Fläche.

use glam::Vec3;

use std::collections::HashMap;

const CELL: f32 = 0.03;

pub struct Collider {
    tris: Vec<[Vec3; 3]>,
    normals: Vec<Vec3>,
    grid: HashMap<(i32, i32, i32), Vec<u32>>,
    /// Eckpunkte der sichtbaren Teile: Spitzen (Ohren, Fell, Schnurrhaare) sollen nicht durch den Stoff stechen
    pub verts: Vec<Vec3>,
    /// Kanten dünner Teile (Fell, Haarsträhnen, Schnurrhaare) als Röhren mit Mantel
    thin_edges: Vec<[Vec3; 2]>,
    thin_grid: HashMap<(i32, i32, i32), Vec<u32>>,
}

/// Größter Mantel, für den das Raster der dünnen Kanten ausgelegt ist
const THIN_MARGIN: f32 = 0.04;

fn cell(p: Vec3) -> (i32, i32, i32) {
    ((p.x / CELL).floor() as i32, (p.y / CELL).floor() as i32, (p.z / CELL).floor() as i32)
}

impl Collider {
    /// Nur sichtbare Teile des Avatars zählen (ausgeblendete Haare stören dann nicht).
    /// Dünne Teile (`thin`) kollidieren nicht über ihre Flächen, sondern über ihre Kanten mit einem Mantel:
    /// sie sind dünner als der Punktabstand des Stoffs und würden sonst zwischen den Punkten hindurchstechen.
    pub fn new(avatar: &crate::import::AvatarMesh, hidden: &[bool], thin: &[bool]) -> Self {
        let mut tris = Vec::new();
        let mut edge_set = std::collections::HashSet::new();
        for (pi, part) in avatar.parts.iter().enumerate() {
            if hidden.get(pi).copied().unwrap_or(false) {
                continue;
            }
            let r = part.first as usize..(part.first + part.count) as usize;
            let is_thin = thin.get(pi).copied().unwrap_or(false);
            for t in avatar.indices[r].chunks_exact(3) {
                if is_thin {
                    for k in 0..3 {
                        let (a, b) = (t[k], t[(k + 1) % 3]);
                        edge_set.insert((a.min(b), a.max(b)));
                    }
                } else {
                    tris.push([t[0], t[1], t[2]].map(|i| avatar.positions[i as usize]));
                }
            }
        }
        let thin_edges: Vec<[Vec3; 2]> =
            edge_set.into_iter().map(|(a, b)| [avatar.positions[a as usize], avatar.positions[b as usize]]).collect();
        let mut thin_grid: HashMap<(i32, i32, i32), Vec<u32>> = HashMap::new();
        for (i, e) in thin_edges.iter().enumerate() {
            let lo = cell(e[0].min(e[1]) - Vec3::splat(THIN_MARGIN));
            let hi = cell(e[0].max(e[1]) + Vec3::splat(THIN_MARGIN));
            for x in lo.0..=hi.0 {
                for y in lo.1..=hi.1 {
                    for z in lo.2..=hi.2 {
                        thin_grid.entry((x, y, z)).or_default().push(i as u32);
                    }
                }
            }
        }
        let normals = tris.iter().map(|t| (t[1] - t[0]).cross(t[2] - t[0]).normalize_or_zero()).collect();
        let mut grid: HashMap<(i32, i32, i32), Vec<u32>> = HashMap::new();
        for (i, t) in tris.iter().enumerate() {
            let lo = cell(t[0].min(t[1]).min(t[2]) - Vec3::splat(0.01));
            let hi = cell(t[0].max(t[1]).max(t[2]) + Vec3::splat(0.01));
            for x in lo.0..=hi.0 {
                for y in lo.1..=hi.1 {
                    for z in lo.2..=hi.2 {
                        grid.entry((x, y, z)).or_default().push(i as u32);
                    }
                }
            }
        }
        let mut verts: Vec<Vec3> = tris.iter().flatten().copied().collect();
        // Doppelte Punkte (gemeinsame Ecken) zusammenfassen
        verts.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)).then(a.z.total_cmp(&b.z)));
        verts.dedup_by(|a, b| a.distance_squared(*b) < 1e-12);
        // Auch die Spitzen dünner Teile zählen für die Zeltprüfung
        verts.extend(thin_edges.iter().flatten().copied());
        verts.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)).then(a.z.total_cmp(&b.z)));
        verts.dedup_by(|a, b| a.distance_squared(*b) < 1e-12);
        Self { tris, normals, grid, verts, thin_edges, thin_grid }
    }

    /// Mantel um dünne Kanten: Stoffpunkte bleiben `radius` von jeder Kante entfernt
    pub fn push_out_thin(&self, p: Vec3, prev: Vec3, radius: f32) -> Option<(Vec3, Vec3)> {
        let list = self.thin_grid.get(&cell(p))?;
        let r = radius.min(THIN_MARGIN);
        let mut best: Option<(f32, Vec3)> = None;
        for &ei in list {
            let [a, b] = self.thin_edges[ei as usize];
            let ab = b - a;
            let t = if ab.length_squared() > 0.0 { ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
            let q = a + ab * t;
            let d2 = q.distance_squared(p);
            if d2 < r * r && best.is_none_or(|bb| d2 < bb.0) {
                best = Some((d2, q));
            }
        }
        let (_, q) = best?;
        let mut dir = p - q;
        if dir.length_squared() < 1e-12 {
            dir = prev - q;
        }
        let dir = dir.normalize_or(Vec3::Y);
        Some((q + dir * r, dir))
    }

    /// Liegt der Punkt zu nah an oder in der Haut, wird er auf die Oberfläche (plus Dicke) geschoben.
    /// „Außen“ ist die Seite, von der der Punkt kam (`prev`), nicht die Richtung der Flächennormale:
    /// so stören weder falsch herum zeigende Flächen noch einseitige Ohren oder Schnurrhaare.
    pub fn push_out(&self, p: Vec3, prev: Vec3, thickness: f32) -> Option<(Vec3, Vec3)> {
        let list = self.grid.get(&cell(p))?;
        let mut best: Option<(f32, Vec3, Vec3)> = None;
        let mut best_tri = 0u32;
        for &ti in list {
            let t = &self.tris[ti as usize];
            let q = closest_on_triangle(p, t[0], t[1], t[2]);
            let d2 = p.distance_squared(q);
            if best.is_none_or(|b| d2 < b.0) {
                best = Some((d2, q, self.normals[ti as usize]));
                best_tri = ti;
            }
        }

        let (d2, q, n) = best?;
        if n == Vec3::ZERO {
            return None;
        }
        let came_from = (prev - q).dot(n);
        let mut out = if came_from >= 0.0 { n } else { -n };
        // An Kanten und Spitzen (Ohrspitze, Schnurrhaar-Ende) ist die Flächennormale die falsche Richtung:
        // sie würde den Stoff seitlich neben die Spitze ziehen. Dort weg von der Kante, dorthin, woher er kam.
        let interior = is_interior(q, self.tris[best_tri as usize]);
        if !interior {
            let away = prev - q;
            if away.length_squared() > 1e-12 {
                out = away.normalize();
            }
        }
        let side = (p - q).dot(out);
        // Auf der eigenen Seite und weiter weg als die Dicke: nichts zu tun
        if side >= thickness || (side > 0.0 && d2.sqrt() >= thickness) {
            return None;
        }
        Some((q + out * thickness, out))
    }
}

/// Liegt q im Inneren des Dreiecks (nicht auf Kante oder Ecke)?
fn is_interior(q: Vec3, [a, b, c]: [Vec3; 3]) -> bool {
    let (v0, v1, v2) = (b - a, c - a, q - a);
    let (d00, d01, d11, d20, d21) = (v0.dot(v0), v0.dot(v1), v1.dot(v1), v2.dot(v0), v2.dot(v1));
    let den = d00 * d11 - d01 * d01;
    if den.abs() < 1e-14 {
        return false;
    }
    let v = (d11 * d20 - d01 * d21) / den;
    let w = (d00 * d21 - d01 * d20) / den;
    let u = 1.0 - v - w;
    u > 1e-4 && v > 1e-4 && w > 1e-4
}

/// Nächster Punkt auf einem Dreieck (Ericson, Real-Time Collision Detection 5.1.5)
pub fn closest_on_triangle(p: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Vec3 {
    let ab = b - a;
    let ac = c - a;
    let ap = p - a;
    let d1 = ab.dot(ap);
    let d2 = ac.dot(ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = p - b;
    let d3 = ab.dot(bp);
    let d4 = ac.dot(bp);
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return a + ab * (d1 / (d1 - d3));
    }
    let cp = p - c;
    let d5 = ab.dot(cp);
    let d6 = ac.dot(cp);
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return a + ac * (d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        return b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)));
    }
    let denom = 1.0 / (va + vb + vc);
    a + ab * (vb * denom) + ac * (vc * denom)
}

#[cfg(test)]
mod tests {
    #[test]
    fn einseitige_flaeche_haelt_von_beiden_seiten() {
        // Ein einzelnes Dreieck (wie ein Ohr), Normale zeigt nach +z. Stoff kommt von hinten (−z):
        // er muss hinten bleiben, nicht nach vorn durchgereicht werden
        let mut m = crate::import::AvatarMesh::new();
        for p in [Vec3::new(-0.1, 1.0, 0.0), Vec3::new(0.1, 1.0, 0.0), Vec3::new(0.0, 1.2, 0.0)] {
            m.push_vertex(p, Vec3::Z, glam::Vec2::ZERO, [1.0; 3]);
        }
        m.indices.extend([0, 1, 2]);
        m.close_part(0, None, false);
        let col = Collider::new(&m, &[], &[]);
        let (q, n) = col.push_out(Vec3::new(0.0, 1.05, 0.001), Vec3::new(0.0, 1.05, -0.003), 0.004).unwrap();
        assert!(q.z < 0.0 && n.z < 0.0, "{q} {n}");
    }

    use super::*;

    #[test]
    fn punkt_in_der_haut_wird_hinausgeschoben() {
        let m = crate::import::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/dateien/figur.glb"),
            &Default::default(),
        )
        .unwrap();
        let col = Collider::new(&m, &[], &[]);
        // Rumpf vorne bei z = 0,12; ein Punkt 1 cm hinter der Vorderseite steckt im Körper
        // Von vorn gekommen (prev vor der Haut), jetzt 1 cm drin: nach vorn hinaus
        let (q, n) = col.push_out(Vec3::new(0.0, 1.2, 0.11), Vec3::new(0.0, 1.2, 0.13), 0.004).unwrap();
        assert!((q.z - 0.124).abs() < 1e-3, "{q}");
        assert!(n.z > 0.9);
        // 5 cm davor: frei
        assert!(col.push_out(Vec3::new(0.0, 1.2, 0.17), Vec3::new(0.0, 1.2, 0.18), 0.004).is_none());
    }
}
