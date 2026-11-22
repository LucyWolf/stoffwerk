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
use serde::{Deserialize, Serialize};
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

/// Einheit, in der die Zahlen in der Datei gemeint sind
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Unit {
    Millimeter,
    Centimeter,
    Meter,
    Inch,
    Foot,
}

impl Unit {
    pub const ALL: [Unit; 5] = [Unit::Millimeter, Unit::Centimeter, Unit::Meter, Unit::Inch, Unit::Foot];

    pub fn meters(self) -> f32 {
        match self {
            Unit::Millimeter => 0.001,
            Unit::Centimeter => 0.01,
            Unit::Meter => 1.0,
            Unit::Inch => 0.0254,
            Unit::Foot => 0.3048,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Unit::Millimeter => "Millimeter",
            Unit::Centimeter => "Zentimeter (DAZ, 3ds Max)",
            Unit::Meter => "Meter (Blender, Unity, VRM)",
            Unit::Inch => "Zoll",
            Unit::Foot => "Fuß",
        }
    }
}

/// Einstellungen aus dem Import-Fenster
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImportOptions {
    /// Einheit aus der Datei übernehmen bzw. erkennen
    pub auto_unit: bool,
    pub unit: Unit,
    /// Zusätzliche Skalierung in Prozent
    pub percent: f32,
    /// Achsen aus der Datei übernehmen
    pub auto_axes: bool,
    /// Für Ziel-X, -Y (oben), -Z: welche Achse der Datei (0 = X, 1 = Y, 2 = Z) und ob umgekehrt
    pub axes: [(usize, bool); 3],
    /// Füße auf die Bodenebene
    pub ground: bool,
}

impl Default for ImportOptions {
    fn default() -> Self {
        Self {
            auto_unit: true,
            unit: Unit::Meter,
            percent: 100.0,
            auto_axes: true,
            axes: [(0, false), (1, false), (2, false)],
            ground: true,
        }
    }
}

impl ImportOptions {
    /// Jede Achse der Datei genau einmal verwendet?
    pub fn axes_valid(&self) -> bool {
        let mut seen = [false; 3];
        for (src, _) in self.axes {
            if src > 2 || seen[src] {
                return false;
            }
            seen[src] = true;
        }
        true
    }
}

pub fn load(path: &Path, opts: &ImportOptions) -> Result<AvatarMesh> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    if !opts.auto_axes && !opts.axes_valid() {
        bail!("Achsen: Jede Achse der Datei darf nur einmal zugeordnet werden.");
    }

    let mut mesh = match ext.as_str() {
        "gltf" | "glb" | "vrm" => gltf::load(path)?,
        "fbx" => fbx::load(path, opts)?,
        "obj" => obj::load(path)?,
        "dae" => dae::load(path, opts)?,
        "usd" | "usda" | "usdc" | "usdz" => usd::load(path, opts)?,
        "abc" => abc::load(path, opts)?,
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
    finish(&mut mesh, opts);
    Ok(mesh)
}

/// Fehlende Normalen berechnen, Achsen und Einheit anwenden, Füße auf den Boden.
fn finish(mesh: &mut AvatarMesh, opts: &ImportOptions) {
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

    // Achsen von Hand: Ziel-Achse k bekommt Achse axes[k] der Datei
    if !opts.auto_axes {
        let map = |v: Vec3| {
            let a = v.to_array();
            Vec3::from_array(std::array::from_fn(|k| {
                let (src, inv) = opts.axes[k];
                if inv { -a[src] } else { a[src] }
            }))
        };
        mesh.positions.iter_mut().for_each(|p| *p = map(*p));
        mesh.normals.iter_mut().for_each(|n| *n = map(*n));
        // Eine Spiegelung dreht den Umlaufsinn der Dreiecke um
        let m = glam::Mat3::from_cols(map(Vec3::X), map(Vec3::Y), map(Vec3::Z));
        if m.determinant() < 0.0 {
            mesh.indices.chunks_exact_mut(3).for_each(|t| t.swap(1, 2));
        }
    }

    let bounds = |m: &AvatarMesh| {
        let (mut min, mut max) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for p in &m.positions {
            min = min.min(*p);
            max = max.max(*p);
        }
        (min, max)
    };
    let (min, max) = bounds(mesh);

    let mut scale = opts.percent.max(0.01) / 100.0;
    if opts.auto_unit {
        // Ein Mensch ist 0,5 bis 3 m groß. Alles über 20 „Meter“ ist fast sicher in Zentimetern gespeichert.
        if max.y - min.y > 20.0 {
            scale *= 0.01;
        }
    } else {
        scale *= opts.unit.meters();
    }
    if (scale - 1.0).abs() > 1e-6 {
        mesh.positions.iter_mut().for_each(|p| *p *= scale);
    }

    let (mut min, mut max) = bounds(mesh);
    if opts.ground {
        let lift = -min.y;
        if lift.abs() > 1e-4 {
            mesh.positions.iter_mut().for_each(|p| p.y += lift);
            min.y += lift;
            max.y += lift;
        }
    }

    mesh.min = min;
    mesh.max = max;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn datei(n: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/dateien").join(n)
    }

    #[test]
    fn automatisch_steht_die_figur() {
        for f in ["figur_zup_cm.fbx", "figur.dae", "figur.usda", "figur_cm.obj", "figur.glb"] {
            let m = load(&datei(f), &ImportOptions::default()).unwrap();
            assert!((m.height() - 1.75).abs() < 1e-3, "{f}: {}", m.height());
        }
    }

    #[test]
    fn von_hand_zentimeter_und_z_oben() {
        // Rohzahlen der FBX: Zentimeter, Z oben. Ziel-Y = Datei-Z, Ziel-Z = −Datei-Y
        let o = ImportOptions {
            auto_unit: false,
            unit: Unit::Centimeter,
            auto_axes: false,
            axes: [(0, false), (2, false), (1, true)],
            ..Default::default()
        };
        let m = load(&datei("figur_zup_cm.fbx"), &o).unwrap();
        assert!((m.height() - 1.75).abs() < 1e-3, "{}", m.height());
        assert!((m.max.x - m.min.x - 0.76).abs() < 1e-3);
    }

    #[test]
    fn falsche_einheit_wirkt_wirklich() {
        // Meter statt Zentimeter von Hand: dann ist die Figur 175 m groß – die Einstellung greift
        let o = ImportOptions { auto_unit: false, unit: Unit::Meter, ..Default::default() };
        let m = load(&datei("figur_cm.obj"), &o).unwrap();
        assert!((m.height() - 175.0).abs() < 1e-2, "{}", m.height());
    }

    #[test]
    fn prozent_und_ohne_boden() {
        let o = ImportOptions { percent: 50.0, ground: false, ..Default::default() };
        let m = load(&datei("figur.glb"), &o).unwrap();
        assert!((m.height() - 0.875).abs() < 1e-3);
        assert!(m.min.y.abs() < 1e-4); // Figur steht in der Datei schon auf 0
    }

    #[test]
    fn doppelte_achse_wird_abgelehnt() {
        let o = ImportOptions { auto_axes: false, axes: [(0, false), (0, false), (2, false)], ..Default::default() };
        assert!(load(&datei("figur.glb"), &o).is_err());
    }
}

