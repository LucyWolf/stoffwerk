//! Collada (.dae), eigener kleiner Leser auf Basis von roxmltree.
//! Gelesen werden: Szene mit Transformationen, Netze (triangles, polylist, polygons), skinned Netze über
//! den Controller, Einheit und Hochachse, Grundfarbe und -textur aus dem Effekt.

use super::texture::TextureCache;
use super::{AvatarMesh, SKIN_GRAY};
use anyhow::{Context, Result, anyhow};
use glam::{Mat4, Vec2, Vec3, Vec4};
use roxmltree::{Document, Node};
use std::collections::HashMap;
use std::path::Path;

pub fn load(path: &Path) -> Result<AvatarMesh> {
    let text = std::fs::read_to_string(path).with_context(|| format!("Datei konnte nicht gelesen werden: {}", path.display()))?;
    let doc = Document::parse_with_options(&text, roxmltree::ParsingOptions { allow_dtd: true, ..Default::default() })
        .map_err(|e| anyhow!("Keine gültige Collada-Datei: {e}"))?;
    let root = doc.root_element();

    let mut ids: HashMap<&str, Node> = HashMap::new();
    for n in root.descendants() {
        if let Some(id) = n.attribute("id") {
            ids.insert(id, n);
        }
    }

    // Einheit und Hochachse
    let asset = child(root, "asset");
    let meter = asset
        .and_then(|a| child(a, "unit"))
        .and_then(|u| u.attribute("meter"))
        .and_then(|m| m.parse::<f32>().ok())
        .unwrap_or(1.0);
    let up = asset.and_then(|a| child(a, "up_axis")).and_then(|u| u.text()).unwrap_or("Y_UP").trim().to_string();
    let mut fix = Mat4::from_scale(Vec3::splat(meter));
    match up.as_str() {
        "Z_UP" => fix = Mat4::from_rotation_x(-std::f32::consts::FRAC_PI_2) * fix,
        "X_UP" => fix = Mat4::from_rotation_z(std::f32::consts::FRAC_PI_2) * fix,
        _ => {}
    }

    let mut ctx = Ctx { ids, out: AvatarMesh::new(), cache: TextureCache::new(path), materials: HashMap::new() };

    let scene_url = child(root, "scene")
        .and_then(|s| child(s, "instance_visual_scene"))
        .and_then(|i| i.attribute("url"))
        .map(|u| u.trim_start_matches('#').to_string());
    let scene = scene_url
        .and_then(|u| ctx.ids.get(u.as_str()).copied())
        .or_else(|| root.descendants().find(|n| n.has_tag_name("visual_scene")))
        .context("Die Collada-Datei enthält keine Szene.")?;

    for node in scene.children().filter(|n| n.has_tag_name("node")) {
        ctx.node(node, fix, 0);
    }

    let mut out = ctx.out;
    out.textures = ctx.cache.textures;
    out.warnings.extend(ctx.cache.missing.iter().map(|m| format!("Textur fehlt: {m}")));
    Ok(out)
}

fn child<'a, 'i>(n: Node<'a, 'i>, tag: &str) -> Option<Node<'a, 'i>> {
    n.children().find(|c| c.has_tag_name(tag))
}

fn floats(n: Node) -> Vec<f32> {
    n.text().unwrap_or("").split_ascii_whitespace().filter_map(|s| s.parse().ok()).collect()
}

fn uints(n: Node) -> Vec<u32> {
    n.text().unwrap_or("").split_ascii_whitespace().filter_map(|s| s.parse().ok()).collect()
}

/// Collada-Matrizen stehen zeilenweise
fn mat(v: &[f32]) -> Mat4 {
    if v.len() < 16 {
        return Mat4::IDENTITY;
    }
    Mat4::from_cols_array(&v[..16].try_into().unwrap()).transpose()
}

struct Ctx<'a, 'i> {
    ids: HashMap<&'a str, Node<'a, 'i>>,
    out: AvatarMesh,
    cache: TextureCache,
    materials: HashMap<String, (Option<usize>, [f32; 3], bool)>,
}

impl<'a, 'i> Ctx<'a, 'i> {
    fn get(&self, url: &str) -> Option<Node<'a, 'i>> {
        self.ids.get(url.trim_start_matches('#')).copied()
    }

    fn node(&mut self, node: Node<'a, 'i>, parent: Mat4, depth: u32) {
        if depth > 128 {
            return;
        }
        let mut local = Mat4::IDENTITY;
        for t in node.children().filter(|c| c.is_element()) {
            let v = floats(t);
            let m = match t.tag_name().name() {
                "matrix" => mat(&v),
                "translate" if v.len() >= 3 => Mat4::from_translation(Vec3::new(v[0], v[1], v[2])),
                "rotate" if v.len() >= 4 => {
                    Mat4::from_axis_angle(Vec3::new(v[0], v[1], v[2]).normalize_or(Vec3::Y), v[3].to_radians())
                }
                "scale" if v.len() >= 3 => Mat4::from_scale(Vec3::new(v[0], v[1], v[2])),
                _ => continue,
            };
            local *= m;
        }
        let world = parent * local;

        for inst in node.children().filter(|c| c.is_element()) {
            match inst.tag_name().name() {
                "instance_geometry" => {
                    if let Some(g) = inst.attribute("url").and_then(|u| self.get(u)) {
                        let binds = self.bindings(inst);
                        self.geometry(g, world, &binds);
                    }
                }
                // Skinned Netz: Geometrie hängt am Controller, Ruhepose = bind_shape_matrix
                "instance_controller" => {
                    let ctrl = inst.attribute("url").and_then(|u| self.get(u));
                    let skin = ctrl.and_then(|c| child(c, "skin"));
                    if let Some(skin) = skin {
                        let bind = child(skin, "bind_shape_matrix").map(|m| mat(&floats(m))).unwrap_or(Mat4::IDENTITY);
                        if let Some(g) = skin.attribute("source").and_then(|u| self.get(u)) {
                            let binds = self.bindings(inst);
                            self.geometry(g, world * bind, &binds);
                        }
                    }
                }
                "node" => self.node(inst, world, depth + 1),
                "instance_node" => {
                    if let Some(n) = inst.attribute("url").and_then(|u| self.get(u)) {
                        self.node(n, world, depth + 1);
                    }
                }
                _ => {}
            }
        }
    }

    /// symbol → Material-ID aus bind_material
    fn bindings(&self, inst: Node) -> HashMap<String, String> {
        inst.descendants()
            .filter(|n| n.has_tag_name("instance_material"))
            .filter_map(|m| Some((m.attribute("symbol")?.to_string(), m.attribute("target")?.trim_start_matches('#').to_string())))
            .collect()
    }

    fn source(&self, url: &str) -> Option<(Vec<f32>, usize)> {
        let s = self.get(url)?;
        let arr = s.children().find(|c| c.has_tag_name("float_array"))?;
        let stride = s
            .descendants()
            .find(|c| c.has_tag_name("accessor"))
            .and_then(|a| a.attribute("stride"))
            .and_then(|v| v.parse().ok())
            .unwrap_or(3);
        Some((floats(arr), stride))
    }

    fn geometry(&mut self, geom: Node<'a, 'i>, xf: Mat4, binds: &HashMap<String, String>) {
        let Some(mesh) = child(geom, "mesh") else { return };
        let nxf = xf.inverse().transpose();
        let flip = xf.determinant() < 0.0;

        // <vertices> → POSITION (und manchmal NORMAL/TEXCOORD)
        let mut vert_inputs: HashMap<String, String> = HashMap::new();
        if let Some(v) = child(mesh, "vertices") {
            for i in v.children().filter(|c| c.has_tag_name("input")) {
                if let (Some(sem), Some(src)) = (i.attribute("semantic"), i.attribute("source")) {
                    vert_inputs.insert(sem.to_string(), src.to_string());
                }
            }
        }

        for prim in mesh.children().filter(|c| c.is_element()) {
            let kind = prim.tag_name().name();
            if !matches!(kind, "triangles" | "polylist" | "polygons") {
                continue;
            }
            let mut offs: HashMap<&str, (usize, String)> = HashMap::new();
            let mut stride = 0;
            for i in prim.children().filter(|c| c.has_tag_name("input")) {
                let off: usize = i.attribute("offset").and_then(|o| o.parse().ok()).unwrap_or(0);
                stride = stride.max(off + 1);
                let sem = i.attribute("semantic").unwrap_or("");
                let src = i.attribute("source").unwrap_or("").to_string();
                // Bei mehreren UV-Sätzen den ersten nehmen
                if sem == "TEXCOORD" && offs.contains_key("TEXCOORD") {
                    continue;
                }
                offs.insert(sem, (off, src));
            }
            let Some((v_off, _)) = offs.get("VERTEX").cloned() else { continue };
            let Some(pos) = vert_inputs.get("POSITION").and_then(|u| self.source(u)) else { continue };
            let norm = offs
                .get("NORMAL")
                .map(|(o, s)| (*o, s.clone()))
                .or_else(|| vert_inputs.get("NORMAL").map(|s| (v_off, s.clone())))
                .and_then(|(o, s)| self.source(&s).map(|d| (o, d)));
            let uv = offs
                .get("TEXCOORD")
                .map(|(o, s)| (*o, s.clone()))
                .or_else(|| vert_inputs.get("TEXCOORD").map(|s| (v_off, s.clone())))
                .and_then(|(o, s)| self.source(&s).map(|d| (o, d)));

            // Polygone: Liste von Eckenlisten
            let polys: Vec<Vec<u32>> = match kind {
                "triangles" => child(prim, "p").map(uints).unwrap_or_default().chunks(stride * 3).map(|c| c.to_vec()).collect(),
                "polylist" => {
                    let vc = child(prim, "vcount").map(uints).unwrap_or_default();
                    let p = child(prim, "p").map(uints).unwrap_or_default();
                    let mut at = 0;
                    vc.iter()
                        .map(|&n| {
                            let end = (at + n as usize * stride).min(p.len());
                            let v = p[at.min(p.len())..end].to_vec();
                            at = end;
                            v
                        })
                        .collect()
                }
                _ => prim.children().filter(|c| c.has_tag_name("p")).map(uints).collect(),
            };

            let (texture, color, cutout) = match prim.attribute("material") {
                Some(sym) => {
                    let id = binds.get(sym).cloned().unwrap_or_else(|| sym.to_string());
                    self.material(&id)
                }
                None => (None, SKIN_GRAY, false),
            };

            let first = self.out.indices.len();
            for poly in polys {
                let corners: Vec<u32> = poly
                    .chunks_exact(stride)
                    .filter_map(|c| {
                        let pi = c[v_off] as usize;
                        let p = pos.0.get(pi * pos.1..pi * pos.1 + 3)?;
                        let n = norm
                            .as_ref()
                            .and_then(|(o, (d, s))| d.get(c[*o] as usize * s..c[*o] as usize * s + 3))
                            .map(|n| nxf.transform_vector3(Vec3::new(n[0], n[1], n[2])).normalize_or_zero())
                            .unwrap_or(Vec3::ZERO);
                        let t = uv
                            .as_ref()
                            .and_then(|(o, (d, s))| d.get(c[*o] as usize * s..c[*o] as usize * s + 2))
                            .map(|t| Vec2::new(t[0], 1.0 - t[1]))
                            .unwrap_or(Vec2::ZERO);
                        let p = xf * Vec4::new(p[0], p[1], p[2], 1.0);
                        Some(self.out.push_vertex(p.truncate(), n, t, color))
                    })
                    .collect();
                for k in 1..corners.len().saturating_sub(1) {
                    let (a, b, c) = (corners[0], corners[k], corners[k + 1]);
                    if flip {
                        self.out.indices.extend([a, c, b]);
                    } else {
                        self.out.indices.extend([a, b, c]);
                    }
                }
            }
            self.out.close_part(first, texture, cutout);
        }
    }

    /// material → instance_effect → effect → diffuse (color oder texture → sampler → surface → image)
    fn material(&mut self, id: &str) -> (Option<usize>, [f32; 3], bool) {
        if let Some(m) = self.materials.get(id) {
            return *m;
        }
        let result = self.material_inner(id).unwrap_or((None, SKIN_GRAY, false));
        self.materials.insert(id.to_string(), result);
        result
    }

    fn material_inner(&mut self, id: &str) -> Option<(Option<usize>, [f32; 3], bool)> {
        let mat = self.get(id)?;
        let effect = self.get(child(mat, "instance_effect")?.attribute("url")?)?;
        let diffuse = effect.descendants().find(|n| n.has_tag_name("diffuse"))?;
        if let Some(c) = child(diffuse, "color") {
            let v = floats(c);
            if v.len() >= 3 {
                return Some((None, [SKIN_GRAY[0] * v[0], SKIN_GRAY[1] * v[1], SKIN_GRAY[2] * v[2]], false));
            }
        }
        let tex = child(diffuse, "texture")?.attribute("texture")?;
        // sampler sid → surface sid → image id (Collada 1.4); in 1.5 oder bei Blender oft direkt die Bild-ID
        let by_sid = |sid: &str| effect.descendants().find(|n| n.attribute("sid") == Some(sid));
        let mut image_id = tex.to_string();
        if let Some(sampler) = by_sid(tex) {
            let source = sampler.descendants().find(|n| n.has_tag_name("source")).and_then(|s| s.text());
            if let Some(surface) = source.and_then(|s| by_sid(s.trim())) {
                if let Some(init) = surface.descendants().find(|n| n.has_tag_name("init_from")).and_then(|n| n.text()) {
                    image_id = init.trim().to_string();
                }
            }
        }
        let image = self.get(&image_id)?;
        let file = image
            .descendants()
            .find(|n| n.has_tag_name("init_from"))
            .and_then(|n| n.descendants().find(|d| d.is_text()).and_then(|t| t.text()))?
            .trim()
            .trim_start_matches("file://")
            .to_string();
        let t = self.cache.from_file(&[&file]);
        let cutout = t.is_some_and(|i| self.cache.textures[i].has_alpha);
        Some((t, if t.is_some() { [1.0; 3] } else { SKIN_GRAY }, cutout))
    }
}
