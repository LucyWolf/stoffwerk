// Unter Windows kein Konsolenfenster neben dem Programm
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod import;
mod camera;
mod schnitt;
mod viewport;

use eframe::egui;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> eframe::Result {
    // Für den Release-Workflow: prüft, dass das fertige Programm überhaupt startet
    if std::env::args().nth(1).as_deref() == Some("--version") {
        println!("Stoffwerk {VERSION}");
        return Ok(());
    }
    // Avatar-Datei ohne Fenster laden und berichten (für Tests und zur Fehlersuche bei fremden Dateien)
    if std::env::args().nth(1).as_deref() == Some("--pruefen") {
        let mut ok = true;
        for f in std::env::args().skip(2) {
            match import::load(std::path::Path::new(&f)) {
                Ok(m) => {
                    println!(
                        "OK  {f}: {} Dreiecke, {} Teile, {} Texturen, {} ausgestanzt, Höhe {:.3} m, x {:.3}..{:.3}, z {:.3}..{:.3}",
                        m.triangle_count(),
                        m.parts.len(),
                        m.textures.len(),
                        m.parts.iter().filter(|p| p.cutout).count(),
                        m.height(),
                        m.min.x,
                        m.max.x,
                        m.min.z,
                        m.max.z
                    );
                    for (i, p) in m.parts.iter().enumerate() {
                        println!("    Teil {i}: {} Dreiecke, Textur {:?}, ausgestanzt {}", p.count / 3, p.texture, p.cutout);
                    }
                    for w in &m.warnings {
                        println!("    Hinweis: {w}");
                    }
                }
                Err(e) => {
                    ok = false;
                    println!("FEHLER  {f}: {e:#}");
                }
            }
        }
        std::process::exit(if ok { 0 } else { 1 });
    }
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(format!("Stoffwerk {VERSION}"))
            .with_app_id("stoffwerk")
            .with_inner_size([1400.0, 850.0])
            .with_min_inner_size([800.0, 500.0])
            .with_drag_and_drop(true),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };

    eframe::run_native(
        "Stoffwerk",
        options,
        Box::new(|cc| Ok(Box::new(app::StoffwerkApp::new(cc)?))),
    )
}
