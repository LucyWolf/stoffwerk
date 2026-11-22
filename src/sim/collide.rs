//! Kollision Stoff ↔ Avatar: Dreiecke des Avatars in einem Raster, je Punkt die nächste Fläche.

use glam::Vec3;
use std::collections::HashMap;

const CELL: f32 = 0.03;

pub struct Collider {
    tris: Vec<[Vec3; 3]>,
    normals: Vec<Vec3>,
    grid: HashMap<(i32, i32, i32), Vec<u32>>,
}

fn cell(p: Vec3) -> (i32, i32, i32) {
    ((p.x / CELL).floor() as i32, (p.y / CELL).floor() as i32, (p.z / CELL).floor() as i32)
}

impl Collider {
    /// Nur sichtbare Teile des Avatars zählen (ausgeblendete Haare stören dann nicht)
    pub fn new(avatar: &crate::import::AvatarMesh, hidden: &[bool]) -> Self {
        let mut tris = Vec::new();
        for (pi, part) in avatar.parts.iter().enumerate() {
            if hidden.get(pi).copied().unwrap_or(false) {
                continue;
            }
            let r = part.first as usize..(part.first + part.count) as usize;
            for t in avatar.indices[r].chunks_exact(3) {
                tris.push([t[0], t[1], t[2]].map(|i| avatar.positions[i as usize]));
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
        Self { tris, normals, grid }
    }

    /// Liegt der Punkt zu nah an oder in der Haut, wird er auf die Oberfläche (plus Dicke) geschoben
    pub fn push_out(&self, p: Vec3, thickness: f32) -> Option<(Vec3, Vec3)> {
        let list = self.grid.get(&cell(p))?;
        let mut best: Option<(f32, Vec3, Vec3)> = None;
        for &ti in list {
            let t = &self.tris[ti as usize];
            let q = closest_on_triangle(p, t[0], t[1], t[2]);
            let d2 = p.distance_squared(q);
            if best.is_none_or(|b| d2 < b.0) {
                best = Some((d2, q, self.normals[ti as usize]));
            }
        }
        let (d2, q, n) = best?;
        if n == Vec3::ZERO {
            return None;
        }
        let side = (p - q).dot(n);
        // Vor der Fläche und weiter weg als die Dicke: nichts zu tun
        if side >= thickness || (side > 0.0 && d2.sqrt() >= thickness) {
            return None;
        }
        Some((q + n * thickness, n))
    }
}

/// Nächster Punkt auf einem Dreieck (Ericson, Real-Time Collision Detection 5.1.5)
fn closest_on_triangle(p: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Vec3 {
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
    use super::*;

    #[test]
    fn punkt_in_der_haut_wird_hinausgeschoben() {
        let m = crate::import::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/dateien/figur.glb"),
            &Default::default(),
        )
        .unwrap();
        let col = Collider::new(&m, &[]);
        // Rumpf vorne bei z = 0,12; ein Punkt 1 cm hinter der Vorderseite steckt im Körper
        let (q, n) = col.push_out(Vec3::new(0.0, 1.2, 0.11), 0.004).unwrap();
        assert!((q.z - 0.124).abs() < 1e-3, "{q}");
        assert!(n.z > 0.9);
        // 5 cm davor: frei
        assert!(col.push_out(Vec3::new(0.0, 1.2, 0.17), 0.004).is_none());
    }
}
