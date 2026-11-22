//! Avatar laden. Jedes Format liefert dasselbe Ergebnis: ein Dreiecksnetz in Metern, Y nach oben,
//! aufgeteilt in Teile mit je einem Material (Farbe, Textur, ausgestanzte Ränder wie bei Haaren).

mod abc;
mod dae;
mod fbx;
mod gltf;
mod obj;
pub mod texture;
mod usd;

use anyhow::{Result, bail};
use glam::{Vec2, Vec3};
use std::path::Path;
pub use texture::TextureImage;

/// Grundfarbe für Flächen ohne Textur: neutrales Hellgrau, leicht warm
pub const SKIN_GRAY: [f32; 3] = [0.78, 0.76, 0.74];

/// Alle Endungen, die geladen werden können (für den Dateidialog)
pub const EXTENSIONS: &[&str] = &["fbx", "vrm", "glb", "gltf", "obj", "dae", "usd", "usda", "usdc", "usdz", "abc"];

pub struct Part {
    /// Erster Index in `indices`
    pub first: u32,
    pub count: u32,
    pub texture: Option<usize>,
    /// Pixel mit wenig Deckkraft werden weggelassen (Haarsträhnen, Wimpern, Spitze)
    pub cutout: bool,
}

pub struct AvatarMesh {
    pub name: String,
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub uvs: Vec<Vec2>,
    /// Farbe des Materials (mit Textur multipliziert)
    pub colors: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
    pub parts: Vec<Part>,
    pub textures: Vec<TextureImage>,
    pub min: Vec3,
    pub max: Vec3,
    /// Hinweise, die den Avatar nicht verhindern (fehlende Texturen u. ä.)
    pub warnings: Vec<String>,
}

impl AvatarMesh {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            positions: Vec::new(),
            normals: Vec::new(),
            uvs: Vec::new(),
            colors: Vec::new(),
            indices: Vec::new(),
            parts: Vec::new(),
            textures: Vec::new(),
            min: Vec3::ZERO,
            max: Vec3::ZERO,
            warnings: Vec::new(),
        }
    }

    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    pub fn height(&self) -> f32 {
        self.max.y - self.min.y
    }

    /// Hängt einen Punkt an und gibt seine Nummer zurück
    pub fn push_vertex(&mut self, p: Vec3, n: Vec3, uv: Vec2, color: [f32; 3]) -> u32 {
        self.positions.push(p);
        self.normals.push(n);
        self.uvs.push(uv);
        self.colors.push(color);
        (self.positions.len() - 1) as u32
    }

    /// Schließt einen Teil ab: alle Indizes seit `first` bekommen dieses Material
    pub fn close_part(&mut self, first: usize, texture: Option<usize>, cutout: bool) {
        let count = self.indices.len() - first;
        if count > 0 {
            self.parts.push(Part { first: first as u32, count: count as u32, texture, cutout });
        }
    }
}

pub fn load(path: &Path) -> Result<AvatarMesh> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();

    let mut mesh = match ext.as_str() {
        "gltf" | "glb" | "vrm" => gltf::load(path)?,
        "fbx" => fbx::load(path)?,
        "obj" => obj::load(path)?,
        "dae" => dae::load(path)?,
        "usd" | "usda" | "usdc" | "usdz" => usd::load(path)?,
        "abc" => abc::load(path)?,
        _ => bail!(
            "Unbekanntes Dateiformat „.{ext}“. Stoffwerk liest {}.",
            EXTENSIONS.iter().map(|e| format!(".{e}")).collect::<Vec<_>>().join(", ")
        ),
    };
    if mesh.indices.is_empty() {
        bail!("In der Datei wurde kein Netz gefunden.");
    }
    mesh.name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    finish(&mut mesh);
    Ok(mesh)
}

/// Fehlende Normalen berechnen, Maße bestimmen, Zentimeter auf Meter bringen, Füße auf den Boden.
fn finish(mesh: &mut AvatarMesh) {
    if mesh.parts.is_empty() {
        mesh.parts.push(Part { first: 0, count: mesh.indices.len() as u32, texture: None, cutout: false });
    }

    let missing: Vec<bool> = mesh.normals.iter().map(|n| *n == Vec3::ZERO || !n.is_finite()).collect();
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
    if max.y - min.y > 20.0 {
        let s = 0.01;
        mesh.positions.iter_mut().for_each(|p| *p *= s);
        min *= s;
        max *= s;
    }

    let lift = -min.y;
    if lift.abs() > 1e-4 {
        mesh.positions.iter_mut().for_each(|p| p.y += lift);
        min.y += lift;
        max.y += lift;
    }

    mesh.min = min;
    mesh.max = max;
}
