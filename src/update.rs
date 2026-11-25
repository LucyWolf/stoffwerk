//! Nachsehen, ob auf GitHub eine neuere Fassung liegt – und sie einspielen.
//! Nach dem Vorbild von Notizmappe und DL24 Akkutest:
//!
//!   Windows: stoffwerk-setup.exe holen, gegen die .sha256 am Release prüfen, still installieren
//!            (das Setup beendet das laufende Programm) und neu starten.
//!   Linux:   stoffwerk-linux holen, gegen die Prüfsumme von GitHub prüfen, die laufende Datei
//!            ersetzen und neu starten.
//!
//! Ein selbst gebautes Programm (aus target/) aktualisiert sich nicht – das geht mit git.

use anyhow::{Context, Result, anyhow, bail};
use sha2::{Digest, Sha256};
use std::time::Duration;

const API: &str = "https://api.github.com/repos/LucyWolf/stoffwerk/releases/latest";
const MAX_FILE: u64 = 300 * 1024 * 1024;
const LINUX_FILE: &str = "stoffwerk-linux";
const SETUP: &str = "stoffwerk-setup.exe";

/// Pfad der eigenen Programmdatei, festgehalten beim Start. Nach dem Austausch zeigt
/// `current_exe()` unter Linux auf die gelöschte alte Datei („… (deleted)“) – der Neustart ginge ins Leere.
static EXE: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();

/// Gleich beim Programmstart aufrufen
pub fn remember_exe() {
    if let Ok(p) = std::env::current_exe().and_then(|p| p.canonicalize()) {
        let _ = EXE.set(p);
    }
}

fn exe() -> Result<std::path::PathBuf> {
    match EXE.get() {
        Some(p) => Ok(p.clone()),
        None => Ok(std::env::current_exe()?.canonicalize()?),
    }
}

#[derive(Clone, Debug, Default)]
pub struct Status {
    pub installed: String,
    pub available: Option<String>,
    /// Neuere Version, deren Datei schon am Release hängt
    pub newer: bool,
    /// Neuere Version, deren Datei noch gebaut wird (Release entsteht in zwei Schritten)
    pub pending: Option<String>,
    pub error: Option<String>,
    pub updatable: bool,
}

/// „v0.1.10“ → (0, 1, 10): stellenweise als Zahl, sonst wäre 0.1.9 größer als 0.1.10
pub fn version_tuple(raw: &str) -> Vec<u64> {
    let v: Vec<u64> = raw
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .take(4)
        .filter_map(|s| s.parse().ok())
        .collect();
    if v.is_empty() { vec![0] } else { v }
}

/// Selbst gebaut (cargo) oder ausdrücklich abgeschaltet: nicht selbst austauschen
pub fn updatable() -> bool {
    if std::env::var_os("STOFFWERK_KEINE_UPDATES").is_some() {
        return false;
    }
    exe().map(|p| !p.components().any(|c| c.as_os_str() == "target")).unwrap_or(false)
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(60)))
        .http_status_as_error(false)
        .build()
        .into()
}

fn get(url: &str, raw: bool, limit: u64) -> Result<Vec<u8>> {
    let accept = if raw { "application/octet-stream" } else { "application/vnd.github+json" };
    let mut resp = agent()
        .get(url)
        .header("Accept", accept)
        .header("User-Agent", &format!("Stoffwerk/{}", crate::VERSION))
        .call()
        .map_err(|e| anyhow!("GitHub nicht erreichbar: {e}"))?;
    let code = resp.status().as_u16();
    if code == 403 || code == 429 {
        bail!("GitHub lässt gerade keine Anfragen zu ({code}) – später noch einmal versuchen");
    }
    if code >= 400 {
        bail!("GitHub antwortet mit {code}");
    }
    resp.body_mut()
        .with_config()
        .limit(limit)
        .read_to_vec()
        .map_err(|e| anyhow!("Download abgebrochen: {e}"))
}

struct Release {
    version: String,
    assets: serde_json::Map<String, serde_json::Value>,
}

fn latest() -> Result<Release> {
    let json: serde_json::Value = serde_json::from_slice(&get(API, false, 4 * 1024 * 1024)?)?;
    let version = json["tag_name"].as_str().unwrap_or("").trim_start_matches('v').to_string();
    let mut assets = serde_json::Map::new();
    for a in json["assets"].as_array().into_iter().flatten() {
        if let Some(name) = a["name"].as_str() {
            assets.insert(name.to_string(), a.clone());
        }
    }
    Ok(Release { version, assets })
}

fn needed_file() -> &'static str {
    if cfg!(windows) { SETUP } else { LINUX_FILE }
}

pub fn check() -> Status {
    let mut s = Status { installed: crate::VERSION.to_string(), updatable: updatable(), ..Default::default() };
    match latest() {
        Ok(r) => {
            s.available = (!r.version.is_empty()).then(|| r.version.clone());
            let newer = version_tuple(&r.version) > version_tuple(crate::VERSION);
            // Das Windows-Setup hängt erst ein paar Minuten nach dem Release dran
            if newer && !r.assets.contains_key(needed_file()) {
                s.pending = Some(r.version);
            } else {
                s.newer = newer;
            }
        }
        Err(e) => s.error = Some(format!("{e:#}")),
    }
    s
}

/// Prüfsumme: aus der .sha256-Datei am Release, sonst die, die GitHub selbst nennt
fn expected_sha(assets: &serde_json::Map<String, serde_json::Value>, name: &str) -> Option<String> {
    if let Some(url) = assets.get(&format!("{name}.sha256")).and_then(|a| a["browser_download_url"].as_str()) {
        if let Ok(text) = get(url, true, 4096) {
            let text = String::from_utf8_lossy(&text);
            if let Some(hex) = text.split_whitespace().find(|w| w.len() == 64 && w.chars().all(|c| c.is_ascii_hexdigit())) {
                return Some(hex.to_ascii_lowercase());
            }
        }
    }
    assets.get(name)?["digest"].as_str()?.strip_prefix("sha256:").map(|h| h.to_ascii_lowercase())
}

/// Neue Fassung holen, prüfen und einspielen. Danach `restart()` aufrufen.
pub fn install() -> Result<String> {
    if !updatable() {
        bail!("Selbst gebautes Programm – hier wird mit git aktualisiert.");
    }
    let r = latest()?;
    if version_tuple(&r.version) <= version_tuple(crate::VERSION) {
        bail!("Nichts Neueres da (installiert {}, im Release {})", crate::VERSION, r.version);
    }
    let name = needed_file();
    let url = r
        .assets
        .get(name)
        .and_then(|a| a["browser_download_url"].as_str())
        .ok_or_else(|| anyhow!("Version {} wird gerade noch bereitgestellt – in ein paar Minuten noch einmal versuchen.", r.version))?
        .to_string();
    let want = expected_sha(&r.assets, name).ok_or_else(|| anyhow!("Für die Datei gibt es keine Prüfsumme – Update abgebrochen"))?;
    let data = get(&url, true, MAX_FILE)?;
    let got: String = Sha256::digest(&data).iter().map(|b| format!("{b:02x}")).collect();
    if got != want {
        bail!("Prüfsumme passt nicht (erwartet {}…, geladen {}…) – Update abgebrochen", &want[..12], &got[..12]);
    }
    log::info!("Update {} -> {}, Prüfsumme {}", crate::VERSION, r.version, &got[..12]);
    place(&data)?;
    Ok(r.version)
}

#[cfg(not(windows))]
fn place(data: &[u8]) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    // Die laufende Datei ersetzen: unter Linux behält der laufende Prozess die alte bis zum Neustart.
    // Erst daneben schreiben, dann in einem Schritt tauschen.
    let now = exe()?;
    let new = now.with_file_name(format!("{}.neu", now.file_name().unwrap_or_default().to_string_lossy()));
    let result = (|| -> Result<()> {
        std::fs::write(&new, data)?;
        std::fs::set_permissions(&new, std::fs::Permissions::from_mode(0o755))?;
        std::fs::rename(&new, &now)?;
        Ok(())
    })();
    if let Err(e) = result {
        let _ = std::fs::remove_file(&new);
        return Err(e).with_context(|| format!("Programmdatei ließ sich nicht ersetzen ({})", now.display()));
    }
    Ok(())
}

#[cfg(windows)]
fn place(data: &[u8]) -> Result<()> {
    use std::os::windows::process::CommandExt;
    let dir: std::path::PathBuf = std::env::temp_dir().join(format!("stoffwerk-update-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let setup = dir.join(SETUP);
    std::fs::write(&setup, data)?;
    let log = dir.join("update.log");
    let exe = exe()?;
    // Stapeldatei: das Setup beendet uns, installiert still, danach startet die neue Fassung
    let cmd = dir.join("update.cmd");
    std::fs::write(
        &cmd,
        format!(
            "@echo off\r\n\"{}\" /SILENT /SUPPRESSMSGBOXES /NORESTART /LOG=\"{}\"\r\ntimeout /t 3 /nobreak >nul\r\nstart \"\" \"{}\"\r\n",
            setup.display(),
            log.display(),
            exe.display()
        ),
    )?;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const DETACHED: u32 = 0x0000_0008;
    std::process::Command::new("cmd")
        .arg("/c")
        .arg(&cmd)
        .current_dir(&dir)
        .creation_flags(CREATE_NO_WINDOW | DETACHED)
        .spawn()?;
    Ok(())
}

/// Nach dem Einspielen: Linux startet die neue Datei an Stelle dieses Prozesses,
/// Windows beendet sich und überlässt den Rest dem Setup
pub fn restart() -> ! {
    #[cfg(not(windows))]
    {
        use std::os::unix::process::CommandExt;
        if let Ok(exe) = exe() {
            let err = std::process::Command::new(exe).args(std::env::args_os().skip(1)).exec();
            log::error!("Neustart fehlgeschlagen: {err}");
        }
    }
    std::process::exit(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versionen_stellenweise() {
        assert!(version_tuple("v0.1.10") > version_tuple("0.1.9"));
        assert!(version_tuple("0.2.0") > version_tuple("0.1.99"));
        assert_eq!(version_tuple("v1.2.3"), vec![1, 2, 3]);
        assert_eq!(version_tuple("kaputt"), vec![0]);
    }
}
