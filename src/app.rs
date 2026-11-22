use crate::camera::OrbitCamera;
use crate::import::{self, AvatarMesh, ImportOptions, Unit};
use crate::schnitt::{self, Doc, Editor, editor::Sel};
use crate::schnitt::editor::Tool;
use crate::schnitt::icons::{self, Icon};
use crate::viewport::{Display, Viewport3D};
use eframe::egui::{self, Color32, RichText, Sense};
use eframe::egui_wgpu;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};

struct Loading {
    name: String,
    path: PathBuf,
    opts: ImportOptions,
    rx: Receiver<anyhow::Result<AvatarMesh>>,
}

/// Fensteraufteilung (Knöpfe rechts in der Statusleiste)
#[derive(Clone, Copy, PartialEq)]
enum Layout {
    Both,
    Only3D,
    Only2D,
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
    /// Einstellungen, mit denen der aktuelle Avatar geladen wurde (kommen ins Projekt)
    avatar_opts: ImportOptions,
    /// Zuletzt gewählte Einstellungen, Vorgabe für das nächste Import-Fenster
    import_opts: ImportOptions,
    /// Offenes Import-Fenster: Datei und Einstellungen
    import_dialog: Option<(PathBuf, ImportOptions)>,
    display: Display,
    layout: Layout,
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
            avatar_opts: ImportOptions::default(),
            import_opts: ImportOptions::default(),
            import_dialog: None,
            display: Display::default(),
            layout: Layout::Both,
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
            self.import_dialog = Some((path.to_path_buf(), self.import_opts));
        }
    }

    /// Lädt im Hintergrund: große Avatare mit vielen Texturen brauchen Sekunden, das Fenster soll dabei nicht hängen.
    fn start_loading(&mut self, ctx: &egui::Context, path: &Path, opts: ImportOptions) {
        let (tx, rx) = channel();
        let p = path.to_path_buf();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let r = std::panic::catch_unwind(|| import::load(&p, &opts))
                .unwrap_or_else(|_| Err(anyhow::anyhow!("Beim Lesen der Datei ist ein interner Fehler aufgetreten.")));
            let _ = tx.send(r);
            ctx.request_repaint();
        });
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        self.loading = Some(Loading { name, path: path.to_path_buf(), opts, rx });
        self.error = None;
    }

    fn poll_loading(&mut self, ctx: &egui::Context) {
        let Some(l) = &self.loading else { return };
        let Ok(result) = l.rx.try_recv() else { return };
        let path = l.path.clone();
        let opts = l.opts;
        self.loading = None;
        match result {
            Ok(mesh) => {
                self.camera.frame_bounds(mesh.min, mesh.max);
                self.viewport.set_avatar(&self.rs, Some(&mesh));
                self.editor.set_avatar(ctx, Some(&mesh));
                self.avatar = Some(mesh);
                if self.avatar_path.as_ref() != Some(&path) || self.avatar_opts != opts {
                    self.editor.dirty = true;
                }
                self.avatar_opts = opts;
                self.avatar_path = Some(path);
                self.display.avatar = true;
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
            self.open_any(ctx, &path);
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
                Ok((doc, avatar, opts)) => {
                    self.doc = doc;
                    self.editor.clear_history();
                    self.project_path = Some(path);
                    match avatar {
                        Some(a) if a.exists() => {
                            self.avatar_path = Some(a.clone());
                            self.avatar_opts = opts;
                            self.start_loading(ctx, &a, opts);
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
        match schnitt::save(&path, &self.doc, self.avatar_path.as_deref(), &self.avatar_opts) {
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
        let img = self.viewport.screenshot(&self.rs, size, &self.camera, self.display);
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
                if ui.button("Avatar importieren …").clicked() {
                    self.open_avatar_dialog(&ctx);
                }
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
            ui.menu_button("2D-Schnitt", |ui| {
                for t in Tool::ALL {
                    if t == Tool::Seam {
                        continue;
                    }
                    if ui.selectable_label(self.editor.tool == t, t.label()).clicked() {
                        self.editor.set_tool(t);
                        ui.close();
                    }
                }
                ui.separator();
                ui.checkbox(&mut self.editor.snap, "Am Raster einrasten");
                ui.checkbox(&mut self.editor.split, "Ansicht teilen");
                if ui.button("Ansicht zurücksetzen").clicked() {
                    self.editor.reset_views();
                }
            });
            ui.menu_button("Nähen", |ui| {
                if ui.selectable_label(self.editor.tool == Tool::Seam, "Naht setzen").clicked() {
                    self.editor.set_tool(Tool::Seam);
                    ui.close();
                }
                if ui.add_enabled(!self.doc.seams.is_empty(), egui::Button::new("Alle Nähte löschen")).clicked() {
                    self.editor.delete_all_seams(&mut self.doc);
                    ui.close();
                }
                ui.checkbox(&mut self.editor.show_seams, "Nähte anzeigen");
            });
            ui.menu_button("Avatar", |ui| {
                if ui.button("Importieren …").clicked() {
                    self.open_avatar_dialog(&ctx);
                }
                let has = self.avatar.is_some();
                if ui.add_enabled(has, egui::Button::new("Neu importieren …")).clicked() {
                    if let Some(p) = self.avatar_path.clone() {
                        self.import_dialog = Some((p, self.avatar_opts));
                    }
                }
                if ui.add_enabled(has, egui::Button::new("Entfernen")).clicked() {
                    self.remove_avatar(&ctx);
                }
                ui.separator();
                ui.checkbox(&mut self.display.avatar, "Anzeigen");
                ui.checkbox(&mut self.display.textures, "Texturen");
                ui.checkbox(&mut self.editor.show_silhouette, "Silhouette im Schnitt");
            });
            ui.menu_button("Anzeige", |ui| {
                ui.label(RichText::new("3D").weak().small());
                ui.checkbox(&mut self.display.floor, "Boden");
                ui.checkbox(&mut self.display.shadow, "Schatten");
                if ui.button("Kamera auf Avatar").clicked() {
                    self.frame_avatar();
                }
                ui.separator();
                ui.label(RichText::new("2D").weak().small());
                ui.checkbox(&mut self.editor.show_names, "Namen der Teile");
                ui.checkbox(&mut self.editor.show_lengths, "Kantenlängen");
            });
            ui.menu_button("Fenster", |ui| {
                for (l, name) in [(Layout::Both, "3D und 2D"), (Layout::Only3D, "Nur 3D"), (Layout::Only2D, "Nur 2D")] {
                    if ui.selectable_label(self.layout == l, name).clicked() {
                        self.layout = l;
                        ui.close();
                    }
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

    fn status(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let project = self
                .project_path
                .as_ref()
                .and_then(|p| p.file_stem())
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Unbenannt".into());
            ui.label(RichText::new(format!("Version: {}", crate::VERSION)).weak().small());
            ui.label(RichText::new(format!("{project}{}", if self.editor.dirty { " *" } else { "" })).weak().small());
            ui.separator();
            if let Some(l) = &self.loading {
                ui.spinner();
                ui.label(RichText::new(format!("Lade {} …", l.name)).small());
            } else if let Some(a) = &self.avatar {
                ui.label(
                    RichText::new(format!("{}  ·  {} Dreiecke  ·  {:.2} m", a.name, a.triangle_count(), a.height())).weak().small(),
                );
            }
            if let Some(info) = &self.info {
                ui.separator();
                ui.label(RichText::new(info).weak().small());
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for (l, icon, tip) in [
                    (Layout::Only2D, Icon::Layout2D, "Nur 2D-Schnitt"),
                    (Layout::Only3D, Icon::Layout3D, "Nur 3D"),
                    (Layout::Both, Icon::LayoutBoth, "3D und 2D nebeneinander"),
                ] {
                    if icons::button(ui, icon, self.layout == l, true, tip).clicked() {
                        self.layout = l;
                    }
                }
                ui.separator();
                ui.label(RichText::new(self.rs.adapter.get_info().name).weak().small());
            });
        });
    }

    /// Kopfleiste über einer Ansicht: Titel links, Werkzeuge daneben
    /// Kopfleiste über einer Ansicht: Titel und zwei Symbolzeilen (Werkzeuge, Anzeige)
    fn header(ui: &mut egui::Ui, title: &str, mut row: impl FnMut(&mut egui::Ui, usize)) {
        egui::Frame::NONE
            .fill(HEADER_BG)
            .inner_margin(egui::Margin::symmetric(6, 3))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(title).strong().small());
                    ui.separator();
                    row(ui, 0);
                });
                ui.horizontal_wrapped(|ui| row(ui, 1));
            });
    }

    fn view_3d(&mut self, ui: &mut egui::Ui) {
        let mut preset: Option<(f32, f32)> = None;
        let mut frame = false;
        let mut shot = false;
        let display = &mut self.display;
        Self::header(ui, "3D", |ui, row| {
            ui.spacing_mut().item_spacing.x = 2.0;
            if row == 0 {
                for (icon, tip, yaw, pitch) in [
                    (Icon::ViewFront, "Ansicht von vorn", 0.0, 0.1),
                    (Icon::ViewBack, "Ansicht von hinten", std::f32::consts::PI, 0.1),
                    (Icon::ViewLeft, "Ansicht von links", -std::f32::consts::FRAC_PI_2, 0.1),
                    (Icon::ViewRight, "Ansicht von rechts", std::f32::consts::FRAC_PI_2, 0.1),
                    (Icon::ViewTop, "Ansicht von oben", 0.0, 1.5),
                ] {
                    if icons::button(ui, icon, false, true, tip).clicked() {
                        preset = Some((yaw, pitch));
                    }
                }
                ui.separator();
                if icons::button(ui, Icon::Frame, false, true, "Kamera auf den Avatar (Doppelklick)").clicked() {
                    frame = true;
                }
            } else {
                for (icon, flag, tip) in [
                    (Icon::Person, &mut display.avatar, "Avatar anzeigen"),
                    (Icon::Textures, &mut display.textures, "Texturen des Avatars"),
                    (Icon::Floor, &mut display.floor, "Boden"),
                    (Icon::Shadow, &mut display.shadow, "Schatten"),
                ] {
                    if icons::button(ui, icon, *flag, true, tip).clicked() {
                        *flag = !*flag;
                    }
                }
                ui.separator();
                if icons::button(ui, Icon::Camera, false, true, "Bildschirmfoto speichern").clicked() {
                    shot = true;
                }
            }
        });
        if shot {
            self.screenshot_requested = true;
        }
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
            let tex = self.viewport.render(&self.rs, size, &self.camera, self.display);
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
            Color32::from_gray(30),
        );
    }

    fn view_2d(&mut self, ui: &mut egui::Ui) {
        let (editor, doc) = (&mut self.editor, &mut self.doc);
        Self::header(ui, "2D-Schnitt", |ui, row| {
            if row == 0 { editor.toolbar(ui, doc) } else { editor.display_toolbar(ui) }
        });
        self.editor.ui(ui, &mut self.doc);
    }

    /// Rechte Leiste: oben die Objektliste, darunter die Eigenschaften der Auswahl
    fn side_panel(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        Self::header(ui, "Objekte", |_, _| {});
        egui::ScrollArea::vertical().id_salt("objekte").max_height(ui.available_height() * 0.45).show(ui, |ui| {
            egui::Frame::NONE.inner_margin(egui::Margin::same(8)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new("Avatar").weak().small());
                match &self.avatar {
                    Some(a) => {
                        let name = a.name.clone();
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut self.display.avatar, "").on_hover_text("Anzeigen / ausblenden");
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
        Self::header(ui, "Eigenschaften", |_, _| {});
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
                        ui.horizontal(|ui| {
                            if ui.button("Neu importieren …").on_hover_text("Einheit, Achsen und Boden ändern").clicked() {
                                if let Some(p) = self.avatar_path.clone() {
                                    self.import_dialog = Some((p, self.avatar_opts));
                                }
                            }
                            if ui.button("Avatar entfernen").clicked() {
                                self.remove_avatar(&ctx);
                            }
                        });
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

    /// Import-Fenster: Einheit, Achsen, Boden – wie beim Import in Marvelous Designer
    fn import_window(&mut self, ctx: &egui::Context) {
        let Some((path, mut o)) = self.import_dialog.clone() else { return };
        let mut result: Option<bool> = None;
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        egui::Modal::new(egui::Id::unique("import")).show(ctx, |ui| {
            ui.set_width(360.0);
            ui.heading("Avatar importieren");
            ui.label(RichText::new(&name).weak());
            ui.add_space(6.0);

            let section = |ui: &mut egui::Ui, title: &str, add: &mut dyn FnMut(&mut egui::Ui)| {
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(title).strong());
                    add(ui);
                });
            };

            section(ui, "Skalierung", &mut |ui| {
                ui.checkbox(&mut o.auto_unit, "Einheit aus der Datei übernehmen")
                    .on_hover_text("FBX, DAE und USD speichern ihre Einheit; bei OBJ und glTF wird sie an der Größe erkannt");
                ui.add_enabled_ui(!o.auto_unit, |ui| {
                    egui::ComboBox::from_id_salt("einheit").selected_text(o.unit.label()).width(240.0).show_ui(ui, |ui| {
                        for u in Unit::ALL {
                            ui.selectable_value(&mut o.unit, u, u.label());
                        }
                    });
                });
                ui.horizontal(|ui| {
                    ui.label("Zusätzlich");
                    ui.add(egui::DragValue::new(&mut o.percent).range(1.0..=10000.0).speed(0.5).suffix(" %"));
                });
            });
            ui.add_space(4.0);
            section(ui, "Achsen", &mut |ui| {
                ui.checkbox(&mut o.auto_axes, "Achsen aus der Datei übernehmen")
                    .on_hover_text("FBX, DAE und USD sagen, welche Achse oben ist; sonst gilt Y oben");
                ui.add_enabled_ui(!o.auto_axes, |ui| {
                    ui.horizontal(|ui| {
                        for (k, target) in ["X", "Y (oben)", "Z"].iter().enumerate() {
                            ui.vertical(|ui| {
                                ui.label(RichText::new(*target).small());
                                let (src, inv) = &mut o.axes[k];
                                egui::ComboBox::from_id_salt(("achse", k))
                                    .selected_text(["X", "Y", "Z"][*src])
                                    .width(60.0)
                                    .show_ui(ui, |ui| {
                                        for (i, n) in ["X", "Y", "Z"].iter().enumerate() {
                                            ui.selectable_value(src, i, *n);
                                        }
                                    });
                                ui.checkbox(inv, "Umkehren");
                            });
                        }
                    });
                    if !o.axes_valid() {
                        ui.label(RichText::new("Jede Achse der Datei nur einmal zuordnen").color(Color32::from_rgb(230, 170, 80)).small());
                    }
                });
            });
            ui.add_space(4.0);
            section(ui, "Position", &mut |ui| {
                ui.checkbox(&mut o.ground, "Am Boden ausrichten").on_hover_text("Die Füße stehen auf der Bodenebene");
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Zurücksetzen").clicked() {
                    o = ImportOptions::default();
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Abbrechen").clicked() {
                        result = Some(false);
                    }
                    let ok = o.auto_axes || o.axes_valid();
                    if ui.add_enabled(ok, egui::Button::new("OK")).clicked() {
                        result = Some(true);
                    }
                });
            });
        });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            result = Some(false);
        }
        match result {
            Some(true) => {
                self.import_dialog = None;
                self.import_opts = o;
                self.start_loading(ctx, &path, o);
            }
            Some(false) => self.import_dialog = None,
            None => self.import_dialog = Some((path, o)),
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
        if self.layout == Layout::Only3D {
            egui::CentralPanel::no_frame().show(ui, |ui| self.view_3d(ui));
        } else if self.layout == Layout::Only2D {
            egui::CentralPanel::no_frame().show(ui, |ui| self.view_2d(ui));
        } else {
        egui::Panel::left("3d")
            .resizable(true)
            .default_size((width - 270.0) * 0.4)
            .size_range(250.0..=(width - 270.0 - 300.0).max(260.0))
            .frame(egui::Frame::NONE)
            .show(ui, |ui| self.view_3d(ui));
        egui::CentralPanel::no_frame().show(ui, |ui| self.view_2d(ui));
        }

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

        self.import_window(&ctx);
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
