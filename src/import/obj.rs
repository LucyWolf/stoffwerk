//! OBJ mit MTL-Materialien.

use super::texture::TextureCache;
use super::{AvatarMesh, SKIN_GRAY};
use anyhow::{Context, Result};
use glam::{Vec2, Vec3};
use std::path::Path;

pub fn load(path: &Path) -> Result<AvatarMesh> {
    let (models, materials) = tobj::load_obj(
        path,
        &tobj::LoadOptions { triangulate: true, single_index: true, ..Default::default() },
    )
    .with_context(|| format!("Datei konnte nicht gelesen werden: {}", path.display()))?;
    let materials = materials.unwrap_or_default();

    let mut out = AvatarMesh::new();
    let mut cache = TextureCache::new(path);
    for m in models {
        let mesh = m.mesh;
        let mat = mesh.material_id.and_then(|i| materials.get(i));
        let texture = mat
            .and_then(|m| m.diffuse_texture.as_deref())
            .and_then(|t| cache.from_file(&[t.split_whitespace().last().unwrap_or(t)]));
        let f = mat.and_then(|m| m.diffuse).unwrap_or([1.0; 3]);
        let base = if texture.is_some() { [1.0; 3] } else { SKIN_GRAY };
        let color = [base[0] * f[0], base[1] * f[1], base[2] * f[2]];
        let cutout = texture.is_some_and(|i| cache.textures[i].has_alpha);

        let base_vertex = out.positions.len() as u32;
        let count = mesh.positions.len() / 3;
        for i in 0..count {
            let p = Vec3::new(mesh.positions[i * 3], mesh.positions[i * 3 + 1], mesh.positions[i * 3 + 2]);
            let n = if mesh.normals.len() == mesh.positions.len() {
                Vec3::new(mesh.normals[i * 3], mesh.normals[i * 3 + 1], mesh.normals[i * 3 + 2]).normalize_or_zero()
            } else {
                Vec3::ZERO
            };
            // OBJ zählt V von unten
            let uv = if mesh.texcoords.len() == count * 2 {
                Vec2::new(mesh.texcoords[i * 2], 1.0 - mesh.texcoords[i * 2 + 1])
            } else {
                Vec2::ZERO
            };
            out.push_vertex(p, n, uv, color);
        }
        let first = out.indices.len();
        out.indices.extend(mesh.indices.iter().map(|i| base_vertex + i));
        out.close_part(first, texture, cutout);
    }
    out.textures = cache.textures;
    out.warnings.extend(cache.missing.iter().map(|m| format!("Textur fehlt: {m}")));
    Ok(out)
}
