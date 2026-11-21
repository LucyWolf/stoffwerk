//! Avatar laden: glTF/GLB/VRM (VRM ist intern glTF) und OBJ.
//! Ergebnis ist ein einzelnes Dreiecksnetz in Metern, Y nach oben.

use anyhow::{Context, Result, anyhow, bail};
use glam::{Mat4, Vec3};
use std::path::Path;

pub struct AvatarMesh {
    pub name: String,
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub colors: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
    pub min: Vec3,
    pub max: Vec3,
}

impl AvatarMesh {
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    pub fn height(&self) -> f32 {
        self.max.y - self.min.y
    }
}

const SKIN_GRAY: [f32; 3] = [0.78, 0.76, 0.74];

pub fn load(path: &Path) -> Result<AvatarMesh> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();

    let mut mesh = match ext.as_str() {
        "gltf" | "glb" | "vrm" => load_gltf(path)?,
        "obj" => load_obj(path)?,
        "fbx" => load_fbx(path)?,
        _ => bail!("Unbekanntes Dateiformat „.{ext}“. Erlaubt sind .fbx, .vrm, .glb, .gltf und .obj."),
    };
    if mesh.indices.is_empty() {
        bail!("In der Datei wurde kein Netz gefunden.");
    }
    mesh.name = name;
    finish(&mut mesh);
    Ok(mesh)
}

fn load_gltf(path: &Path) -> Result<AvatarMesh> {
    let (doc, buffers, _images) =
        gltf::import(path).with_context(|| format!("Datei konnte nicht gelesen werden: {}", path.display()))?;

    let mut out = empty();
    let scene = doc
        .default_scene()
        .or_else(|| doc.scenes().next())
        .context("Die Datei enthält keine Szene.")?;

    for node in scene.nodes() {
        visit_node(&node, Mat4::IDENTITY, &buffers, &mut out);
    }
    Ok(out)
}

fn visit_node(node: &gltf::Node, parent: Mat4, buffers: &[gltf::buffer::Data], out: &mut AvatarMesh) {
    let world = parent * Mat4::from_cols_array_2d(&node.transform().matrix());

    if let Some(mesh) = node.mesh() {
        // Bei Netzen mit Skelett gilt laut glTF-Regel nicht die Knoten-Transformation,
        // sondern die der Knochen. In der Ruhepose ergibt das die Punkte, wie sie in der Datei stehen.
        let xf = if node.skin().is_some() { Mat4::IDENTITY } else { world };
        let normal_xf = xf.inverse().transpose();

        for prim in mesh.primitives() {
            if prim.mode() != gltf::mesh::Mode::Triangles {
                continue;
            }
            let reader = prim.reader(|b| buffers.get(b.index()).map(|d| &d.0[..]));
            let Some(pos_iter) = reader.read_positions() else { continue };
            let base = out.positions.len() as u32;

            let positions: Vec<Vec3> = pos_iter.map(|p| xf.transform_point3(Vec3::from(p))).collect();
            let count = positions.len();

            let normals: Option<Vec<Vec3>> = reader.read_normals().map(|it| {
                it.map(|n| normal_xf.transform_vector3(Vec3::from(n)).normalize_or_zero())
                    .collect()
            });

            let f = prim.material().pbr_metallic_roughness().base_color_factor();
            let color = [
                SKIN_GRAY[0] * f[0],
                SKIN_GRAY[1] * f[1],
                SKIN_GRAY[2] * f[2],
            ];

            let indices: Vec<u32> = match reader.read_indices() {
                Some(idx) => idx.into_u32().collect(),
                None => (0..count as u32).collect(),
            };
            // Spiegelnde Transformation dreht die Dreiecke um
            let flip = xf.determinant() < 0.0;

            out.positions.extend(positions);
            match normals {
                Some(n) if n.len() == count => out.normals.extend(n),
                _ => out.normals.extend(std::iter::repeat_n(Vec3::ZERO, count)),
            }
            out.colors.extend(std::iter::repeat_n(color, count));
            for tri in indices.chunks_exact(3) {
                if tri.iter().any(|&i| i as usize >= count) {
                    continue;
                }
                if flip {
                    out.indices.extend([base + tri[0], base + tri[2], base + tri[1]]);
                } else {
                    out.indices.extend([base + tri[0], base + tri[1], base + tri[2]]);
                }
            }
        }
    }

    for child in node.children() {
        visit_node(&child, world, buffers, out);
    }
}

/// FBX über ufbx. ufbx rechnet Achsen und Einheiten gleich ins Netz um:
/// Unity- und Blender-FBX haben oft Z oben oder Zentimeter, hier kommt immer Y oben in Metern an.
fn load_fbx(path: &Path) -> Result<AvatarMesh> {
    let opts = ufbx::LoadOpts {
        target_axes: ufbx::CoordinateAxes::right_handed_y_up(),
        target_unit_meters: 1.0,
        space_conversion: ufbx::SpaceConversion::ModifyGeometry,
        generate_missing_normals: true,
        ..Default::default()
    };
    let file = path.to_str().context("Der Dateipfad enthält Zeichen, die nicht gelesen werden können.")?;
    let scene = ufbx::load_file(file, opts)
        .map_err(|e| anyhow!("FBX konnte nicht gelesen werden: {}", e.description))?;

    let mut out = empty();
    for node in scene.nodes.iter() {
        let Some(mesh) = node.mesh.as_ref() else { continue };
        let xf = &node.geometry_to_world;
        let nxf = ufbx::matrix_for_normals(xf);
        let mut tri = vec![0u32; mesh.max_face_triangles * 3];

        for (fi, face) in mesh.faces.iter().enumerate() {
            let n = mesh.triangulate_face(&mut tri, *face) as usize;
            let color = mesh
                .face_material
                .get(fi)
                .and_then(|&m| mesh.materials.get(m as usize))
                .map(|m| {
                    let c = m.fbx.diffuse_color.value_vec4;
                    let f = if m.fbx.diffuse_color.has_value { [c.x, c.y, c.z] } else { [1.0; 3] };
                    // Viele Avatare haben Weiß + Textur; dunkle Grundfarben sind meist Absicht (Haare, Kleidung)
                    [SKIN_GRAY[0] * f[0] as f32, SKIN_GRAY[1] * f[1] as f32, SKIN_GRAY[2] * f[2] as f32]
                })
                .unwrap_or(SKIN_GRAY);

            for &ix in &tri[..n * 3] {
                let ix = ix as usize;
                let p = ufbx::transform_position(xf, mesh.vertex_position[ix]);
                let nrm = if mesh.vertex_normal.exists {
                    let v = ufbx::transform_direction(&nxf, mesh.vertex_normal[ix]);
                    Vec3::new(v.x as f32, v.y as f32, v.z as f32).normalize_or_zero()
                } else {
                    Vec3::ZERO
                };
                out.indices.push(out.positions.len() as u32);
                out.positions.push(Vec3::new(p.x as f32, p.y as f32, p.z as f32));
                out.normals.push(nrm);
                out.colors.push(color);
            }
        }
    }
    Ok(out)
}

fn load_obj(path: &Path) -> Result<AvatarMesh> {
    let (models, _) = tobj::load_obj(
        path,
        &tobj::LoadOptions { triangulate: true, single_index: true, ..Default::default() },
    )
    .with_context(|| format!("Datei konnte nicht gelesen werden: {}", path.display()))?;

    let mut out = empty();
    for m in models {
        let mesh = m.mesh;
        let base = out.positions.len() as u32;
        let count = mesh.positions.len() / 3;
        out.positions
            .extend(mesh.positions.chunks_exact(3).map(|p| Vec3::new(p[0], p[1], p[2])));
        if mesh.normals.len() == mesh.positions.len() {
            out.normals
                .extend(mesh.normals.chunks_exact(3).map(|n| Vec3::new(n[0], n[1], n[2]).normalize_or_zero()));
        } else {
            out.normals.extend(std::iter::repeat_n(Vec3::ZERO, count));
        }
        out.colors.extend(std::iter::repeat_n(SKIN_GRAY, count));
        out.indices.extend(mesh.indices.iter().map(|i| base + i));
    }
    Ok(out)
}

fn empty() -> AvatarMesh {
    AvatarMesh {
        name: String::new(),
        positions: Vec::new(),
        normals: Vec::new(),
        colors: Vec::new(),
        indices: Vec::new(),
        min: Vec3::ZERO,
        max: Vec3::ZERO,
    }
}

/// Fehlende Normalen berechnen, Maße bestimmen, und Zentimeter-Modelle auf Meter bringen.
fn finish(mesh: &mut AvatarMesh) {
    // Normalen für Punkte ohne eigene Normale aus den Dreiecken mitteln
    let missing: Vec<bool> = mesh.normals.iter().map(|n| *n == Vec3::ZERO).collect();
    if missing.iter().any(|&m| m) {
        let mut acc = vec![Vec3::ZERO; mesh.positions.len()];
        for t in mesh.indices.chunks_exact(3) {
            let (a, b, c) = (t[0] as usize, t[1] as usize, t[2] as usize);
            let n = (mesh.positions[b] - mesh.positions[a]).cross(mesh.positions[c] - mesh.positions[a]);
            acc[a] += n;
            acc[b] += n;
            acc[c] += n;
        }
        for (i, m) in missing.iter().enumerate() {
            if *m {
                mesh.normals[i] = acc[i].normalize_or(Vec3::Y);
            }
        }
    }

    let (mut min, mut max) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for p in &mesh.positions {
        min = min.min(*p);
        max = max.max(*p);
    }

    // Ein Mensch ist 0,5 bis 3 m groß. Alles über 20 „Meter“ ist fast sicher in Zentimetern gespeichert.
    let height = max.y - min.y;
    if height > 20.0 {
        let s = 0.01;
        for p in &mut mesh.positions {
            *p *= s;
        }
        min *= s;
        max *= s;
    }

    // Füße auf den Boden
    let lift = -min.y;
    if lift.abs() > 1e-4 {
        for p in &mut mesh.positions {
            p.y += lift;
        }
        min.y += lift;
        max.y += lift;
    }

    mesh.min = min;
    mesh.max = max;
}
