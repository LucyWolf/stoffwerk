//! Alembic (.abc, Ogawa-Format) über `ogawa-rs`.
//! Die Bibliothek liefert Netze, aber keine Transformationen und keine UVs. Gelesen wird deshalb das erste
//! Bild jedes PolyMesh, so wie es gespeichert ist. Das passt für Exporte aus Marvelous Designer und Blender,
//! die die Netze in Weltkoordinaten schreiben.

use super::{AvatarMesh, SKIN_GRAY};
use anyhow::{Result, anyhow};
use glam::{Vec2, Vec3};
use ogawa_rs::{Archive, ArchiveReader, FileReader, ObjectReader, Schema};
use std::path::Path;

pub fn load(path: &Path, opts: &super::ImportOptions) -> Result<AvatarMesh> {
    let auto_unit = opts.auto_unit;
    // ogawa-rs bricht bei manchen Dateien mit einem Programmfehler statt eines Fehlers ab – abfangen
    let path = path.to_path_buf();
    std::panic::catch_unwind(move || load_inner(&path, auto_unit))
        .map_err(|_| anyhow!("Diese Alembic-Datei kann Stoffwerk nicht lesen (Aufbau wird nicht unterstützt)."))?
}

fn load_inner(path: &Path, auto_unit: bool) -> Result<AvatarMesh> {
    let file = std::fs::File::open(path)?;
    let mut reader = FileReader::new(file).map_err(|e| anyhow!("Alembic: {e:?}"))?;
    let archive = Archive::new(&mut reader).map_err(|e| anyhow!("Keine gültige Alembic-Datei (nur das Ogawa-Format wird unterstützt): {e:?}"))?;
    let root = archive.load_root_object(&mut reader).map_err(|e| anyhow!("Alembic: {e:?}"))?;

    let mut out = AvatarMesh::new();
    visit(&root, &mut reader, &archive, &mut out, 0);
    if out.indices.is_empty() {
        return Err(anyhow!("In der Alembic-Datei wurde kein Polygonnetz gefunden."));
    }
    // Alembic hat keine feste Einheit; Marvelous Designer schreibt Millimeter oder Zentimeter.
    // Über 200 „Meter“ Höhe → Millimeter (die Zentimeter-Prüfung danach erledigt der Rest).
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    for p in &out.positions {
        lo = lo.min(p.y);
        hi = hi.max(p.y);
    }
    if auto_unit && hi - lo > 200.0 {
        out.positions.iter_mut().for_each(|p| *p *= 0.1);
    }
    Ok(out)
}

fn visit(obj: &ObjectReader, reader: &mut dyn ArchiveReader, archive: &Archive, out: &mut AvatarMesh, depth: u32) {
    if depth > 64 {
        return;
    }
    if let Ok(Schema::PolyMesh(mesh)) = Schema::parse(obj, reader, archive) {
        let pts = mesh.load_vertices_sample(0, reader);
        let counts = mesh.load_facecounts_sample(0, reader);
        let idx = mesh.load_faceindices_sample(0, reader);
        if let (Ok(pts), Ok(counts), Ok(idx)) = (pts, counts, idx) {
            let base = out.positions.len() as u32;
            for p in &pts {
                out.push_vertex(Vec3::from(*p), Vec3::ZERO, Vec2::ZERO, SKIN_GRAY);
            }
            let first = out.indices.len();
            let mut corner = 0usize;
            for &c in &counts {
                let c = c.max(0) as usize;
                let f = &idx[corner.min(idx.len())..(corner + c).min(idx.len())];
                // Alembic speichert Polygone im Uhrzeigersinn
                for k in 1..f.len().saturating_sub(1) {
                    let ok = |i: i32| (i as usize) < pts.len();
                    if ok(f[0]) && ok(f[k]) && ok(f[k + 1]) {
                        out.indices.extend([base + f[0] as u32, base + f[k + 1] as u32, base + f[k] as u32]);
                    }
                }
                corner += c;
            }
            out.close_part(first, None, false);
        }
    }
    for i in 0..obj.child_count() {
        if let Ok(child) = obj.load_child(i, reader, &archive.indexed_meta_data, &archive.time_samplings) {
            visit(&child, reader, archive, out, depth + 1);
        }
    }
}
