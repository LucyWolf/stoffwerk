use crate::camera::OrbitCamera;
use crate::import::{self, AvatarMesh};
use crate::schnitt::{self, Doc, Editor, editor::Sel};
use crate::viewport::Viewport3D;
use eframe::egui::{self, Color32, RichText, Sense};
use eframe::egui_wgpu;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};

struct Loading {
    name: String,
    path: PathBuf,
    rx: Receiver<anyhow::Result<AvatarMesh>>,
}

/// Was nach der Rückfrage „ungespeicherte Änderungen“ passieren soll
#[derive(Clone)]
enum Pending {
    Close,
    New,
    Open(PathBuf),
}

pub struct StoffwerkApp {
    rs: egui_wgpu::RenderState,
    viewport: Viewport3D,
    camera: OrbitCamera,
    doc: Doc,
    editor: Editor,
    project_path: Option<PathBuf>,
    avatar: Option<AvatarMesh>,
    avatar_path: Option<PathBuf>,
    avatar_visible: bool,
    avatar_selected: bool,
    loading: Option<Loading>,
    error: Option<String>,
    info: Option<String>,
    show_about: bool,
    pending: Option<Pending>,
    allow_close: bool,
    title: String,
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
            doc: Doc::default(),
            editor: Editor::default(),
            project_path: None,
            avatar: None,
            avatar_path: None,
            avatar_visible: true,
            avatar_selected: false,
            loading: None,
            error: None,
            info: None,
            show_about: false,
            pending: None,
            allow_close: false,
            title: String::new(),
            view_px: [1280, 720],
            screenshot_requested: false,
        };

        // Datei direkt beim Start öffnen: stoffwerk <avatar oder projekt>
        if let Some(arg) = std::env::args_os().nth(1) {
            app.open_any(&cc.egui_ctx, Path::new(&arg));
        }
        Ok(app)
    }

    fn open_any(&mut self, ctx: &egui::Context, path: &Path) {
        let is_project = path.extension().is_some_and(|e| e.eq_ignore_ascii_case(schnitt::EXT));
        if is_project {
            self.request(ctx, Pending::Open(path.to_path_buf()));
        } else {
            self.start_loading(ctx, path);
        }
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
        self.loading = Some(Loading { name, path: path.to_path_buf(), rx });
        self.error = None;
    }

    fn poll_loading(&mut self, ctx: &egui::Context) {
        let Some(l) = &self.loading else { return };
        let Ok(result) = l.rx.try_recv() else { return };
        let path = l.path.clone();
        self.loading = None;
        match result {
            Ok(mesh) => {
                self.camera.frame_bounds(mesh.min, mesh.max);
                self.viewport.set_avatar(&self.rs, Some(&mesh));
                self.editor.set_avatar(ctx, Some(&mesh));
                self.avatar = Some(mesh);
                if self.avatar_path.as_ref() != Some(&path) {
                    self.editor.dirty = true;
                }
                self.avatar_path = Some(path);
                self.avatar_visible = true;
            }
            Err(e) => self.error = Some(format!("{e:#}")),
        }
    }

    fn remove_avatar(&mut self, ctx: &egui::Context) {
        self.avatar = None;
        self.avatar_path = None;
        self.avatar_selected = false;
        self.viewport.set_avatar(&self.rs, None);
        self.editor.set_avatar(ctx, None);
        self.editor.dirty = true;
    }

    fn open_avatar_dialog(&mut self, ctx: &egui::Context) {
        let picked = rfd::FileDialog::new()
            .set_title("Avatar laden")
            .add_filter("Avatar (FBX, VRM, glTF, OBJ, DAE, USD, Alembic)", import::EXTENSIONS)
            .add_filter("Alle Dateien", &["*"])
            .pick_file();
        if let Some(path) = picked {
            self.start_loading(ctx, &path);
        }
    }

    // ------------------------------------------------------------ Projekt

    /// Fragt nach, wenn Ungespeichertes verloren ginge
    fn request(&mut self, ctx: &egui::Context, action: Pending) {
        if self.editor.dirty {
            self.pending = Some(action);
        } else {
            self.run(ctx, action);
        }
    }

    fn run(&mut self, ctx: &egui::Context, action: Pending) {
        match action {
            Pending::Close => {
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Pending::New => {
                self.doc = Doc::default();
                self.editor.clear_history();
                self.project_path = None;
                self.remove_avatar(ctx);
                self.editor.dirty = false;
            }
            Pending::Open(path) => match schnitt::load(&path) {
                Ok((doc, avatar)) => {
                    self.doc = doc;
                    self.editor.clear_history();
                    self.project_path = Some(path);
                    match avatar {
                        Some(a) if a.exists() => {
                            self.avatar_path = Some(a.clone());
                            self.start_loading(ctx, &a);
                        }
                        Some(a) => {
                            self.remove_avatar(ctx);
                            self.error = Some(format!("Der Avatar des Projekts wurde nicht gefunden:\n{}", a.display()));
                        }
                        None => self.remove_avatar(ctx),
                    }
                    self.editor.dirty = false;
                }
                Err(e) => self.error = Some(format!("{e:#}")),
            },
        }
    }

    fn open_project_dialog(&mut self, ctx: &egui::Context) {
        if let Some(p) = rfd::FileDialog::new()
            .set_title("Projekt öffnen")
            .add_filter("Stoffwerk-Projekt", &[schnitt::EXT])
            .pick_file()
        {
            self.request(ctx, Pending::Open(p));
        }
    }

    /// Gibt true zurück, wenn gespeichert wurde
    fn save(&mut self, save_as: bool) -> bool {
        let path = match (&self.project_path, save_as) {
            (Some(p), false) => p.clone(),
            _ => {
                let Some(mut p) = rfd::FileDialog::new()
                    .set_title("Projekt speichern")
                    .add_filter("Stoffwerk-Projekt", &[schnitt::EXT])
                    .set_file_name("Projekt.stoffwerk")
                    .save_file()
                else {
                    return false;
                };
                if p.extension().is_none() {
                    p.set_extension(schnitt::EXT);
                }
                p
            }
        };
        match schnitt::save(&path, &self.doc, self.avatar_path.as_deref()) {
            Ok(()) => {
                self.info = Some(format!("Gespeichert: {}", path.display()));
                self.project_path = Some(path);
                self.editor.dirty = false;
                true
            }
            Err(e) => {
                self.error = Some(format!("{e:#}"));
                false
            }
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

    // ------------------------------------------------------------ Oberfläche

    fn menu(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("Datei", |ui| {
                if ui.button("Neues Projekt").clicked() {
                    self.request(&ctx, Pending::New);
                }
                if ui.add(egui::Button::new("Projekt öffnen …").shortcut_text("Strg+O")).clicked() {
                    self.open_project_dialog(&ctx);
                }
                if ui.add(egui::Button::new("Projekt speichern").shortcut_text("Strg+S")).clicked() {
                    self.save(false);
                }
                if ui.button("Projekt speichern unter …").clicked() {
                    self.save(true);
                }
                ui.separator();
                if ui.button("Avatar laden …").clicked() {
                    self.open_avatar_dialog(&ctx);
                }
                if ui.add_enabled(self.avatar.is_some(), egui::Button::new("Avatar entfernen")).clicked() {
                    self.remove_avatar(&ctx);
                }
                ui.separator();
                if ui.button("Bildschirmfoto speichern …").clicked() {
                    self.screenshot_requested = true;
                }
                ui.separator();
                if ui.button("Beenden").clicked() {
                    self.request(&ctx, Pending::Close);
                }
            });
            ui.menu_button("Bearbeiten", |ui| {
                if ui.add_enabled(self.editor.can_undo(), egui::Button::new("Rückgängig").shortcut_text("Strg+Z")).clicked() {
                    self.editor.undo(&mut self.doc);
                }
                if ui.add_enabled(self.editor.can_redo(), egui::Button::new("Wiederholen").shortcut_text("Strg+Y")).clicked() {
                    self.editor.redo(&mut self.doc);
                }
                ui.separator();
                if ui
                    .add_enabled(self.editor.sel != Sel::None, egui::Button::new("Auswahl löschen").shortcut_text("Entf"))
                    .clicked()
                {
                    self.editor.delete_selection(&mut self.doc);
                }
            });
            ui.menu_button("Ansicht", |ui| {
                if ui.button("Kamera auf Avatar").clicked() {
                    self.frame_avatar();
                }
                if ui.button("Schnittansicht zurücksetzen").clicked() {
                    self.editor.reset_views();
                }
                ui.checkbox(&mut self.editor.split, "Schnittansicht teilen");
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
                ui.horizontal_wrapped(|ui| {
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
        let (editor, doc) = (&mut self.editor, &mut self.doc);
        Self::header(ui, "2D-Schnitt", |ui| editor.toolbar(ui, doc));
        self.editor.ui(ui, &mut self.doc);
    }

    /// Rechte Leiste: oben die Objektliste, darunter die Eigenschaften der Auswahl
    fn side_panel(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        Self::header(ui, "Objekte", |_| {});
        egui::ScrollArea::vertical().id_salt("objekte").max_height(ui.available_height() * 0.45).show(ui, |ui| {
            egui::Frame::NONE.inner_margin(egui::Margin::same(8)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new("Avatar").weak().small());
                match &self.avatar {
                    Some(a) => {
                        let name = a.name.clone();
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut self.avatar_visible, "").on_hover_text("Anzeigen / ausblenden");
                            let sel = self.avatar_selected && self.editor.sel == Sel::None;
                            if ui.selectable_label(sel, name).clicked() {
                                self.avatar_selected = true;
                                self.editor.sel = Sel::None;
                            }
                        });
                    }
                    None => {
                        if ui.button("Avatar laden …").clicked() {
                            self.open_avatar_dialog(&ctx);
                        }
                    }
                }
                ui.add_space(8.0);
                self.editor.objects_ui(ui, &self.doc);
            });
        });

        ui.add_space(6.0);
        Self::header(ui, "Eigenschaften", |_| {});
        egui::ScrollArea::vertical().id_salt("eigenschaften").show(ui, |ui| {
            egui::Frame::NONE.inner_margin(egui::Margin::same(8)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                if self.editor.properties_ui(ui, &mut self.doc) {
                    return;
                }
                match (&self.avatar, self.avatar_selected) {
                    (Some(a), true) => {
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
                            self.remove_avatar(&ctx);
                        }
                    }
                    _ => {
                        ui.label(RichText::new("Nichts ausgewählt").weak());
                    }
                }
            });
        });
    }

    fn update_title(&mut self, ctx: &egui::Context) {
        let name = self
            .project_path
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Unbenannt".into());
        let title = format!("{}{} – Stoffwerk {}", name, if self.editor.dirty { " *" } else { "" }, crate::VERSION);
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
    }

    fn unsaved_dialog(&mut self, ctx: &egui::Context) {
        let Some(action) = self.pending.clone() else { return };
        let mut choice = None;
        egui::Modal::new(egui::Id::unique("ungespeichert")).show(ctx, |ui| {
            ui.heading("Ungespeicherte Änderungen");
            ui.label("Der Schnitt wurde seit dem letzten Speichern geändert.");
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Speichern").clicked() {
                    choice = Some(0);
                }
                if ui.button("Nicht speichern").clicked() {
                    choice = Some(1);
                }
                if ui.button("Abbrechen").clicked() {
                    choice = Some(2);
                }
            });
        });
        match choice {
            Some(0) => {
                self.pending = None;
                if self.save(false) {
                    self.run(ctx, action);
                }
            }
            Some(1) => {
                self.pending = None;
                self.editor.dirty = false;
                self.run(ctx, action);
            }
            Some(_) => self.pending = None,
            None => {}
        }
    }
}

impl eframe::App for StoffwerkApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll_loading(&ctx);

        // Schließen abfangen, solange Ungespeichertes da ist
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close && self.editor.dirty {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pending = Some(Pending::Close);
        }

        let dropped: Option<PathBuf> = ui.input(|i| i.raw.dropped_files.iter().next().map(|f| f.path().to_path_buf()));
        if let Some(path) = dropped {
            self.open_any(&ctx, &path);
        }

        let (save, open) = ctx.input(|i| (i.modifiers.command && i.key_pressed(egui::Key::S), i.modifiers.command && i.key_pressed(egui::Key::O)));
        if save {
            self.save(i_shift(&ctx));
        }
        if open {
            self.open_project_dialog(&ctx);
        }

        egui::Panel::top("menu").show(ui, |ui| self.menu(ui));
        egui::Panel::bottom("status").show(ui, |ui| self.status(ui));

        // Aufteilung wie gewohnt: 3D links, 2D-Schnitt in der Mitte, Objekte und Eigenschaften rechts
        let width = ui.available_width();
        egui::Panel::right("seite")
            .resizable(true)
            .default_size(270.0)
            .size_range(200.0..=440.0)
            .frame(egui::Frame::NONE.fill(PANEL_BG))
            .show(ui, |ui| self.side_panel(ui));
        egui::Panel::left("3d")
            .resizable(true)
            .default_size((width - 270.0) * 0.4)
            .size_range(250.0..=(width - 270.0 - 300.0).max(260.0))
            .frame(egui::Frame::NONE)
            .show(ui, |ui| self.view_3d(ui));
        egui::CentralPanel::no_frame().show(ui, |ui| self.view_2d(ui));

        if self.editor.sel != Sel::None {
            self.avatar_selected = false;
        }

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
                .show(&ctx, |ui| {
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
                .show(&ctx, |ui| {
                    ui.heading(format!("Stoffwerk {}", crate::VERSION));
                    ui.label("Kleidung für VRChat-Avatare: Schnittmuster zeichnen, nähen, simulieren, exportieren.");
                    ui.hyperlink("https://github.com/LucyWolf/stoffwerk");
                });
        }

        self.unsaved_dialog(&ctx);
        self.update_title(&ctx);

        if self.loading.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
}

fn i_shift(ctx: &egui::Context) -> bool {
    ctx.input(|i| i.modifiers.shift)
}
