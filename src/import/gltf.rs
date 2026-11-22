//! glTF, GLB und VRM (VRM 0.x und 1.0 sind glTF mit Zusatzangaben; die Grundfarbtextur steht am üblichen Platz).

use super::texture::{TextureCache, from_rgba};
use super::{AvatarMesh, SKIN_GRAY};
use anyhow::{Context, Result};
use glam::{Mat4, Vec2, Vec3};
use std::collections::HashMap;
use std::path::Path;

pub fn load(path: &Path) -> Result<AvatarMesh> {
    let gltf = gltf::Gltf::open(path)
        .with_context(|| format!("Datei konnte nicht gelesen werden: {}", path.display()))?;
    let base = path.parent();
    let buffers = gltf::import_buffers(&gltf.document, base, gltf.blob.clone())
        .context("Die Netzdaten der Datei fehlen oder sind beschädigt.")?;

    let mut ctx = Ctx {
        out: AvatarMesh::new(),
        buffers: &buffers,
        base,
        cache: TextureCache::new(path),
        by_image: HashMap::new(),
    };
    let doc = &gltf.document;
    let scene = doc.default_scene().or_else(|| doc.scenes().next()).context("Die Datei enthält keine Szene.")?;
    for node in scene.nodes() {
        ctx.visit(&node, Mat4::IDENTITY);
    }

    let mut out = ctx.out;
    out.textures = ctx.cache.textures;
    out.warnings.extend(ctx.cache.missing.iter().map(|m| format!("Textur fehlt: {m}")));
    Ok(out)
}

struct Ctx<'a> {
    out: AvatarMesh,
    buffers: &'a [gltf::buffer::Data],
    base: Option<&'a Path>,
    cache: TextureCache,
    by_image: HashMap<usize, Option<usize>>,
}

impl Ctx<'_> {
    fn visit(&mut self, node: &gltf::Node, parent: Mat4) {
        let world = parent * Mat4::from_cols_array_2d(&node.transform().matrix());
        if let Some(mesh) = node.mesh() {
            // Bei Netzen mit Skelett gilt die Knoten-Transformation nicht, sondern die der Knochen.
            // In der Ruhepose ergibt das die Punkte, wie sie in der Datei stehen.
            let xf = if node.skin().is_some() { Mat4::IDENTITY } else { world };
            let mname = mesh.name().or(node.name()).unwrap_or("").to_string();
            for prim in mesh.primitives() {
                self.primitive(&prim, xf, &mname);
            }
        }
        for child in node.children() {
            self.visit(&child, world);
        }
    }

    fn primitive(&mut self, prim: &gltf::Primitive, xf: Mat4, mesh_name: &str) {
        if prim.mode() != gltf::mesh::Mode::Triangles {
            return;
        }
        let buffers = self.buffers;
        let reader = prim.reader(|b| buffers.get(b.index()).map(|d| &d.0[..]));
        let Some(pos) = reader.read_positions() else { return };
        let positions: Vec<Vec3> = pos.map(|p| xf.transform_point3(Vec3::from(p))).collect();
        let count = positions.len();
        let nxf = xf.inverse().transpose();
        let normals: Vec<Vec3> = reader
            .read_normals()
            .map(|it| it.map(|n| nxf.transform_vector3(Vec3::from(n)).normalize_or_zero()).collect())
            .filter(|n: &Vec<Vec3>| n.len() == count)
            .unwrap_or_else(|| vec![Vec3::ZERO; count]);

        let material = prim.material();
        let pbr = material.pbr_metallic_roughness();
        let f = pbr.base_color_factor();
        let tex_info = pbr.base_color_texture();
        let texture = tex_info.as_ref().and_then(|t| self.texture(t.texture().source()));
        let uv_set = tex_info.as_ref().map(|t| t.tex_coord()).unwrap_or(0);
        let uvs: Vec<Vec2> = reader
            .read_tex_coords(uv_set)
            .map(|it| it.into_f32().map(Vec2::from).collect())
            .filter(|u: &Vec<Vec2>| u.len() == count)
            .unwrap_or_else(|| vec![Vec2::ZERO; count]);
        let base = if texture.is_some() { [1.0; 3] } else { SKIN_GRAY };
        let color = [base[0] * f[0], base[1] * f[1], base[2] * f[2]];
        let cutout = !matches!(material.alpha_mode(), gltf::material::AlphaMode::Opaque);

        let indices: Vec<u32> = match reader.read_indices() {
            Some(idx) => idx.into_u32().collect(),
            None => (0..count as u32).collect(),
        };
        let flip = xf.determinant() < 0.0;
        let first_vertex = self.out.positions.len() as u32;
        for i in 0..count {
            self.out.push_vertex(positions[i], normals[i], uvs[i], color);
        }
        let first = self.out.indices.len();
        for t in indices.chunks_exact(3) {
            if t.iter().any(|&i| i as usize >= count) {
                continue;
            }
            let (a, b, c) = if flip { (t[0], t[2], t[1]) } else { (t[0], t[1], t[2]) };
            self.out.indices.extend([first_vertex + a, first_vertex + b, first_vertex + c]);
        }
        self.out.close_part(first, texture, cutout);
        self.out.name_part(first, &[mesh_name, material.name().unwrap_or("")]);
    }

    fn texture(&mut self, image: gltf::Image) -> Option<usize> {
        let idx = image.index();
        if let Some(hit) = self.by_image.get(&idx) {
            return *hit;
        }
        let result = match image.source() {
            // Eingebettet (GLB/VRM): Bytes direkt aus dem Puffer
            gltf::image::Source::View { view, .. } => {
                let buf = &self.buffers[view.buffer().index()].0;
                let bytes = &buf[view.offset()..view.offset() + view.length()];
                self.cache.from_bytes(&format!("gltf-image-{idx}"), image.name().unwrap_or("bild"), bytes)
            }
            gltf::image::Source::Uri { uri, .. } if uri.starts_with("data:") => {
                gltf::image::Data::from_source(image.source(), self.base, self.buffers)
                    .ok()
                    .and_then(to_rgba)
                    .map(|img| {
                        self.cache.textures.push(from_rgba(img));
                        self.cache.textures.len() - 1
                    })
            }
            gltf::image::Source::Uri { uri, .. } => {
                let name = percent_decode(uri);
                self.cache.from_file(&[&name])
            }
        };
        self.by_image.insert(idx, result);
        result
    }
}

fn to_rgba(d: gltf::image::Data) -> Option<image::RgbaImage> {
    use gltf::image::Format::*;
    let px: Vec<u8> = match d.format {
        R8G8B8A8 => d.pixels,
        R8G8B8 => d.pixels.chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
        R8G8 => d.pixels.chunks_exact(2).flat_map(|p| [p[0], p[0], p[0], p[1]]).collect(),
        R8 => d.pixels.iter().flat_map(|&p| [p, p, p, 255]).collect(),
        _ => return None,
    };
    image::RgbaImage::from_raw(d.width, d.height, px)
}

/// „Haar%20Textur.png“ → „Haar Textur.png“
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}
