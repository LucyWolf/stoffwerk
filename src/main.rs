// Unter Windows kein Konsolenfenster neben dem Programm
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod avatar;
mod camera;
mod pattern;
mod viewport;

use eframe::egui;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> eframe::Result {
    // Für den Release-Workflow: prüft, dass das fertige Programm überhaupt startet
    if std::env::args().nth(1).as_deref() == Some("--version") {
        println!("Stoffwerk {VERSION}");
        return Ok(());
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
