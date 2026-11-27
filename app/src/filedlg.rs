//! Selector de archivos propio (igual en Windows y Linux).

use egui::{vec2, Align2, RichText};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    Open,
    Save,
    Image(u32),
    ExportSpine,
    ImportSpine,
}

impl Purpose {
    pub fn is_save(self) -> bool {
        matches!(self, Purpose::Save | Purpose::ExportSpine)
    }
}

pub enum Outcome {
    Pending,
    Cancel,
    Picked(PathBuf),
}

pub struct FileDialog {
    pub purpose: Purpose,
    dir: PathBuf,
    name: String,
    dir_text: String,
    error: Option<String>,
    /// Dato extra (p. ej. nombre del adjunto que recibe la imagen).
    pub tag: String,
}

pub fn home_dir() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

impl FileDialog {
    pub fn new(purpose: Purpose, start: Option<&Path>) -> Self {
        let dir = start
            .and_then(|p| if p.is_dir() { Some(p.to_path_buf()) } else { p.parent().map(|d| d.to_path_buf()) })
            .filter(|d| d.is_dir())
            .unwrap_or_else(home_dir);
        let name = match purpose {
            Purpose::Save => start
                .and_then(|p| p.file_name())
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "proyecto.json".into()),
            Purpose::ExportSpine => start
                .and_then(|p| p.file_stem())
                .map(|s| format!("{}.json", s.to_string_lossy().trim_end_matches(".spine")))
                .unwrap_or_else(|| "esqueleto.json".into()),
            _ => String::new(),
        };
        Self { purpose, dir_text: dir.display().to_string(), dir, name, error: None, tag: String::new() }
    }

    fn exts(&self) -> &'static [&'static str] {
        match self.purpose {
            Purpose::Image(_) => &["png", "jpg", "jpeg"],
            _ => &["json"],
        }
    }

    fn title(&self) -> &'static str {
        match self.purpose {
            Purpose::Open => "Abrir proyecto",
            Purpose::Save => "Guardar proyecto",
            Purpose::Image(_) => "Elegir imagen",
            Purpose::ExportSpine => "Exportar a Spine (JSON + atlas)",
            Purpose::ImportSpine => "Importar esqueleto de Spine (.json)",
        }
    }

    fn set_dir(&mut self, d: PathBuf) {
        self.dir_text = d.display().to_string();
        self.dir = d;
        self.error = None;
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let mut out = Outcome::Pending;
        let mut open = true;
        egui::Window::new(self.title())
            .collapsible(false)
            .resizable(true)
            .default_size(vec2(560.0, 420.0))
            .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .open(&mut open)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("⬆ Subir").clicked() {
                        if let Some(p) = self.dir.parent() {
                            self.set_dir(p.to_path_buf());
                        }
                    }
                    if ui.button("🏠 Inicio").clicked() {
                        self.set_dir(home_dir());
                    }
                    #[cfg(windows)]
                    for letter in b'C'..=b'Z' {
                        let d = PathBuf::from(format!("{}:\\", letter as char));
                        if d.exists() && ui.small_button(format!("{}:", letter as char)).clicked() {
                            self.set_dir(d);
                        }
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Carpeta");
                    let r = ui.add(egui::TextEdit::singleline(&mut self.dir_text).desired_width(f32::INFINITY));
                    if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        let d = PathBuf::from(&self.dir_text);
                        if d.is_dir() {
                            self.set_dir(d);
                        } else {
                            self.error = Some("Esa carpeta no existe".into());
                        }
                    }
                });
                ui.separator();

                let mut entries: Vec<(bool, String, PathBuf)> = Vec::new();
                if let Ok(rd) = std::fs::read_dir(&self.dir) {
                    for e in rd.flatten() {
                        let p = e.path();
                        let name = e.file_name().to_string_lossy().to_string();
                        if name.starts_with('.') {
                            continue;
                        }
                        if p.is_dir() {
                            entries.push((true, name, p));
                        } else {
                            let ext = p.extension().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
                            if self.exts().contains(&ext.as_str()) {
                                entries.push((false, name, p));
                            }
                        }
                    }
                }
                entries.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.to_lowercase().cmp(&b.1.to_lowercase())));

                let list_h = (ui.available_height() - 70.0).max(120.0);
                egui::ScrollArea::vertical().max_height(list_h).auto_shrink([false, false]).show(ui, |ui| {
                    if entries.is_empty() {
                        ui.label(RichText::new("(carpeta vacía)").weak());
                    }
                    for (is_dir, name, p) in &entries {
                        let label = if *is_dir { format!("📁 {name}") } else { format!("📄 {name}") };
                        let selected = !is_dir && self.name == *name;
                        let r = ui.selectable_label(selected, label);
                        if r.clicked() {
                            if *is_dir {
                                self.set_dir(p.clone());
                            } else {
                                self.name = name.clone();
                            }
                        }
                        if r.double_clicked() && !is_dir {
                            out = Outcome::Picked(p.clone());
                        }
                    }
                });
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Nombre");
                    ui.add(egui::TextEdit::singleline(&mut self.name).desired_width(300.0));
                    let ok_txt = match self.purpose { Purpose::Save => "Guardar", Purpose::ExportSpine => "Exportar", Purpose::ImportSpine => "Importar", _ => "Abrir" };
                    if ui.button(ok_txt).clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter)) && !self.name.is_empty() {
                        let mut p = self.dir.join(&self.name);
                        if self.purpose.is_save() && p.extension().is_none() {
                            p.set_extension("json");
                        }
                        if self.name.is_empty() {
                            self.error = Some("Escribe un nombre de archivo".into());
                        } else if !self.purpose.is_save() && !p.is_file() {
                            self.error = Some("Ese archivo no existe".into());
                        } else {
                            out = Outcome::Picked(p);
                        }
                    }
                    if ui.button("Cancelar").clicked() {
                        out = Outcome::Cancel;
                    }
                });
                if let Some(e) = &self.error {
                    ui.colored_label(egui::Color32::from_rgb(240, 110, 110), e);
                }
            });
        if !open {
            out = Outcome::Cancel;
        }
        out
    }
}
