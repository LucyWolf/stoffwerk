//! Schnittteil → Dreiecksnetz mit gleichmäßigem Punktabstand (eingeschränkte Delaunay-Triangulierung).

use crate::schnitt::model::{Piece, dist_to_poly, poly_len};
use glam::Vec2;
use spade::{AngleLimit, ConstrainedDelaunayTriangulation, Point2, RefinementParameters, Triangulation};

pub struct PieceMesh {
    /// Punkte im Schnitt (cm)
    pub verts: Vec<Vec2>,
    pub tris: Vec<u32>,
}

pub fn mesh_piece(piece: &Piece, spacing: f32) -> Option<PieceMesh> {
    // Rand gleichmäßig abtasten, jede Kante für sich (Ecken bleiben erhalten)
    let mut ring: Vec<Vec2> = Vec::new();
    for s in piece.segments() {
        let len = poly_len(&s.poly);
        let n = ((len / spacing).ceil() as usize).max(1);
        for k in 0..n {
            ring.push(point_at(&s.poly, k as f32 / n as f32));
        }
    }
    if ring.len() < 3 {
        return None;
    }

    let mut cdt: ConstrainedDelaunayTriangulation<Point2<f64>> = ConstrainedDelaunayTriangulation::new();
    let mut handles = Vec::with_capacity(ring.len());
    for p in &ring {
        handles.push(cdt.insert(Point2::new(p.x as f64, p.y as f64)).ok()?);
    }
    for i in 0..handles.len() {
        let (a, b) = (handles[i], handles[(i + 1) % handles.len()]);
        if a != b && cdt.can_add_constraint(a, b) {
            cdt.add_constraint(a, b);
        }
    }
    // Gleichseitiges Dreieck mit Kante s hat die Fläche 0,433·s²
    let max_area = 0.433 * (spacing as f64).powi(2) * 1.3;
    let result = cdt.refine(
        RefinementParameters::<f64>::new()
            .exclude_outer_faces(true)
            .with_max_allowed_area(max_area)
            .with_angle_limit(AngleLimit::from_deg(25.0))
            .with_max_additional_vertices(200_000),
    );
    let excluded: std::collections::HashSet<_> = result.excluded_faces.into_iter().collect();

    let mut index = vec![u32::MAX; cdt.num_vertices()];
    let mut verts = Vec::new();
    let mut tris = Vec::new();
    for face in cdt.inner_faces() {
        if excluded.contains(&face.fix()) {
            continue;
        }
        for v in face.vertices() {
            let i = v.fix().index();
            if index[i] == u32::MAX {
                index[i] = verts.len() as u32;
                let p = v.position();
                verts.push(Vec2::new(p.x as f32, p.y as f32));
            }
            tris.push(index[i]);
        }
    }
    if tris.is_empty() {
        return None;
    }
    // Gegen den Uhrzeigersinn ausrichten
    for t in tris.chunks_exact_mut(3) {
        let (a, b, c) = (verts[t[0] as usize], verts[t[1] as usize], verts[t[2] as usize]);
        if (b - a).perp_dot(c - a) < 0.0 {
            t.swap(1, 2);
        }
    }
    Some(PieceMesh { verts, tris })
}

/// Randpunkte des Netzes, die auf einer Kante liegen, nach Anteil der Kantenlänge sortiert
pub fn boundary_on(m: &PieceMesh, poly: &[Vec2], eps: f32) -> Vec<(f32, u32)> {
    let total = poly_len(poly).max(1e-6);
    let mut out: Vec<(f32, u32)> = m
        .verts
        .iter()
        .enumerate()
        .filter_map(|(i, &p)| {
            let (d, seg, t) = dist_to_poly(p, poly);
            if d > eps {
                return None;
            }
            let along: f32 = poly.windows(2).take(seg).map(|w| w[0].distance(w[1])).sum::<f32>()
                + poly[seg].distance(poly[(seg + 1).min(poly.len() - 1)]) * t;
            Some((along / total, i as u32))
        })
        .collect();
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out
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
