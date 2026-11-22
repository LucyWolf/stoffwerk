//! FBX über ufbx. ufbx rechnet Achsen und Einheiten gleich ins Netz um:
//! Unity- und Blender-FBX haben oft Z oben oder Zentimeter, hier kommt immer Y oben in Metern an.

use super::texture::TextureCache;
use super::{AvatarMesh, SKIN_GRAY};
use anyhow::{Context, Result, anyhow};
use glam::{Vec2, Vec3};
use std::collections::HashMap;
use std::path::Path;

pub fn load(path: &Path) -> Result<AvatarMesh> {
    let opts = ufbx::LoadOpts {
        target_axes: ufbx::CoordinateAxes::right_handed_y_up(),
        target_unit_meters: 1.0,
        space_conversion: ufbx::SpaceConversion::ModifyGeometry,
        generate_missing_normals: true,
        ..Default::default()
    };
    let file = path.to_str().context("Der Dateipfad enthält Zeichen, die nicht gelesen werden können.")?;
    let scene = ufbx::load_file(file, opts).map_err(|e| anyhow!("FBX konnte nicht gelesen werden: {}", e.description))?;

    let mut out = AvatarMesh::new();
    let mut cache = TextureCache::new(path);
    // Material-ID → (Textur, Farbe, ausstanzen)
    let mut materials: HashMap<u32, (Option<usize>, [f32; 3], bool)> = HashMap::new();

    for node in scene.nodes.iter() {
        let Some(mesh) = node.mesh.as_ref() else { continue };
        let xf = &node.geometry_to_world;
        let nxf = ufbx::matrix_for_normals(xf);
        let mut tri = vec![0u32; mesh.max_face_triangles * 3];

        for part in mesh.material_parts.iter() {
            if part.num_triangles == 0 {
                continue;
            }
            let mat = node.materials.get(part.index as usize).or_else(|| mesh.materials.get(part.index as usize));
            let (texture, color, cutout) = match mat {
                Some(m) => *materials.entry(m.element.element_id).or_insert_with(|| material(m, &mut cache)),
                None => (None, SKIN_GRAY, false),
            };

            let first = out.indices.len();
            for &fi in part.face_indices.iter() {
                let face = mesh.faces[fi as usize];
                let n = mesh.triangulate_face(&mut tri, face) as usize;
                for &ix in &tri[..n * 3] {
                    let ix = ix as usize;
                    let p = ufbx::transform_position(xf, mesh.vertex_position[ix]);
                    let nrm = if mesh.vertex_normal.exists {
                        let v = ufbx::transform_direction(&nxf, mesh.vertex_normal[ix]);
                        Vec3::new(v.x as f32, v.y as f32, v.z as f32).normalize_or_zero()
                    } else {
                        Vec3::ZERO
                    };
                    // FBX zählt V von unten, die GPU von oben
                    let uv = if mesh.vertex_uv.exists {
                        let u = mesh.vertex_uv[ix];
                        Vec2::new(u.x as f32, 1.0 - u.y as f32)
                    } else {
                        Vec2::ZERO
                    };
                    let v = out.push_vertex(Vec3::new(p.x as f32, p.y as f32, p.z as f32), nrm, uv, color);
                    out.indices.push(v);
                }
            }
            out.close_part(first, texture, cutout);
        }
    }

    out.textures = cache.textures;
    out.warnings.extend(cache.missing.iter().map(|m| format!("Textur fehlt: {m}")));
    Ok(out)
}

fn material(m: &ufbx::Material, cache: &mut TextureCache) -> (Option<usize>, [f32; 3], bool) {
    // Unity-Exporte nutzen meist die klassische Diffus-Farbe, neuere Blender-Exporte die PBR-Grundfarbe
    let map = if m.pbr.base_color.texture.is_some() || !m.fbx.diffuse_color.has_value {
        &m.pbr.base_color
    } else {
        &m.fbx.diffuse_color
    };
    let tex = map.texture.as_ref().or(m.fbx.diffuse_color.texture.as_ref());
    let texture = tex.and_then(|t| {
        let t = t.file_textures.first().map(|f| &**f).unwrap_or(&**t);
        if !t.content.is_empty() {
            cache.from_bytes(&format!("fbx-{}", t.element.element_id), &t.filename, &t.content)
        } else {
            cache.from_file(&[&t.absolute_filename, &t.relative_filename, &t.filename])
        }
    });
    let c = map.value_vec4;
    let f = if map.has_value { [c.x as f32, c.y as f32, c.z as f32] } else { [1.0; 3] };
    let base = if texture.is_some() { [1.0; 3] } else { SKIN_GRAY };
    // Ausstanzen, wenn die Textur durchsichtige Stellen hat (Haare, Wimpern)
    let cutout = texture.is_some_and(|i| cache.textures[i].has_alpha);
    (texture, [base[0] * f[0], base[1] * f[1], base[2] * f[2]], cutout)
}
