//! Texturen laden: PNG, JPG, TGA, TIF, BMP, GIF, EXR, WebP über `image`, PSD über `psd`.
//! Fertig verkleinert und mit Mip-Stufen, damit die GPU sie nur noch hochladen muss.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Größere Texturen werden für die Anzeige verkleinert; 2048² mit Mips sind ~22 MB je Textur.
const MAX_SIZE: u32 = 2048;

pub struct TextureImage {
    pub width: u32,
    pub height: u32,
    /// RGBA8, Stufe 0 = volle Größe
    pub mips: Vec<Vec<u8>>,
    /// Ob irgendein Pixel durchsichtig ist (für Haare, Wimpern …)
    pub has_alpha: bool,
}

/// Merkt sich geladene Texturen, damit jede Datei nur einmal gelesen wird.
pub struct TextureCache {
    /// Ordner der Avatar-Datei: dort und darunter wird nach Texturen gesucht
    base: PathBuf,
    by_key: HashMap<String, Option<usize>>,
    pub textures: Vec<TextureImage>,
    pub missing: Vec<String>,
    file_index: Option<Vec<PathBuf>>,
}

impl TextureCache {
    pub fn new(model_path: &Path) -> Self {
        Self {
            base: model_path.parent().map(Path::to_path_buf).unwrap_or_default(),
            by_key: HashMap::new(),
            textures: Vec::new(),
            missing: Vec::new(),
            file_index: None,
        }
    }

    /// Textur aus Bytes (eingebettet in FBX/GLB). `name` dient der Dateiendung (PSD) und als Schlüssel.
    pub fn from_bytes(&mut self, key: &str, name: &str, bytes: &[u8]) -> Option<usize> {
        if let Some(hit) = self.by_key.get(key) {
            return *hit;
        }
        let result = decode(name, bytes).map(|img| self.push(img));
        if result.is_none() {
            self.missing.push(format!("{name} (nicht lesbar)"));
        }
        self.by_key.insert(key.to_string(), result);
        result
    }

    /// Textur über einen Dateinamen, wie er in der Modelldatei steht. Der Pfad stammt oft vom
    /// Rechner der Person, die den Avatar gebaut hat – deshalb wird auch nach dem bloßen Namen gesucht.
    pub fn from_file(&mut self, names: &[&str]) -> Option<usize> {
        let names: Vec<&str> = names.iter().copied().filter(|n| !n.is_empty()).collect();
        let key = names.first()?.to_string();
        if let Some(hit) = self.by_key.get(&key) {
            return *hit;
        }
        let result = match self.resolve(&names) {
            None => {
                self.missing.push(file_name(&key).to_string());
                None
            }
            Some(p) => {
                let decoded = std::fs::read(&p).ok().and_then(|b| decode(&p.to_string_lossy(), &b));
                match decoded {
                    Some(img) => Some(self.push(img)),
                    None => {
                        self.missing.push(format!("{} (nicht lesbar)", file_name(&key)));
                        None
                    }
                }
            }
        };
        self.by_key.insert(key, result);
        result
    }

    fn push(&mut self, img: TextureImage) -> usize {
        self.textures.push(img);
        self.textures.len() - 1
    }

    fn resolve(&mut self, names: &[&str]) -> Option<PathBuf> {
        for n in names {
            let n = n.replace('\\', "/");
            let p = Path::new(&n);
            if p.is_absolute() && p.is_file() {
                return Some(p.to_path_buf());
            }
            let rel = self.base.join(&n);
            if rel.is_file() {
                return Some(rel);
            }
        }
        // Nach dem bloßen Dateinamen suchen (ohne Groß-/Kleinschreibung), im Avatar-Ordner und darüber
        let wanted: Vec<String> = names
            .iter()
            .map(|n| file_name(&n.replace('\\', "/")).to_lowercase())
            .filter(|n| !n.is_empty())
            .collect();
        let index = self.file_index.get_or_insert_with(|| {
            let mut files = Vec::new();
            let roots = [Some(self.base.clone()), self.base.parent().map(Path::to_path_buf)];
            for root in roots.into_iter().flatten() {
                collect_files(&root, 4, &mut files);
            }
            files
        });
        // Gleicher Name; bei anderer Endung (Unity macht aus .psd oft .png) als zweite Wahl
        let stem = |s: &str| s.rsplit_once('.').map(|(a, _)| a.to_string()).unwrap_or(s.to_string());
        index
            .iter()
            .find(|f| wanted.iter().any(|w| f.file_name().is_some_and(|n| n.to_string_lossy().to_lowercase() == *w)))
            .or_else(|| {
                index.iter().find(|f| {
                    let n = f.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
                    is_image(&n) && wanted.iter().any(|w| stem(&n) == stem(w))
                })
            })
            .cloned()
    }
}

fn file_name(p: &str) -> &str {
    p.rsplit(['/', '\\']).next().unwrap_or(p)
}

fn is_image(name: &str) -> bool {
    const EXT: &[&str] = &[".png", ".jpg", ".jpeg", ".tga", ".tif", ".tiff", ".bmp", ".gif", ".exr", ".psd", ".webp", ".hdr"];
    EXT.iter().any(|e| name.ends_with(e))
}

fn collect_files(dir: &Path, depth: u32, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            if depth > 0 && out.len() < 20_000 {
                collect_files(&p, depth - 1, out);
            }
        } else {
            out.push(p);
        }
    }
}

/// Bilddaten → RGBA8 mit Mip-Stufen
pub fn decode(name: &str, bytes: &[u8]) -> Option<TextureImage> {
    let rgba = if name.to_lowercase().ends_with(".psd") {
        let psd = psd::Psd::from_bytes(bytes).ok()?;
        image::RgbaImage::from_raw(psd.width(), psd.height(), psd.rgba())?
    } else {
        image::load_from_memory(bytes).ok()?.to_rgba8()
    };
    Some(from_rgba(rgba))
}

pub fn from_rgba(mut img: image::RgbaImage) -> TextureImage {
    use image::imageops::{FilterType, resize};
    if img.width() > MAX_SIZE || img.height() > MAX_SIZE {
        let s = MAX_SIZE as f32 / img.width().max(img.height()) as f32;
        let (w, h) = (((img.width() as f32 * s) as u32).max(1), ((img.height() as f32 * s) as u32).max(1));
        img = resize(&img, w, h, FilterType::Triangle);
    }
    let has_alpha = img.pixels().any(|p| p.0[3] < 250);
    let (width, height) = img.dimensions();
    let mut mips = vec![img.as_raw().clone()];
    let mut cur = img;
    while cur.width() > 1 || cur.height() > 1 {
        let (w, h) = ((cur.width() / 2).max(1), (cur.height() / 2).max(1));
        cur = resize(&cur, w, h, FilterType::Triangle);
        mips.push(cur.as_raw().clone());
    }
    TextureImage { width, height, mips, has_alpha }
}
