use crate::camera::OrbitCamera;
use crate::import::{self, AvatarMesh};
use crate::pattern::PatternView;
use crate::viewport::Viewport3D;
use eframe::egui::{self, Color32, RichText, Sense};
use eframe::egui_wgpu;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};

struct Loading {
    name: String,
    rx: Receiver<anyhow::Result<AvatarMesh>>,
}

/// Was in der Objektliste ausgewählt ist; die Eigenschaften rechts zeigen dazu die Werte
#[derive(Clone, Copy, PartialEq)]
enum Selection {
    None,
    Avatar,
}

pub struct StoffwerkApp {
    rs: egui_wgpu::RenderState,
    viewport: Viewport3D,
    camera: OrbitCamera,
    pattern: PatternView,
    avatar: Option<AvatarMesh>,
    avatar_visible: bool,
    selection: Selection,
    loading: Option<Loading>,
    error: Option<String>,
    info: Option<String>,
    show_about: bool,
    /// Größe der 3D-Ansicht in Bildpunkten, für das Bildschirmfoto
    view_px: [u32; 2],
    screenshot_requested: bool,
}

const PANEL_BG: Color32 = Color32::from_rgb(32, 34, 38);
const HEADER_BG: Color32 = Color32::from_rgb(40, 43, 48);

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
            avatar_visible: true,
            selection: Selection::None,
            loading: None,
            error: None,
            info: None,
            show_about: false,
            view_px: [1280, 720],
            screenshot_requested: false,
        };

        // Avatar direkt beim Start öffnen: stoffwerk <datei>
        if let Some(arg) = std::env::args_os().nth(1) {
            app.start_loading(&cc.egui_ctx, Path::new(&arg));
        }
        Ok(app)
    }

    /// Lädt im Hintergrund: große Avatare mit vielen Texturen brauchen Sekunden, das Fenster soll dabei nicht hängen.
    fn start_loading(&mut self, ctx: &egui::Context, path: &Path) {
        let (tx, rx) = channel();
        let p = path.to_path_buf();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let r = std::panic::catch_unwind(|| import::load(&p))
                .unwrap_or_else(|_| Err(anyhow::anyhow!("Beim Lesen der Datei ist ein interner Fehler aufgetreten.")));
            let _ = tx.send(r);
            ctx.request_repaint();
        });
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        self.loading = Some(Loading { name, rx });
        self.error = None;
    }

    fn poll_loading(&mut self) {
        let Some(l) = &self.loading else { return };
        let Ok(result) = l.rx.try_recv() else { return };
        self.loading = None;
        match result {
            Ok(mesh) => {
                self.camera.frame_bounds(mesh.min, mesh.max);
                self.viewport.set_avatar(&self.rs, Some(&mesh));
                self.avatar = Some(mesh);
                self.avatar_visible = true;
                self.selection = Selection::Avatar;
            }
            Err(e) => self.error = Some(format!("{e:#}")),
        }
    }

    fn remove_avatar(&mut self) {
        self.avatar = None;
        self.viewport.set_avatar(&self.rs, None);
        if self.selection == Selection::Avatar {
            self.selection = Selection::None;
        }
    }

    fn open_dialog(&mut self, ctx: &egui::Context) {
        let picked = rfd::FileDialog::new()
            .set_title("Avatar laden")
            .add_filter("Avatar (FBX, VRM, glTF, OBJ, DAE, USD, Alembic)", import::EXTENSIONS)
            .add_filter("Alle Dateien", &["*"])
            .pick_file();
        if let Some(path) = picked {
            self.start_loading(ctx, &path);
        }
    }

    fn save_screenshot(&mut self) {
        // Doppelte Auflösung der Ansicht, damit das Bild auch vergrößert scharf ist
        let size = [self.view_px[0] * 2, self.view_px[1] * 2];
        let img = self.viewport.screenshot(&self.rs, size, &self.camera, self.avatar_visible);
        let default = match &self.avatar {
            Some(a) => format!("{}.png", a.name.rsplit_once('.').map(|(s, _)| s).unwrap_or(&a.name)),
            None => "stoffwerk.png".into(),
        };
        let Some(mut path) = rfd::FileDialog::new()
            .set_title("Bildschirmfoto speichern")
            .add_filter("PNG-Bild", &["png"])
            .set_file_name(default)
            .save_file()
        else {
            return;
        };
        if path.extension().is_none() {
            path.set_extension("png");
        }
        match img.save(&path) {
            Ok(()) => self.info = Some(format!("Gespeichert: {}", path.display())),
            Err(e) => self.error = Some(format!("Bild konnte nicht gespeichert werden: {e}")),
        }
    }

    fn menu(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("Datei", |ui| {
                if ui.button("Avatar laden …").clicked() {
                    self.open_dialog(ui.ctx());
                }
                if ui.add_enabled(self.avatar.is_some(), egui::Button::new("Avatar entfernen")).clicked() {
                    self.remove_avatar();
                }
                ui.separator();
                if ui.button("Bildschirmfoto speichern …").clicked() {
                    self.screenshot_requested = true;
                }
                ui.separator();
                if ui.button("Beenden").clicked() {
                    ui.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.menu_button("Ansicht", |ui| {
                if ui.button("Kamera auf Avatar").clicked() {
                    self.frame_avatar();
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

    fn frame_avatar(&mut self) {
        match &self.avatar {
            Some(a) => self.camera.frame_bounds(a.min, a.max),
            None => self.camera = OrbitCamera::default(),
        }
    }

    fn status(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if let Some(l) = &self.loading {
                ui.spinner();
                ui.label(format!("Lade {} …", l.name));
            } else if let Some(a) = &self.avatar {
                ui.label(format!("{}  ·  {} Dreiecke  ·  {:.2} m groß", a.name, a.triangle_count(), a.height()));
            } else {
                ui.label(RichText::new("Kein Avatar geladen – Datei > Avatar laden oder Datei ins Fenster ziehen").weak());
            }
            if let Some(info) = &self.info {
                ui.separator();
                ui.label(RichText::new(info).weak());
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(format!("v{}", crate::VERSION)).weak());
                ui.separator();
                ui.label(RichText::new(self.rs.adapter.get_info().name).weak());
            });
        });
    }

    /// Kopfleiste über einer Ansicht: Titel links, Werkzeuge daneben
    fn header(ui: &mut egui::Ui, title: &str, tools: impl FnOnce(&mut egui::Ui)) {
        egui::Frame::NONE
            .fill(HEADER_BG)
            .inner_margin(egui::Margin::symmetric(8, 4))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(RichText::new(title).strong());
                    ui.separator();
                    tools(ui);
                });
            });
    }

    fn view_3d(&mut self, ui: &mut egui::Ui) {
        let mut preset: Option<(f32, f32)> = None;
        let mut frame = false;
        Self::header(ui, "3D", |ui| {
            for (label, yaw, pitch) in [
                ("Vorne", 0.0, 0.1),
                ("Hinten", std::f32::consts::PI, 0.1),
                ("Links", -std::f32::consts::FRAC_PI_2, 0.1),
                ("Rechts", std::f32::consts::FRAC_PI_2, 0.1),
                ("Oben", 0.0, 1.5),
            ] {
                if ui.small_button(label).clicked() {
                    preset = Some((yaw, pitch));
                }
            }
            ui.separator();
            if ui.small_button("Auf Avatar").clicked() {
                frame = true;
            }
        });
        if frame {
            self.frame_avatar();
        }
        if let Some((yaw, pitch)) = preset {
            self.camera.yaw = yaw;
            self.camera.pitch = pitch;
        }

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
            self.frame_avatar();
        }

        let ppp = ui.ctx().pixels_per_point();
        let size = [(rect.width() * ppp).round() as u32, (rect.height() * ppp).round() as u32];
        if size[0] > 0 && size[1] > 0 {
            self.view_px = size;
            let tex = self.viewport.render(&self.rs, size, &self.camera, self.avatar_visible);
            ui.painter().image(
                tex,
                rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }

        let painter = ui.painter_at(rect);
        if let Some(l) = &self.loading {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                format!("Lade {} …", l.name),
                egui::FontId::proportional(18.0),
                Color32::from_gray(200),
            );
        }
        painter.text(
            rect.left_bottom() + egui::vec2(8.0, -6.0),
            egui::Align2::LEFT_BOTTOM,
            "Links ziehen: drehen  ·  Rechts/Mitte: verschieben  ·  Mausrad: zoomen  ·  Doppelklick: auf Avatar",
            egui::FontId::proportional(11.0),
            Color32::from_gray(115),
        );
    }

    fn view_2d(&mut self, ui: &mut egui::Ui) {
        let mut reset = false;
        Self::header(ui, "2D-Schnitt", |ui| {
            if ui.small_button("Ansicht zurücksetzen").clicked() {
                reset = true;
            }
        });
        if reset {
            self.pattern.reset();
        }
        self.pattern.ui(ui);
    }

    /// Rechte Leiste: oben die Objektliste, darunter die Eigenschaften des ausgewählten Objekts
    fn side_panel(&mut self, ui: &mut egui::Ui) {
        Self::header(ui, "Objekte", |_| {});
        egui::Frame::NONE.inner_margin(egui::Margin::same(8)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new("Avatar").weak().small());
            match &self.avatar {
                Some(a) => {
                    let name = a.name.clone();
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.avatar_visible, "").on_hover_text("Anzeigen / ausblenden");
                        if ui.selectable_label(self.selection == Selection::Avatar, name).clicked() {
                            self.selection = Selection::Avatar;
                        }
                    });
                }
                None => {
                    if ui.button("Avatar laden …").clicked() {
                        self.open_dialog(ui.ctx());
                    }
                }
            }
            ui.add_space(8.0);
            ui.label(RichText::new("Schnittteile").weak().small());
            ui.label(RichText::new("noch keine").weak());
        });

        ui.add_space(6.0);
        Self::header(ui, "Eigenschaften", |_| {});
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Frame::NONE.inner_margin(egui::Margin::same(8)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                match (self.selection, &self.avatar) {
                    (Selection::Avatar, Some(a)) => {
                        egui::Grid::new("avatar-props").num_columns(2).spacing([12.0, 4.0]).show(ui, |ui| {
                            ui.label("Datei");
                            ui.label(&a.name);
                            ui.end_row();
                            ui.label("Größe");
                            ui.label(format!("{:.2} m", a.height()));
                            ui.end_row();
                            ui.label("Breite");
                            ui.label(format!("{:.2} m", a.max.x - a.min.x));
                            ui.end_row();
                            ui.label("Dreiecke");
                            ui.label(a.triangle_count().to_string());
                            ui.end_row();
                            ui.label("Materialteile");
                            ui.label(a.parts.len().to_string());
                            ui.end_row();
                            ui.label("Texturen");
                            ui.label(a.textures.len().to_string());
                            ui.end_row();
                        });
                        if !a.warnings.is_empty() {
                            ui.add_space(8.0);
                            ui.label(RichText::new("Hinweise").color(Color32::from_rgb(230, 170, 80)));
                            for w in &a.warnings {
                                ui.label(RichText::new(w).small());
                            }
                        }
                        ui.add_space(8.0);
                        if ui.button("Avatar entfernen").clicked() {
                            self.remove_avatar();
                        }
                    }
                    _ => {
                        ui.label(RichText::new("Nichts ausgewählt").weak());
                    }
                }
            });
        });
    }
}

impl eframe::App for StoffwerkApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_loading();

        let dropped: Option<PathBuf> = ui.input(|i| i.raw.dropped_files.iter().next().map(|f| f.path().to_path_buf()));
        if let Some(path) = dropped {
            self.start_loading(ui.ctx(), &path);
        }

        egui::Panel::top("menu").show(ui, |ui| self.menu(ui));
        egui::Panel::bottom("status").show(ui, |ui| self.status(ui));

        // Aufteilung wie gewohnt: 3D links, 2D-Schnitt in der Mitte, Objekte und Eigenschaften rechts
        let width = ui.available_width();
        egui::Panel::right("seite")
            .resizable(true)
            .default_size(260.0)
            .size_range(200.0..=420.0)
            .frame(egui::Frame::NONE.fill(PANEL_BG))
            .show(ui, |ui| self.side_panel(ui));
        egui::Panel::left("3d")
            .resizable(true)
            .default_size((width - 260.0) * 0.45)
            .size_range(250.0..=(width - 260.0 - 250.0).max(260.0))
            .frame(egui::Frame::NONE)
            .show(ui, |ui| self.view_3d(ui));
        egui::CentralPanel::no_frame().show(ui, |ui| self.view_2d(ui));

        // Erst nach dem Zeichnen: das Menü ist dann zu, der Dialog blockiert nicht mitten im Aufbau
        if std::mem::take(&mut self.screenshot_requested) {
            self.save_screenshot();
        }

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

        if self.loading.is_some() {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
}
