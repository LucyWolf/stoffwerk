//! USD (.usd, .usda, .usdc, .usdz) über `openusd`.
//! Gelesen werden Mesh-Prims mit Transformationen, UVs und der Grundfarbe aus UsdPreviewSurface.

use super::texture::TextureCache;
use super::{AvatarMesh, SKIN_GRAY};
use anyhow::{Result, anyhow};
use glam::{DMat4, DQuat, DVec3, Mat4, Vec2, Vec3};
use openusd::usd::{Prim, Stage};
use openusd::{sdf, vt};
use std::path::Path;

pub fn load(path: &Path, opts: &super::ImportOptions) -> Result<AvatarMesh> {
    let file = path.to_string_lossy().into_owned();
    let stage = Stage::open(&file).map_err(|e| anyhow!("USD konnte nicht gelesen werden: {e}"))?;

    // Achse und Einheit der Szene: USD erlaubt Z oben und beliebige Einheiten
    let root = sdf::Path::abs_root();
    let up_z = stage
        .field::<vt::Value>(root.clone(), "upAxis")
        .ok()
        .flatten()
        .is_some_and(|v| matches!(&v, vt::Value::Token(t) if t.as_str() == "Z"));
    let meters = match stage.field::<vt::Value>(root.clone(), "metersPerUnit").ok().flatten() {
        Some(vt::Value::Double(d)) if d > 0.0 => d,
        Some(vt::Value::Float(f)) if f > 0.0 => f as f64,
        _ => 0.01, // USD-Vorgabe sind Zentimeter
    };
    let mut fix = DMat4::from_scale(DVec3::splat(if opts.auto_unit { meters } else { 1.0 }));
    if up_z && opts.auto_axes {
        fix = DMat4::from_rotation_x(-std::f64::consts::FRAC_PI_2) * fix;
    }

    let mut ctx = Ctx { out: AvatarMesh::new(), cache: TextureCache::new(path), stage: &stage };
    let top = stage.prim(root).map_err(|e| anyhow!("USD: {e}"))?;
    for child in top.children().map_err(|e| anyhow!("USD: {e}"))? {
        ctx.visit(&child, fix);
    }
    let mut out = ctx.out;
    out.textures = ctx.cache.textures;
    out.warnings.extend(ctx.cache.missing.iter().map(|m| format!("Textur fehlt: {m}")));
    Ok(out)
}

struct Ctx<'a> {
    out: AvatarMesh,
    cache: TextureCache,
    stage: &'a Stage,
}

fn value(prim: &Prim, name: &str) -> Option<vt::Value> {
    prim.attribute(name).get::<vt::Value>().ok().flatten()
}

fn vec3s(v: Option<vt::Value>) -> Option<Vec<Vec3>> {
    match v? {
        vt::Value::Vec3fVec(a) => Some(a.iter().map(|p| Vec3::new(p.x, p.y, p.z)).collect()),
        vt::Value::Vec3dVec(a) => Some(a.iter().map(|p| Vec3::new(p.x as f32, p.y as f32, p.z as f32)).collect()),
        _ => None,
    }
}

fn vec2s(v: Option<vt::Value>) -> Option<Vec<Vec2>> {
    match v? {
        vt::Value::Vec2fVec(a) => Some(a.iter().map(|p| Vec2::new(p.x, p.y)).collect()),
        vt::Value::Vec2dVec(a) => Some(a.iter().map(|p| Vec2::new(p.x as f32, p.y as f32)).collect()),
        _ => None,
    }
}

fn ints(v: Option<vt::Value>) -> Option<Vec<i32>> {
    match v? {
        vt::Value::IntVec(a) => Some(a),
        _ => None,
    }
}

fn token(v: Option<vt::Value>) -> Option<String> {
    match v? {
        vt::Value::Token(t) => Some(t.as_str().to_string()),
        vt::Value::String(s) => Some(s),
        _ => None,
    }
}

fn dvec3(v: Option<vt::Value>) -> Option<DVec3> {
    match v? {
        vt::Value::Vec3f(p) => Some(DVec3::new(p.x as f64, p.y as f64, p.z as f64)),
        vt::Value::Vec3d(p) => Some(DVec3::new(p.x, p.y, p.z)),
        vt::Value::Float(f) => Some(DVec3::splat(f as f64)),
        vt::Value::Double(d) => Some(DVec3::splat(d)),
        _ => None,
    }
}

fn scalar(v: Option<vt::Value>) -> Option<f64> {
    match v? {
        vt::Value::Float(f) => Some(f as f64),
        vt::Value::Double(d) => Some(d),
        _ => None,
    }
}

/// Lokale Transformation aus xformOpOrder. USD multipliziert Zeilenvektoren; in Spaltenschreibweise
/// wird die Liste von links nach rechts aufmultipliziert (erste Operation = äußerste).
fn local_xform(prim: &Prim) -> DMat4 {
    let order: Vec<String> = match value(prim, "xformOpOrder") {
        Some(vt::Value::TokenVec(t)) => t.iter().map(|t| t.as_str().to_string()).collect(),
        _ => return DMat4::IDENTITY,
    };
    let mut m = DMat4::IDENTITY;
    for op in order {
        if op == "!resetXformStack!" {
            continue;
        }
        let (invert, name) = match op.strip_prefix("!invert!") {
            Some(n) => (true, n.to_string()),
            None => (false, op.clone()),
        };
        let kind = name.trim_start_matches("xformOp:").split(':').next().unwrap_or("").to_string();
        let v = value(prim, &name);
        let deg = |a: f64| a.to_radians();
        let mut t = match kind.as_str() {
            "translate" => DMat4::from_translation(dvec3(v).unwrap_or(DVec3::ZERO)),
            "scale" => DMat4::from_scale(dvec3(v).unwrap_or(DVec3::ONE)),
            "rotateX" => DMat4::from_rotation_x(deg(scalar(v).unwrap_or(0.0))),
            "rotateY" => DMat4::from_rotation_y(deg(scalar(v).unwrap_or(0.0))),
            "rotateZ" => DMat4::from_rotation_z(deg(scalar(v).unwrap_or(0.0))),
            k if k.starts_with("rotate") && k.len() == 9 => {
                let a = dvec3(v).unwrap_or(DVec3::ZERO);
                // rotateXYZ: erst X, dann Y, dann Z (bezogen auf den Punkt)
                let mut r = DMat4::IDENTITY;
                for c in k[6..].chars() {
                    let step = match c {
                        'X' => DMat4::from_rotation_x(deg(a.x)),
                        'Y' => DMat4::from_rotation_y(deg(a.y)),
                        _ => DMat4::from_rotation_z(deg(a.z)),
                    };
                    r = step * r;
                }
                r
            }
            "orient" => match v {
                Some(vt::Value::Quatf(q)) => {
                    DMat4::from_quat(DQuat::from_xyzw(q.x as f64, q.y as f64, q.z as f64, q.w as f64).normalize())
                }
                Some(vt::Value::Quatd(q)) => DMat4::from_quat(DQuat::from_xyzw(q.x, q.y, q.z, q.w).normalize()),
                _ => DMat4::IDENTITY,
            },
            // Zeilenvektor-Matrix: als Spaltenmatrix gelesen ist sie schon richtig herum
            "transform" => match v {
                Some(vt::Value::Matrix4d(mm)) => DMat4::from_cols_array(&mm.0),
                _ => DMat4::IDENTITY,
            },
            _ => DMat4::IDENTITY,
        };
        if invert {
            t = t.inverse();
        }
        m *= t;
    }
    m
}

impl Ctx<'_> {
    fn visit(&mut self, prim: &Prim, parent: DMat4) {
        if !prim.is_active().unwrap_or(true) {
            return;
        }
        let world = parent * local_xform(prim);
        let ty = prim.type_name().ok().flatten().map(|t| t.as_str().to_string()).unwrap_or_default();
        if ty == "Mesh" {
            self.mesh(prim, world.as_mat4());
        }
        if let Ok(children) = prim.children() {
            for c in children {
                self.visit(&c, world);
            }
        }
    }

    fn mesh(&mut self, prim: &Prim, xf: Mat4) {
        let Some(points) = vec3s(value(prim, "points")) else { return };
        let (Some(counts), Some(idx)) = (ints(value(prim, "faceVertexCounts")), ints(value(prim, "faceVertexIndices")))
        else {
            return;
        };
        let left = token(value(prim, "orientation")).as_deref() == Some("leftHanded");

        let normals = vec3s(value(prim, "normals")).or_else(|| vec3s(value(prim, "primvars:normals")));
        let n_interp = token(prim.attribute("normals").metadata::<vt::Value>("interpolation").ok().flatten())
            .unwrap_or_else(|| "vertex".into());
        let st = vec2s(value(prim, "primvars:st")).or_else(|| vec2s(value(prim, "primvars:UVMap")));
        let st_name = if value(prim, "primvars:st").is_some() { "primvars:st" } else { "primvars:UVMap" };
        let st_idx = ints(value(prim, &format!("{st_name}:indices")));
        let st_interp = token(prim.attribute(st_name).metadata::<vt::Value>("interpolation").ok().flatten())
            .unwrap_or_else(|| "faceVarying".into());

        let (texture, color, cutout) = self.material(prim);
        let nxf = xf.inverse().transpose();
        let flip = (xf.determinant() < 0.0) ^ left;

        let first = self.out.indices.len();
        let mut corner = 0usize;
        for (face, &c) in counts.iter().enumerate() {
            let c = c.max(0) as usize;
            let mut verts = Vec::with_capacity(c);
            for k in 0..c {
                let fv = corner + k;
                let Some(&pi) = idx.get(fv) else { break };
                let pi = pi.max(0) as usize;
                let Some(&p) = points.get(pi) else { break };
                let n = normals
                    .as_ref()
                    .and_then(|n| match n_interp.as_str() {
                        "faceVarying" => n.get(fv),
                        "uniform" => n.get(face),
                        _ => n.get(pi),
                    })
                    .map(|n| nxf.transform_vector3(*n).normalize_or_zero())
                    .unwrap_or(Vec3::ZERO);
                let uv = st
                    .as_ref()
                    .and_then(|s| {
                        let i = match st_interp.as_str() {
                            "faceVarying" => fv,
                            "uniform" => face,
                            _ => pi,
                        };
                        let i = st_idx.as_ref().and_then(|x| x.get(i)).map(|&j| j as usize).unwrap_or(i);
                        s.get(i)
                    })
                    .map(|u| Vec2::new(u.x, 1.0 - u.y))
                    .unwrap_or(Vec2::ZERO);
                verts.push(self.out.push_vertex(xf.transform_point3(p), n, uv, color));
            }
            corner += c;
            for k in 1..verts.len().saturating_sub(1) {
                let (a, b, cc) = (verts[0], verts[k], verts[k + 1]);
                if flip {
                    self.out.indices.extend([a, cc, b]);
                } else {
                    self.out.indices.extend([a, b, cc]);
                }
            }
        }
        self.out.close_part(first, texture, cutout);
    }

    /// material:binding → Material → UsdPreviewSurface → diffuseColor (Farbe oder UsdUVTexture)
    fn material(&mut self, prim: &Prim) -> (Option<usize>, [f32; 3], bool) {
        let fallback = (None, SKIN_GRAY, false);
        let Some(mat_path) = prim.relationship("material:binding").targets().ok().and_then(|t| t.into_iter().next())
        else {
            return fallback;
        };
        let Ok(mat) = self.stage.prim(mat_path) else { return fallback };
        // Den Shader finden, der eine UsdPreviewSurface ist
        let shaders = mat.children().unwrap_or_default();
        let Some(surface) = shaders
            .iter()
            .find(|s| token(value(s, "info:id")).as_deref() == Some("UsdPreviewSurface"))
        else {
            return fallback;
        };
        let diffuse = surface.attribute("inputs:diffuseColor");
        if let Some(src) = diffuse.connections().ok().and_then(|c| c.into_iter().next()) {
            let tex_prim_path = src.prim_path();
            if let Ok(tex) = self.stage.prim(tex_prim_path) {
                if let Some(vt::Value::AssetPath(a)) = value(&tex, "inputs:file") {
                    let name = a.authored_path.clone();
                    let t = self.cache.from_file(&[&name]);
                    let cutout = t.is_some_and(|i| self.cache.textures[i].has_alpha);
                    return (t, [1.0; 3], cutout);
                }
            }
        }
        match value(surface, "inputs:diffuseColor") {
            Some(vt::Value::Vec3f(c)) => (None, [c.x, c.y, c.z], false),
            _ => fallback,
        }
    }
}
