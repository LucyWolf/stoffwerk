//! Schnitt: Datenmodell, Editor und Projektdatei.

pub mod editor;
mod icons;
pub mod model;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub use editor::Editor;
pub use model::Doc;

/// Projektdatei (.stoffwerk): offenes JSON, damit man sie auch ohne Stoffwerk lesen kann
#[derive(Serialize, Deserialize)]
pub struct Project {
    pub format: String,
    pub version: u32,
    /// Pfad zum Avatar; relativ zur Projektdatei, wenn er im selben Ordner oder darunter liegt
    #[serde(default)]
    pub avatar: Option<String>,
    pub schnitt: Doc,
}

pub const EXT: &str = "stoffwerk";

pub fn save(path: &Path, doc: &Doc, avatar: Option<&Path>) -> Result<()> {
    let base = path.parent().unwrap_or(Path::new(""));
    let avatar = avatar.map(|a| a.strip_prefix(base).unwrap_or(a).to_string_lossy().into_owned());
    let p = Project { format: "stoffwerk".into(), version: 1, avatar, schnitt: doc.clone() };
    let json = serde_json::to_string_pretty(&p)?;
    // Erst daneben schreiben, dann umbenennen: ein Absturz beim Speichern zerstört so nie die alte Datei
    let tmp = path.with_extension("stoffwerk.tmp");
    std::fs::write(&tmp, json).with_context(|| format!("Konnte nicht schreiben: {}", tmp.display()))?;
    std::fs::rename(&tmp, path).with_context(|| format!("Konnte nicht speichern: {}", path.display()))?;
    Ok(())
}

pub fn load(path: &Path) -> Result<(Doc, Option<PathBuf>)> {
    let text = std::fs::read_to_string(path).with_context(|| format!("Konnte nicht lesen: {}", path.display()))?;
    let p: Project = serde_json::from_str(&text).context("Keine gültige Stoffwerk-Projektdatei")?;
    if p.format != "stoffwerk" {
        bail!("Keine Stoffwerk-Projektdatei");
    }
    if p.version > 1 {
        bail!("Die Datei stammt aus einer neueren Stoffwerk-Version – bitte Stoffwerk aktualisieren.");
    }
    let base = path.parent().unwrap_or(Path::new(""));
    let avatar = p.avatar.map(|a| {
        let ap = PathBuf::from(&a);
        if ap.is_absolute() { ap } else { base.join(ap) }
    });
    Ok((p.schnitt, avatar))
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec2;

    #[test]
    fn speichern_und_laden() {
        let dir = std::env::temp_dir().join(format!("stoffwerk-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut doc = Doc::default();
        let id = doc.add_piece("Vorne", &[(Vec2::ZERO, false), (Vec2::new(10.0, 0.0), false), (Vec2::new(5.0, 8.0), true)]);
        doc.symmetric_copy(id);
        let file = dir.join("t.stoffwerk");
        let avatar = dir.join("avatare/figur.vrm");
        save(&file, &doc, Some(&avatar)).unwrap();
        let (back, av) = load(&file).unwrap();
        assert_eq!(back.pieces.len(), 2);
        assert_eq!(back.pieces[0].pts, doc.pieces[0].pts);
        assert_eq!(av.unwrap(), avatar);
        // relativ gespeichert, damit der Projektordner verschoben werden kann
        assert!(std::fs::read_to_string(&file).unwrap().contains("\"avatare/figur.vrm\""));
        std::fs::remove_dir_all(dir).ok();
    }
}
