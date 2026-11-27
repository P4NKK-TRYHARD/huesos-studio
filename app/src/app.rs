//! Estado del editor, archivos, atajos y bucle principal.

use crate::filedlg::{FileDialog, Outcome, Purpose};
use crate::model::*;
use crate::pose::Physics;
use crate::sample::sample_doc;
use egui::{vec2, Color32, Key as EKey, TextureHandle, TextureId, Vec2};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Setup,
    Animate,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Select,
    Rotate,
    Translate,
    Scale,
    Create,
    Vertices,
}

impl Tool {
    pub const ALL: [Tool; 6] = [Tool::Select, Tool::Rotate, Tool::Translate, Tool::Scale, Tool::Create, Tool::Vertices];
    pub fn label(self) -> &'static str {
        match self {
            Tool::Select => "⬉ Seleccionar",
            Tool::Rotate => "⟲ Rotar",
            Tool::Translate => "✋ Mover",
            Tool::Scale => "🔍 Escalar",
            Tool::Create => "✚ Crear hueso",
            Tool::Vertices => "✏ Vértices",
        }
    }
    pub fn shortcut(self) -> &'static str {
        match self {
            Tool::Select => "V",
            Tool::Rotate => "R",
            Tool::Translate => "T",
            Tool::Scale => "S",
            Tool::Create => "C",
            Tool::Vertices => "M",
        }
    }
    pub fn tip(self) -> &'static str {
        match self {
            Tool::Vertices => "Mover vértices de mallas y puntos de trayectorias. En modo animar crea claves de deformación.",
            _ => "",
        }
    }
}

/// Referencia a una clave de cualquier línea de tiempo.
#[derive(Clone, Debug, PartialEq)]
pub enum KeyRef {
    Bone(u32, Channel, u32),
    Att(u32, u32),
    Color(u32, u32),
    Deform(u32, String, u32),
    Event(u32, String),
    Con(String, u32),
    Draw(u32),
}

impl KeyRef {
    pub fn frame(&self) -> u32 {
        match self {
            KeyRef::Bone(_, _, f) | KeyRef::Att(_, f) | KeyRef::Color(_, f) | KeyRef::Deform(_, _, f) | KeyRef::Event(f, _) | KeyRef::Con(_, f) | KeyRef::Draw(f) => *f,
        }
    }
}

pub enum Drag {
    None,
    Pan,
    Rotate { bone: u32, last: f32 },
    Translate { bone: u32, last: [f32; 2] },
    Scale { bone: u32, start_dist: f32, start: [f32; 2] },
    Create { parent: Option<u32>, start: [f32; 2], cur: [f32; 2] },
    Key(KeyRef),
    Vertex { slot: u32, att: String, idx: usize, last: [f32; 2] },
    PathPt { slot: u32, att: String, idx: usize, part: u8 },
    GraphKey { bone: u32, ch: Channel, frame: u32, dim: usize },
}

pub const BG: Color32 = Color32::from_rgb(38, 40, 44);
pub const PANEL: Color32 = Color32::from_rgb(48, 50, 55);
pub const ACCENT: Color32 = Color32::from_rgb(255, 150, 40);
pub const SEL: Color32 = Color32::from_rgb(90, 200, 255);

pub struct EditorApp {
    pub doc: Doc,
    pub committed: Doc,
    pub undo: Vec<Doc>,
    pub redo: Vec<Doc>,
    pub mode: Mode,
    pub tool: Tool,
    pub selected: Option<u32>,
    pub sel_slot: Option<u32>,
    pub sel_constraint: Option<usize>,
    /// Adjunto que se edita en el panel del slot.
    pub sel_att: Option<String>,
    pub selected_key: Option<KeyRef>,
    pub skin: usize,
    pub anim: usize,
    pub frame: f32,
    pub playing: bool,
    pub looping: bool,
    pub speed: f32,
    pub pan: Vec2,
    pub zoom: f32,
    pub drag: Drag,
    pub show_bones: bool,
    pub show_images: bool,
    pub show_names: bool,
    pub show_mesh: bool,
    pub onion: bool,
    pub graph_view: bool,
    pub graph_ch: Channel,
    pub graph_dim: usize,
    pub phys: Physics,
    pub phys_on: bool,
    pub fired: Vec<(String, f64)>,
    pub bind_bones: Vec<u32>,
    pub grid_cols: usize,
    pub grid_rows: usize,
    pub path: Option<PathBuf>,
    pub dirty: bool,
    pub status: String,
    pub textures: HashMap<String, Option<TextureHandle>>,
    pub screenshot: Option<PathBuf>,
    pub frames_drawn: u32,
    pub shot_sent: bool,
    pub confirm_new: bool,
    pub file_dialog: Option<FileDialog>,
}

impl EditorApp {
    pub fn new(cc: &eframe::CreationContext<'_>, screenshot: Option<PathBuf>, open: Option<PathBuf>, opts: Vec<(String, String)>) -> Self {
        crate::draw::setup_style(&cc.egui_ctx);
        let doc = sample_doc();
        let mut app = Self {
            committed: doc.clone(),
            doc,
            undo: Vec::new(),
            redo: Vec::new(),
            mode: Mode::Animate,
            tool: Tool::Rotate,
            selected: None,
            sel_slot: None,
            sel_constraint: None,
            sel_att: None,
            selected_key: None,
            skin: 0,
            anim: 0,
            frame: 0.0,
            playing: false,
            looping: true,
            speed: 1.0,
            pan: vec2(0.0, 150.0),
            zoom: 1.25,
            drag: Drag::None,
            show_bones: true,
            show_images: true,
            show_names: false,
            show_mesh: true,
            onion: false,
            graph_view: false,
            graph_ch: Channel::Rotate,
            graph_dim: 0,
            phys: Physics::default(),
            phys_on: true,
            fired: Vec::new(),
            bind_bones: Vec::new(),
            grid_cols: 4,
            grid_rows: 4,
            path: None,
            dirty: false,
            status: "Listo. Proyecto de ejemplo cargado.".into(),
            textures: HashMap::new(),
            screenshot,
            frames_drawn: 0,
            shot_sent: false,
            confirm_new: false,
            file_dialog: None,
        };
        app.selected = app.doc.bones.iter().find(|b| b.name == "muslo_frente").map(|b| b.id);
        app.frame = 6.0;
        if let Some(p) = open {
            let is_spine = p.extension().map(|e| e == "json").unwrap_or(false)
                && std::fs::read_to_string(&p).map(|s| s.contains("\"skeleton\"")).unwrap_or(false);
            if is_spine {
                app.import_spine(p);
            } else {
                app.load(p);
            }
        }
        app.apply_opts(&opts);
        app
    }

    /// Opciones de línea de órdenes (útiles para abrir en un estado concreto).
    fn apply_opts(&mut self, opts: &[(String, String)]) {
        for (k, v) in opts {
            match k.as_str() {
                "animacion" => {
                    if let Some(i) = self.doc.animations.iter().position(|a| a.name == *v).or_else(|| v.parse().ok()) {
                        self.anim = i.min(self.doc.animations.len().saturating_sub(1));
                        self.mode = Mode::Animate;
                    }
                }
                "fotograma" => self.frame = v.parse().unwrap_or(0.0),
                "hueso" => {
                    self.selected = self.doc.bones.iter().find(|b| b.name == *v).map(|b| b.id);
                    self.sel_slot = None;
                }
                "slot" => {
                    self.sel_slot = self.doc.slots.iter().find(|s| s.name == *v).map(|s| s.id);
                    self.selected = self.sel_slot.and_then(|s| self.doc.slot(s)).map(|s| s.bone);
                }
                "restriccion" => self.sel_constraint = v.parse().ok().filter(|i: &usize| *i < self.doc.constraints.len()),
                "skin" => {
                    if let Some(i) = self.doc.skins.iter().position(|s| s.name == *v).or_else(|| v.parse().ok()) {
                        self.skin = i.min(self.doc.skins.len().saturating_sub(1));
                    }
                }
                "herramienta" => {
                    self.tool = match v.as_str() {
                        "vertices" => Tool::Vertices,
                        "mover" => Tool::Translate,
                        "escalar" => Tool::Scale,
                        "crear" => Tool::Create,
                        "seleccionar" => Tool::Select,
                        _ => Tool::Rotate,
                    }
                }
                "zoom" => self.zoom = v.parse().unwrap_or(self.zoom),
                "ejemplo" if v == "restricciones" => {
                    self.set_doc(crate::sample::demo_doc());
                    self.mode = Mode::Animate;
                    self.pan = vec2(0.0, 0.0);
                    self.zoom = 1.0;
                }
                "grafico" => self.graph_view = true,
                "configurar" => self.mode = Mode::Setup,
                "reproducir" => self.playing = true,
                "exportar" => {
                    let p = PathBuf::from(v);
                    self.export_spine(p);
                    eprintln!("{}", self.status);
                }
                _ => {}
            }
        }
    }

    // ---------- archivo ----------

    pub fn load(&mut self, p: PathBuf) {
        let res = std::fs::read_to_string(&p)
            .map_err(|e| e.to_string())
            .and_then(|s| serde_json::from_str::<Doc>(&s).map_err(|e| e.to_string()));
        match res {
            Ok(mut d) => {
                d.migrate();
                self.set_doc(d);
                self.status = format!("Abierto: {}", p.display());
                self.path = Some(p);
            }
            Err(e) => self.status = format!("No se pudo abrir: {e}"),
        }
    }

    pub fn set_doc(&mut self, d: Doc) {
        self.doc = d;
        self.committed = self.doc.clone();
        self.undo.clear();
        self.redo.clear();
        self.selected = None;
        self.sel_slot = None;
        self.sel_constraint = None;
        self.selected_key = None;
        self.skin = 0;
        self.anim = 0;
        self.frame = 0.0;
        self.playing = false;
        self.dirty = false;
        self.path = None;
        self.phys.reset();
    }

    pub fn save(&mut self, save_as: bool) {
        match (&self.path, save_as) {
            (Some(p), false) => {
                let p = p.clone();
                self.save_to(p);
            }
            _ => self.file_dialog = Some(FileDialog::new(Purpose::Save, self.path.as_deref())),
        }
    }

    pub fn save_to(&mut self, path: PathBuf) {
        let res = serde_json::to_string_pretty(&self.doc)
            .map_err(|e| e.to_string())
            .and_then(|s| std::fs::write(&path, s).map_err(|e| e.to_string()));
        match res {
            Ok(()) => {
                self.status = format!("Guardado: {}", path.display());
                self.path = Some(path);
                self.dirty = false;
            }
            Err(e) => self.status = format!("No se pudo guardar: {e}"),
        }
    }

    pub fn open_dialog(&mut self) {
        self.file_dialog = Some(FileDialog::new(Purpose::Open, self.path.as_deref()));
    }

    pub fn export_dialog(&mut self) {
        let start = self.path.as_ref().map(|p| p.with_extension("spine.json"));
        self.file_dialog = Some(FileDialog::new(Purpose::ExportSpine, start.as_deref().or(self.path.as_deref())));
    }

    pub fn import_dialog(&mut self) {
        self.file_dialog = Some(FileDialog::new(Purpose::ImportSpine, self.path.as_deref()));
    }

    pub fn export_spine(&mut self, p: PathBuf) {
        match crate::spine_io::export(&self.doc, &p) {
            Ok(n) => self.status = format!("Exportado a formato Spine: {} ({n} imágenes en el atlas)", p.display()),
            Err(e) => self.status = format!("No se pudo exportar: {e}"),
        }
    }

    pub fn import_spine(&mut self, p: PathBuf) {
        match crate::spine_io::import(&p) {
            Ok(d) => {
                self.set_doc(d);
                self.mode = Mode::Animate;
                self.status = format!("Importado de Spine: {}", p.display());
            }
            Err(e) => self.status = format!("No se pudo importar: {e}"),
        }
    }

    pub fn pick_image(&mut self, slot: u32, att: String) {
        let start = self
            .doc
            .find_att(self.skin, slot, &att)
            .map(|(_, a)| a)
            .filter(|a| !a.image.is_empty())
            .map(|a| PathBuf::from(&a.image))
            .or_else(|| self.path.clone());
        self.file_dialog = Some(FileDialog::new(Purpose::Image(slot), start.as_deref()));
        if let Some(d) = self.file_dialog.as_mut() {
            d.tag = att;
        }
    }

    fn file_dialog_ui(&mut self, ctx: &egui::Context) {
        let Some(dlg) = self.file_dialog.as_mut() else { return };
        let purpose = dlg.purpose;
        let tag = dlg.tag.clone();
        match dlg.show(ctx) {
            Outcome::Pending => {}
            Outcome::Cancel => self.file_dialog = None,
            Outcome::Picked(p) => {
                self.file_dialog = None;
                match purpose {
                    Purpose::Open => self.load(p),
                    Purpose::Save => self.save_to(p),
                    Purpose::ExportSpine => self.export_spine(p),
                    Purpose::ImportSpine => self.import_spine(p),
                    Purpose::Image(slot) => {
                        let size = image::image_dimensions(&p).ok();
                        self.textures.remove(&p.to_string_lossy().to_string());
                        if let Some(a) = self.doc.find_att_mut(self.skin, slot, &tag) {
                            if a.kind != AttachKind::Mesh {
                                a.kind = AttachKind::Image;
                            }
                            a.image = p.to_string_lossy().to_string();
                            if let (Some((w, h)), true) = (size, a.kind == AttachKind::Image) {
                                a.w = w as f32;
                                a.h = h as f32;
                            }
                            a.tint = [255, 255, 255, 255];
                        }
                        self.status = "Imagen asignada".into();
                    }
                }
            }
        }
    }

    pub fn do_undo(&mut self) {
        if let Some(prev) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.doc, prev));
            self.committed = self.doc.clone();
            self.fix_selection();
            self.status = "Deshacer".into();
        }
    }

    pub fn do_redo(&mut self) {
        if let Some(next) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.doc, next));
            self.committed = self.doc.clone();
            self.fix_selection();
            self.status = "Rehacer".into();
        }
    }

    pub fn fix_selection(&mut self) {
        if let Some(s) = self.selected {
            if self.doc.bone(s).is_none() {
                self.selected = None;
            }
        }
        if let Some(s) = self.sel_slot {
            if self.doc.slot(s).is_none() {
                self.sel_slot = None;
            }
        }
        if let Some(c) = self.sel_constraint {
            if c >= self.doc.constraints.len() {
                self.sel_constraint = None;
            }
        }
        if self.anim >= self.doc.animations.len() {
            self.anim = self.doc.animations.len().saturating_sub(1);
        }
        if self.skin >= self.doc.skins.len() {
            self.skin = 0;
        }
        self.selected_key = None;
    }

    // ---------- pose ----------

    pub fn cur_anim(&self) -> Option<&Animation> {
        if self.mode == Mode::Animate {
            self.doc.animations.get(self.anim)
        } else {
            None
        }
    }

    pub fn key_frame(&self) -> u32 {
        self.frame.round().max(0.0) as u32
    }

    pub fn current_local(&self, id: u32) -> Local {
        match self.doc.bone(id) {
            Some(b) => self.doc.local_pose(b, self.cur_anim(), self.frame),
            None => Local { sx: 1.0, sy: 1.0, ..Default::default() },
        }
    }

    /// Adjunto visible de un slot en el fotograma actual.
    pub fn slot_att(&self, slot: u32) -> Option<String> {
        let s = self.doc.slot(slot)?;
        if let Some(a) = self.cur_anim() {
            if let Some(n) = a.att_at(slot, self.frame) {
                return n;
            }
        }
        s.attachment.clone()
    }

    /// Orden de dibujo en el fotograma actual.
    pub fn cur_draw_order(&self) -> Vec<u32> {
        let mut o = self.cur_anim().and_then(|a| a.draw_at(self.frame)).cloned().unwrap_or_else(|| self.doc.draw_order.clone());
        for s in &self.doc.draw_order {
            if !o.contains(s) {
                o.push(*s);
            }
        }
        o
    }

    pub fn slot_color(&self, slot: u32) -> [u8; 4] {
        let base = self.doc.slot(slot).map(|s| s.color).unwrap_or([255; 4]);
        self.cur_anim().and_then(|a| a.color_at(slot, self.frame)).unwrap_or(base)
    }

    pub fn write_local(&mut self, id: u32, l: Local, ch: Channel) {
        match self.mode {
            Mode::Setup => {
                if let Some(b) = self.doc.bone_mut(id) {
                    match ch {
                        Channel::Rotate => b.rotation = l.rot,
                        Channel::Translate => {
                            b.x = l.x;
                            b.y = l.y;
                        }
                        Channel::Scale => {
                            b.sx = l.sx;
                            b.sy = l.sy;
                        }
                        Channel::Shear => {
                            b.shx = l.shx;
                            b.shy = l.shy;
                        }
                    }
                }
            }
            Mode::Animate => {
                let f = self.key_frame();
                self.frame = f as f32;
                if let Some(a) = self.doc.animations.get_mut(self.anim) {
                    a.set_key(id, ch, f, ch.get(&l));
                }
            }
        }
    }

    pub fn key_all(&mut self, id: u32) {
        let l = self.current_local(id);
        let f = self.key_frame();
        if let Some(a) = self.doc.animations.get_mut(self.anim) {
            for ch in Channel::ALL {
                a.set_key(id, ch, f, ch.get(&l));
            }
        }
    }

    pub fn texture(&mut self, ctx: &egui::Context, path: &str) -> Option<TextureId> {
        if path.is_empty() {
            return None;
        }
        if !self.textures.contains_key(path) {
            let tex = image::open(path).ok().map(|img| {
                let rgba = img.to_rgba8();
                let size = [rgba.width() as usize, rgba.height() as usize];
                let ci = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
                ctx.load_texture(path, ci, egui::TextureOptions::LINEAR)
            });
            self.textures.insert(path.to_string(), tex);
        }
        self.textures.get(path).and_then(|t| t.as_ref().map(|t| t.id()))
    }

    /// Huesos en orden de árbol con su profundidad.
    pub fn tree_rows(&self) -> Vec<(u32, usize)> {
        fn rec(doc: &Doc, parent: Option<u32>, depth: usize, out: &mut Vec<(u32, usize)>) {
            for c in doc.children(parent) {
                out.push((c, depth));
                if depth < 256 {
                    rec(doc, Some(c), depth + 1, out);
                }
            }
        }
        let mut out = Vec::new();
        rec(&self.doc, None, 0, &mut out);
        out
    }

    pub fn reparent(&mut self, id: u32, new_parent: Option<u32>) {
        // Mantiene la posición en pantalla al cambiar de padre (pose base).
        let world = self.doc.world_fk(None, 0.0);
        let w = world.get(&id).copied().unwrap_or(Aff::IDENTITY);
        let p = new_parent.and_then(|p| world.get(&p).copied()).unwrap_or(Aff::IDENTITY);
        let o = p.inv_point(w.tx, w.ty);
        let ax = p.inv_vec(w.a, w.c);
        let ay = p.inv_vec(w.b, w.d);
        let sx = (ax[0] * ax[0] + ax[1] * ax[1]).sqrt();
        let det = ax[0] * ay[1] - ax[1] * ay[0];
        if let Some(b) = self.doc.bone_mut(id) {
            b.parent = new_parent;
            b.x = o[0];
            b.y = o[1];
            b.rotation = ax[1].atan2(ax[0]).to_degrees();
            b.sx = sx;
            b.sy = if sx > 1e-6 { det / sx } else { 1.0 };
        }
    }

    pub fn jump_key(&mut self, dir: i32) {
        self.playing = false;
        let Some(a) = self.doc.animations.get(self.anim) else { return };
        let cur = self.key_frame();
        let frames: Vec<u32> = match self.selected {
            Some(s) => {
                let mut v: Vec<u32> = a.tracks.iter().filter(|t| t.bone == s).flat_map(|t| t.keys.iter().map(|k| k.frame)).collect();
                v.sort();
                v.dedup();
                if v.is_empty() {
                    a.all_frames()
                } else {
                    v
                }
            }
            None => a.all_frames(),
        };
        let target = if dir < 0 { frames.iter().rev().find(|&&f| f < cur) } else { frames.iter().find(|&&f| f > cur) };
        if let Some(&t) = target {
            self.frame = t as f32;
        }
    }

    pub fn step_frame(&mut self, d: f32) {
        self.playing = false;
        self.frame = (self.frame.round() + d).max(0.0);
    }

    /// Mueve una clave a otro fotograma. Devuelve la nueva referencia.
    pub fn move_key(&mut self, k: &KeyRef, nf: u32) -> Option<KeyRef> {
        let a = self.doc.animations.get_mut(self.anim)?;
        let out = match k {
            KeyRef::Bone(b, ch, f) => {
                let t = a.tracks.iter_mut().find(|t| t.bone == *b && t.channel == *ch)?;
                if t.keys.iter().any(|k| k.frame == nf) {
                    return None;
                }
                t.keys.iter_mut().find(|k| k.frame == *f)?.frame = nf;
                t.sort();
                KeyRef::Bone(*b, *ch, nf)
            }
            KeyRef::Att(s, f) => {
                if a.att_keys.iter().any(|k| k.slot == *s && k.frame == nf) {
                    return None;
                }
                a.att_keys.iter_mut().find(|k| k.slot == *s && k.frame == *f)?.frame = nf;
                a.att_keys.sort_by_key(|k| (k.slot, k.frame));
                KeyRef::Att(*s, nf)
            }
            KeyRef::Color(s, f) => {
                if a.color_keys.iter().any(|k| k.slot == *s && k.frame == nf) {
                    return None;
                }
                a.color_keys.iter_mut().find(|k| k.slot == *s && k.frame == *f)?.frame = nf;
                a.color_keys.sort_by_key(|k| (k.slot, k.frame));
                KeyRef::Color(*s, nf)
            }
            KeyRef::Deform(s, n, f) => {
                if a.deform_keys.iter().any(|k| k.slot == *s && k.att == *n && k.frame == nf) {
                    return None;
                }
                a.deform_keys.iter_mut().find(|k| k.slot == *s && k.att == *n && k.frame == *f)?.frame = nf;
                a.deform_keys.sort_by(|x, y| (x.slot, &x.att, x.frame).cmp(&(y.slot, &y.att, y.frame)));
                KeyRef::Deform(*s, n.clone(), nf)
            }
            KeyRef::Event(f, n) => {
                a.events.iter_mut().find(|k| k.frame == *f && k.name == *n)?.frame = nf;
                a.events.sort_by_key(|k| k.frame);
                KeyRef::Event(nf, n.clone())
            }
            KeyRef::Con(n, f) => {
                if a.con_keys.iter().any(|k| k.con == *n && k.frame == nf) {
                    return None;
                }
                a.con_keys.iter_mut().find(|k| k.con == *n && k.frame == *f)?.frame = nf;
                a.con_keys.sort_by(|x, y| (&x.con, x.frame).cmp(&(&y.con, y.frame)));
                KeyRef::Con(n.clone(), nf)
            }
            KeyRef::Draw(f) => {
                if a.draw_keys.iter().any(|k| k.frame == nf) {
                    return None;
                }
                a.draw_keys.iter_mut().find(|k| k.frame == *f)?.frame = nf;
                a.draw_keys.sort_by_key(|k| k.frame);
                KeyRef::Draw(nf)
            }
        };
        a.grow(nf);
        Some(out)
    }

    pub fn delete_selected_key(&mut self) -> bool {
        let Some(k) = self.selected_key.take() else { return false };
        let Some(a) = self.doc.animations.get_mut(self.anim) else { return false };
        match k {
            KeyRef::Bone(b, ch, f) => a.remove_key(b, ch, f),
            KeyRef::Att(s, f) => a.att_keys.retain(|k| !(k.slot == s && k.frame == f)),
            KeyRef::Color(s, f) => a.color_keys.retain(|k| !(k.slot == s && k.frame == f)),
            KeyRef::Deform(s, n, f) => a.deform_keys.retain(|k| !(k.slot == s && k.att == n && k.frame == f)),
            KeyRef::Event(f, n) => {
                if let Some(i) = a.events.iter().position(|e| e.frame == f && e.name == n) {
                    a.events.remove(i);
                }
            }
            KeyRef::Con(n, f) => a.con_keys.retain(|k| !(k.con == n && k.frame == f)),
            KeyRef::Draw(f) => a.draw_keys.retain(|k| k.frame != f),
        }
        true
    }

    // ---------- atajos ----------

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let typing = ctx.wants_keyboard_input();
        let (undo, redo, save, open, cmd) = ctx.input(|i| {
            let cmd = i.modifiers.command;
            (
                cmd && !i.modifiers.shift && i.key_pressed(EKey::Z),
                cmd && (i.key_pressed(EKey::Y) || (i.modifiers.shift && i.key_pressed(EKey::Z))),
                cmd && i.key_pressed(EKey::S),
                cmd && i.key_pressed(EKey::O),
                cmd,
            )
        });
        if undo && !typing {
            self.do_undo();
        }
        if redo && !typing {
            self.do_redo();
        }
        if save {
            self.save(false);
        }
        if open {
            self.open_dialog();
        }
        if typing || cmd {
            return;
        }
        let pressed = |k: EKey| ctx.input(|i| i.key_pressed(k));
        let tools = [
            (EKey::V, Tool::Select),
            (EKey::R, Tool::Rotate),
            (EKey::T, Tool::Translate),
            (EKey::S, Tool::Scale),
            (EKey::C, Tool::Create),
            (EKey::M, Tool::Vertices),
        ];
        for (k, t) in tools {
            if pressed(k) {
                self.tool = t;
            }
        }
        if pressed(EKey::Space) {
            self.playing = !self.playing;
            if self.playing {
                self.mode = Mode::Animate;
            }
        }
        if pressed(EKey::Comma) || pressed(EKey::ArrowLeft) {
            self.step_frame(-1.0);
        }
        if pressed(EKey::Period) || pressed(EKey::ArrowRight) {
            self.step_frame(1.0);
        }
        if pressed(EKey::Tab) {
            self.mode = if self.mode == Mode::Setup { Mode::Animate } else { Mode::Setup };
            self.playing = false;
        }
        if pressed(EKey::G) {
            self.graph_view = !self.graph_view;
        }
        if pressed(EKey::K) {
            if let Some(s) = self.selected {
                self.mode = Mode::Animate;
                self.key_all(s);
                self.status = "Clave fijada".into();
            }
        }
        if pressed(EKey::Delete) || pressed(EKey::Backspace) {
            if self.delete_selected_key() {
                self.status = "Clave borrada".into();
            } else if let (Some(s), Mode::Setup) = (self.selected, self.mode) {
                self.doc.delete_bone(s);
                self.selected = None;
                self.fix_selection();
                self.status = "Hueso borrado".into();
            }
        }
    }

    fn handle_screenshot(&mut self, ctx: &egui::Context) {
        let Some(path) = self.screenshot.clone() else { return };
        ctx.request_repaint();
        self.frames_drawn += 1;
        if self.frames_drawn >= 120 && !self.shot_sent {
            self.shot_sent = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
        }
        let shot = ctx.input(|i| {
            i.raw.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(img) = shot {
            let [w, h] = img.size;
            let mut buf = Vec::with_capacity(w * h * 4);
            for c in &img.pixels {
                let [r, g, b, _] = c.to_srgba_unmultiplied();
                buf.extend_from_slice(&[r, g, b, 255]);
            }
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Some(im) = image::RgbaImage::from_raw(w as u32, h as u32, buf) {
                if let Err(e) = im.save(&path) {
                    eprintln!("no se pudo guardar la captura: {e}");
                }
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn advance(&mut self, ctx: &egui::Context) {
        if !self.playing {
            return;
        }
        let Some(a) = self.doc.animations.get(self.anim) else { return };
        let dt = ctx.input(|i| i.stable_dt).min(0.1);
        let f0 = self.frame;
        let mut f1 = f0 + dt * a.fps * self.speed;
        let dur = a.duration as f32;
        let mut wrapped = false;
        if f1 > dur {
            if self.looping {
                f1 = if dur > 0.0 { f1 % dur } else { 0.0 };
                wrapped = true;
            } else {
                f1 = dur;
                self.playing = false;
            }
        }
        // eventos disparados en este paso
        let now = ctx.input(|i| i.time);
        for e in &a.events {
            let f = e.frame as f32;
            let hit = if wrapped { f > f0 || f <= f1 } else { f > f0 && f <= f1 };
            if hit || (f0 == 0.0 && f == 0.0 && f1 > 0.0) {
                let mut txt = format!("⚡ {}", e.name);
                if e.int != 0 {
                    txt.push_str(&format!("  i={}", e.int));
                }
                if e.float != 0.0 {
                    txt.push_str(&format!("  f={:.2}", e.float));
                }
                if !e.string.is_empty() {
                    txt.push_str(&format!("  «{}»", e.string));
                }
                self.fired.push((txt, now));
            }
        }
        self.frame = f1;
        ctx.request_repaint();
    }
}

impl eframe::App for EditorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.advance(ctx);
        let now = ctx.input(|i| i.time);
        self.fired.retain(|(_, t)| now - *t < 1.5);
        if !self.fired.is_empty() || (self.phys_on && !self.doc.constraints.iter().all(|c| !matches!(c.kind, ConstraintKind::Physics { .. }))) {
            ctx.request_repaint();
        }

        self.shortcuts(ctx);
        self.top_bar(ctx);
        self.timeline(ctx);
        self.left_panel(ctx);
        self.right_panel(ctx);
        self.canvas(ctx);
        self.new_dialog(ctx);
        self.file_dialog_ui(ctx);

        // historial: guarda un paso cuando termina la interacción
        let busy = ctx.input(|i| i.pointer.any_down());
        if !busy && self.doc != self.committed {
            self.undo.push(std::mem::replace(&mut self.committed, self.doc.clone()));
            if self.undo.len() > 300 {
                self.undo.remove(0);
            }
            self.redo.clear();
            self.dirty = true;
        }

        self.handle_screenshot(ctx);
    }
}
