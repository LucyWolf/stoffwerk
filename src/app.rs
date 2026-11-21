use crate::avatar::{self, AvatarMesh};
use crate::camera::OrbitCamera;
use crate::pattern::PatternView;
use crate::viewport::Viewport3D;
use eframe::egui::{self, Color32, RichText, Sense};
use eframe::egui_wgpu;
use std::path::{Path, PathBuf};

pub struct StoffwerkApp {
    rs: egui_wgpu::RenderState,
    viewport: Viewport3D,
    camera: OrbitCamera,
    pattern: PatternView,
    avatar: Option<AvatarMesh>,
    avatar_path: Option<PathBuf>,
    error: Option<String>,
    show_about: bool,
}

impl StoffwerkApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let rs = cc
            .wgpu_render_state
            .clone()
            .ok_or("Keine Grafikschnittstelle (wgpu) verfügbar – Grafiktreiber prüfen.")?;
        let viewport = Viewport3D::new(&rs);

        let mut app = Self {
            rs,
            viewport,
            camera: OrbitCamera::default(),
            pattern: PatternView::default(),
            avatar: None,
            avatar_path: None,
            error: None,
            show_about: false,
        };

        // Avatar direkt beim Start öffnen: stoffwerk <datei>
        if let Some(arg) = std::env::args_os().nth(1) {
            app.load_avatar(Path::new(&arg));
        }
        Ok(app)
    }

    fn load_avatar(&mut self, path: &Path) {
        match avatar::load(path) {
            Ok(mesh) => {
                self.camera.frame_bounds(mesh.min, mesh.max);
                self.viewport.set_avatar(&self.rs, Some(&mesh));
                self.avatar = Some(mesh);
                self.avatar_path = Some(path.to_path_buf());
                self.error = None;
            }
            Err(e) => {
                self.error = Some(format!("{e:#}"));
            }
        }
    }

    fn open_dialog(&mut self) {
        let picked = rfd::FileDialog::new()
            .set_title("Avatar laden")
            .add_filter("Avatar (FBX, VRM, glTF, OBJ)", &["fbx", "vrm", "glb", "gltf", "obj"])
            .add_filter("Alle Dateien", &["*"])
            .pick_file();
        if let Some(path) = picked {
            self.load_avatar(&path);
        }
    }

    fn menu(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("Datei", |ui| {
                if ui.button("Avatar laden …").clicked() {
                    self.open_dialog();
                }
                if ui
                    .add_enabled(self.avatar.is_some(), egui::Button::new("Avatar entfernen"))
                    .clicked()
                {
                    self.avatar = None;
                    self.avatar_path = None;
                    self.viewport.set_avatar(&self.rs, None);
                }
                ui.separator();
                if ui.button("Beenden").clicked() {
                    ui.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.menu_button("Ansicht", |ui| {
                if ui.button("Kamera auf Avatar").clicked() {
                    match &self.avatar {
                        Some(a) => self.camera.frame_bounds(a.min, a.max),
                        None => self.camera = OrbitCamera::default(),
                    }
                }
                if ui.button("Schnittansicht zurücksetzen").clicked() {
                    self.pattern.reset();
                }
            });
            ui.menu_button("Hilfe", |ui| {
                if ui.button("Über Stoffwerk").clicked() {
                    self.show_about = true;
                }
            });
        });
    }

    fn status(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            match &self.avatar {
                Some(a) => {
                    ui.label(format!(
                        "{}  ·  {} Dreiecke  ·  {:.2} m groß",
                        a.name,
                        a.triangle_count(),
                        a.height()
                    ));
                }
                None => {
                    ui.label(RichText::new("Kein Avatar geladen – Datei > Avatar laden oder Datei ins Fenster ziehen").weak());
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(format!("v{}", crate::VERSION)).weak());
                ui.separator();
                ui.label(RichText::new(self.rs.adapter.get_info().name).weak());
            });
        });
    }

    fn view_3d(&mut self, ui: &mut egui::Ui) {
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());

        let height = rect.height();
        if response.dragged_by(egui::PointerButton::Primary) {
            let d = response.drag_delta();
            self.camera.orbit(d.x, d.y);
        }
        if response.dragged_by(egui::PointerButton::Middle) || response.dragged_by(egui::PointerButton::Secondary) {
            let d = response.drag_delta();
            self.camera.pan(d.x, d.y, height);
        }
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                self.camera.zoom(scroll);
            }
        }
        if response.double_clicked() {
            if let Some(a) = &self.avatar {
                self.camera.frame_bounds(a.min, a.max);
            }
        }

        let ppp = ui.ctx().pixels_per_point();
        let size = [(rect.width() * ppp).round() as u32, (rect.height() * ppp).round() as u32];
        if size[0] > 0 && size[1] > 0 {
            let tex = self.viewport.render(&self.rs, size, &self.camera);
            ui.painter().image(
                tex,
                rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }

        let painter = ui.painter_at(rect);
        painter.text(
            rect.left_bottom() + egui::vec2(8.0, -6.0),
            egui::Align2::LEFT_BOTTOM,
            "Links ziehen: drehen   ·   Rechts/Mitte ziehen: verschieben   ·   Mausrad: zoomen   ·   Doppelklick: auf Avatar",
            egui::FontId::proportional(12.0),
            Color32::from_gray(120),
        );
    }
}

impl eframe::App for StoffwerkApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Datei ins Fenster gezogen
        let dropped: Option<PathBuf> =
            ui.input(|i| i.raw.dropped_files.iter().next().map(|f| f.path().to_path_buf()));
        if let Some(path) = dropped {
            self.load_avatar(&path);
        }

        egui::Panel::top("menu").show(ui, |ui| self.menu(ui));
        egui::Panel::bottom("status").show(ui, |ui| self.status(ui));

        egui::Panel::left("schnitt")
            .resizable(true)
            .default_size(ui.available_width() * 0.42)
            .size_range(250.0..=ui.available_width() - 250.0)
            .frame(egui::Frame::NONE)
            .show(ui, |ui| self.pattern.ui(ui));

        egui::CentralPanel::no_frame().show(ui, |ui| self.view_3d(ui));

        if let Some(err) = self.error.clone() {
            let mut open = true;
            egui::Window::new("Fehler")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .open(&mut open)
                .show(ui.ctx(), |ui| {
                    ui.label(err);
                    if ui.button("OK").clicked() {
                        self.error = None;
                    }
                });
            if !open {
                self.error = None;
            }
        }

        if self.show_about {
            egui::Window::new("Über Stoffwerk")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .open(&mut self.show_about)
                .show(ui.ctx(), |ui| {
                    ui.heading(format!("Stoffwerk {}", crate::VERSION));
                    ui.label("Kleidung für VRChat-Avatare: Schnittmuster zeichnen, nähen, simulieren, exportieren.");
                    ui.hyperlink("https://github.com/LucyWolf/stoffwerk");
                });
        }
    }
}
