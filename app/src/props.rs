//! Panel derecho: animaciones, propiedades de hueso, slot, adjunto, restricción y clave.

use crate::app::*;
use crate::draw::*;
use crate::model::*;
use egui::RichText;

fn bone_combo(ui: &mut egui::Ui, id: impl std::hash::Hash, doc: &Doc, value: &mut u32, exclude: &[u32]) -> bool {
    let txt = doc.bone(*value).map(|b| b.name.clone()).unwrap_or_else(|| "(ninguno)".into());
    let before = *value;
    egui::ComboBox::from_id_salt(id).selected_text(txt).width(150.0).show_ui(ui, |ui| {
        for b in &doc.bones {
            if exclude.contains(&b.id) {
                continue;
            }
            ui.selectable_value(value, b.id, &b.name);
        }
    });
    *value != before
}

fn mix_slider(ui: &mut egui::Ui, label: &str, v: &mut f32) {
    ui.label(label);
    ui.add(egui::Slider::new(v, 0.0..=1.0).max_decimals(2));
    ui.end_row();
}

impl EditorApp {
    pub fn right_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("props")
            .resizable(true)
            .default_width(300.0)
            .frame(egui::Frame::default().fill(PANEL).inner_margin(8.0))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.animations_ui(ui);
                    ui.separator();
                    if let Some(c) = self.sel_constraint {
                        self.constraint_ui(ui, c);
                    } else if let Some(s) = self.sel_slot {
                        self.slot_ui(ui, s);
                    } else {
                        self.bone_props_ui(ui);
                    }
                    ui.separator();
                    self.key_props_ui(ui);
                });
            });
    }

    fn animations_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Animaciones");
        let mut pick = None;
        for (i, a) in self.doc.animations.iter().enumerate() {
            let txt = format!("▶ {}  ({} f)", a.name, a.duration);
            if ui.selectable_label(i == self.anim, txt).clicked() {
                pick = Some(i);
            }
        }
        if let Some(i) = pick {
            self.anim = i;
            self.mode = Mode::Animate;
            self.frame = 0.0;
            self.selected_key = None;
            self.phys.reset();
        }
        ui.horizontal(|ui| {
            if ui.button("✚ Nueva").clicked() {
                let n = self.doc.animations.len() + 1;
                self.doc.animations.push(Animation::new(&format!("animación{n}")));
                self.anim = self.doc.animations.len() - 1;
                self.mode = Mode::Animate;
                self.frame = 0.0;
            }
            if ui.add_enabled(!self.doc.animations.is_empty(), egui::Button::new("⎘ Duplicar")).clicked() {
                let mut a = self.doc.animations[self.anim].clone();
                a.name.push_str(" copia");
                self.doc.animations.push(a);
                self.anim = self.doc.animations.len() - 1;
            }
            if ui.add_enabled(self.doc.animations.len() > 1, egui::Button::new("🗑")).on_hover_text("Borrar animación").clicked() {
                self.doc.animations.remove(self.anim);
                self.anim = self.anim.min(self.doc.animations.len() - 1);
                self.selected_key = None;
            }
        });
        if let Some(a) = self.doc.animations.get_mut(self.anim) {
            egui::Grid::new("anim_grid").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
                ui.label("Nombre");
                ui.text_edit_singleline(&mut a.name);
                ui.end_row();
                ui.label("Duración");
                ui.add(egui::DragValue::new(&mut a.duration).range(1..=10000).suffix(" f"));
                ui.end_row();
                ui.label("FPS");
                ui.add(egui::DragValue::new(&mut a.fps).range(1.0..=240.0));
                ui.end_row();
            });
        }
    }

    // ------------------------------------------------------------------ hueso

    fn bone_props_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Hueso");
        let Some(id) = self.selected else {
            ui.label(RichText::new("Selecciona un hueso, un slot o una restricción.").weak());
            return;
        };
        let Some(bone) = self.doc.bone(id) else {
            self.selected = None;
            return;
        };
        let animating = self.mode == Mode::Animate;
        if animating {
            ui.label(RichText::new(format!("Modo animar: los cambios crean claves en el fotograma {}", self.key_frame())).color(ACCENT).small());
        }

        let mut name = bone.name.clone();
        let parent = bone.parent;
        let candidates: Vec<(u32, String)> = self
            .doc
            .bones
            .iter()
            .filter(|b| b.id != id && !self.doc.is_descendant(b.id, id))
            .map(|b| (b.id, b.name.clone()))
            .collect();
        let ptxt = parent.and_then(|p| self.doc.bone(p)).map(|b| b.name.clone()).unwrap_or("(ninguno)".into());
        let mut new_parent = parent;
        let mut name_changed = false;
        egui::Grid::new("bone_grid").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
            ui.label("Nombre");
            name_changed = ui.text_edit_singleline(&mut name).changed();
            ui.end_row();
            ui.label("Padre");
            egui::ComboBox::from_id_salt("parent").selected_text(ptxt).show_ui(ui, |ui| {
                ui.selectable_value(&mut new_parent, None, "(ninguno)");
                for (cid, cname) in &candidates {
                    ui.selectable_value(&mut new_parent, Some(*cid), cname);
                }
            });
            ui.end_row();
        });
        if name_changed {
            if let Some(b) = self.doc.bone_mut(id) {
                b.name = name;
            }
        }
        if new_parent != parent {
            self.reparent(id, new_parent);
        }

        let mut l = self.current_local(id);
        let before = l;
        let f = self.key_frame();
        let has = |ch: Channel| animating && self.doc.animations.get(self.anim).map(|a| a.has_key(id, ch, f)).unwrap_or(false);
        let keyed = [has(Channel::Rotate), has(Channel::Translate), has(Channel::Scale), has(Channel::Shear)];
        let mut key_clicked: Option<Channel> = None;
        egui::Grid::new("tf_grid").num_columns(3).spacing([6.0, 4.0]).show(ui, |ui| {
            ui.label("Rotación");
            ui.add(egui::DragValue::new(&mut l.rot).speed(0.5).suffix("°").max_decimals(1));
            if animating && key_btn(ui, keyed[0], ch_color(Channel::Rotate)) {
                key_clicked = Some(Channel::Rotate);
            }
            ui.end_row();
            ui.label("Posición");
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut l.x).speed(0.5).prefix("x ").max_decimals(1));
                ui.add(egui::DragValue::new(&mut l.y).speed(0.5).prefix("y ").max_decimals(1));
            });
            if animating && key_btn(ui, keyed[1], ch_color(Channel::Translate)) {
                key_clicked = Some(Channel::Translate);
            }
            ui.end_row();
            ui.label("Escala");
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut l.sx).speed(0.01).prefix("x ").max_decimals(2));
                ui.add(egui::DragValue::new(&mut l.sy).speed(0.01).prefix("y ").max_decimals(2));
            });
            if animating && key_btn(ui, keyed[2], ch_color(Channel::Scale)) {
                key_clicked = Some(Channel::Scale);
            }
            ui.end_row();
            ui.label("Cizalla");
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut l.shx).speed(0.5).prefix("x ").suffix("°").max_decimals(1));
                ui.add(egui::DragValue::new(&mut l.shy).speed(0.5).prefix("y ").suffix("°").max_decimals(1));
            });
            if animating && key_btn(ui, keyed[3], ch_color(Channel::Shear)) {
                key_clicked = Some(Channel::Shear);
            }
            ui.end_row();
        });
        if l.rot != before.rot {
            self.write_local(id, l, Channel::Rotate);
        }
        if l.x != before.x || l.y != before.y {
            self.write_local(id, l, Channel::Translate);
        }
        if l.sx != before.sx || l.sy != before.sy {
            self.write_local(id, l, Channel::Scale);
        }
        if l.shx != before.shx || l.shy != before.shy {
            self.write_local(id, l, Channel::Shear);
        }
        if let Some(ch) = key_clicked {
            let l = self.current_local(id);
            if let Some(a) = self.doc.animations.get_mut(self.anim) {
                if a.has_key(id, ch, f) {
                    a.remove_key(id, ch, f);
                } else {
                    a.set_key(id, ch, f, ch.get(&l));
                }
            }
        }

        if let Some(b) = self.doc.bone_mut(id) {
            egui::Grid::new("bone_grid2").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
                ui.label("Longitud");
                ui.add(egui::DragValue::new(&mut b.length).speed(0.5).range(0.0..=5000.0).max_decimals(1));
                ui.end_row();
                ui.label("Color");
                ui.color_edit_button_srgba_unmultiplied(&mut b.color);
                ui.end_row();
                ui.label("Hereda");
                egui::ComboBox::from_id_salt("inherit").selected_text(b.inherit.label()).show_ui(ui, |ui| {
                    for i in Inherit::ALL {
                        ui.selectable_value(&mut b.inherit, i, i.label());
                    }
                })
                .response
                .on_hover_text("Qué transformaciones del padre sigue este hueso");
                ui.end_row();
            });
        }
        if animating && ui.button("🔑 Fijar clave de todo (K)").clicked() {
            self.key_all(id);
        }
        let cons: Vec<(usize, String)> = self
            .doc
            .constraints
            .iter()
            .enumerate()
            .filter(|(_, c)| c.bones.contains(&id))
            .map(|(i, c)| (i, format!("{} {}", c.kind.icon(), c.name)))
            .collect();
        if !cons.is_empty() {
            ui.add_space(4.0);
            ui.label(RichText::new("Controlado por:").small());
            for (i, txt) in cons {
                if ui.link(txt).clicked() {
                    self.sel_constraint = Some(i);
                }
            }
        }
        let slots = self.doc.slots_of(id);
        if !slots.is_empty() {
            ui.add_space(4.0);
            ui.label(RichText::new("Slots:").small());
            for sid in slots {
                let n = self.doc.slot(sid).map(|s| s.name.clone()).unwrap_or_default();
                if ui.link(format!("▣ {n}")).clicked() {
                    self.sel_slot = Some(sid);
                }
            }
        }
    }

    // ------------------------------------------------------------------ slot

    fn rename_att(&mut self, slot: u32, old: &str, new: &str) {
        if old == new || new.is_empty() || self.doc.att_names(slot).iter().any(|n| n == new) {
            return;
        }
        for sk in &mut self.doc.skins {
            for e in sk.entries.iter_mut().filter(|e| e.slot == slot && e.name == old) {
                e.name = new.to_string();
            }
        }
        if let Some(s) = self.doc.slot_mut(slot) {
            if s.attachment.as_deref() == Some(old) {
                s.attachment = Some(new.to_string());
            }
        }
        for a in &mut self.doc.animations {
            for k in a.att_keys.iter_mut().filter(|k| k.slot == slot && k.name.as_deref() == Some(old)) {
                k.name = Some(new.to_string());
            }
            for k in a.deform_keys.iter_mut().filter(|k| k.slot == slot && k.att == old) {
                k.att = new.to_string();
            }
        }
        self.sel_att = Some(new.to_string());
    }

    fn set_visible_att(&mut self, slot: u32, name: Option<String>) {
        match self.mode {
            Mode::Setup => {
                if let Some(s) = self.doc.slot_mut(slot) {
                    s.attachment = name;
                }
            }
            Mode::Animate => {
                let f = self.key_frame();
                if let Some(a) = self.doc.animations.get_mut(self.anim) {
                    a.set_att_key(slot, f, name);
                }
            }
        }
    }

    fn slot_ui(&mut self, ui: &mut egui::Ui, sid: u32) {
        ui.heading("Slot");
        let Some(slot) = self.doc.slot(sid).cloned() else {
            self.sel_slot = None;
            return;
        };
        let animating = self.mode == Mode::Animate;
        let f = self.key_frame();
        let mut name = slot.name.clone();
        let mut bone = slot.bone;
        let mut color = self.slot_color(sid);
        let color_before = color;
        let color_keyed = animating && self.doc.animations.get(self.anim).map(|a| a.color_keys.iter().any(|k| k.slot == sid && k.frame == f)).unwrap_or(false);
        let mut toggle_color_key = false;
        egui::Grid::new("slot_grid").num_columns(3).spacing([8.0, 4.0]).show(ui, |ui| {
            ui.label("Nombre");
            ui.text_edit_singleline(&mut name);
            ui.end_row();
            ui.label("Hueso");
            bone_combo(ui, "slot_bone", &self.doc, &mut bone, &[]);
            ui.end_row();
            ui.label("Color");
            ui.color_edit_button_srgba_unmultiplied(&mut color);
            if animating && key_btn(ui, color_keyed, egui::Color32::from_rgb(230, 200, 90)) {
                toggle_color_key = true;
            }
            ui.end_row();
        });
        if let Some(s) = self.doc.slot_mut(sid) {
            if name != s.name && !name.is_empty() {
                s.name = name;
            }
            s.bone = bone;
        }
        if color != color_before {
            match self.mode {
                Mode::Setup => {
                    if let Some(s) = self.doc.slot_mut(sid) {
                        s.color = color;
                    }
                }
                Mode::Animate => {
                    if let Some(a) = self.doc.animations.get_mut(self.anim) {
                        a.set_color_key(sid, f, color);
                    }
                }
            }
        }
        if toggle_color_key {
            if let Some(a) = self.doc.animations.get_mut(self.anim) {
                if color_keyed {
                    a.color_keys.retain(|k| !(k.slot == sid && k.frame == f));
                } else {
                    a.set_color_key(sid, f, color);
                }
            }
        }

        ui.add_space(6.0);
        ui.label(RichText::new("Adjuntos").strong());
        if animating {
            ui.label(RichText::new("Elegir el visible crea una clave de adjunto").color(ACCENT).small());
        }
        let names = self.doc.att_names(sid);
        let cur = self.slot_att(sid);
        let mut set_vis: Option<Option<String>> = None;
        if ui.radio(cur.is_none(), "(ninguno)").clicked() {
            set_vis = Some(None);
        }
        let editing = self.sel_att.clone().filter(|n| names.contains(n)).or_else(|| cur.clone()).or_else(|| names.first().cloned());
        for n in &names {
            let where_ = match self.doc.find_att(self.skin, sid, n) {
                Some((si, a)) => format!("{} · {}", a.kind.label(), self.doc.skins[si].name),
                None => "solo en otra skin".into(),
            };
            ui.horizontal(|ui| {
                if ui.radio(cur.as_deref() == Some(n.as_str()), "").on_hover_text("Visible").clicked() {
                    set_vis = Some(Some(n.clone()));
                }
                if ui.selectable_label(editing.as_deref() == Some(n.as_str()), format!("{n}")).on_hover_text("Editar este adjunto").clicked() {
                    self.sel_att = Some(n.clone());
                }
                ui.label(RichText::new(where_).weak().small());
            });
        }
        if let Some(v) = set_vis {
            if let Some(n) = &v {
                self.sel_att = Some(n.clone());
            }
            self.set_visible_att(sid, v);
        }
        ui.horizontal_wrapped(|ui| {
            let len = self.doc.bone(slot.bone).map(|b| b.length).unwrap_or(30.0).max(30.0);
            let mut new_att: Option<(String, Attachment, bool)> = None;
            if ui.button("✚ Forma").on_hover_text("Rectángulo de color").clicked() {
                new_att = Some(("forma".into(), Attachment::rect(len, len * 0.4, len / 2.0, 0.0, [200, 200, 200, 255]), false));
            }
            if ui.button("✚ Imagen…").clicked() {
                let mut a = Attachment::rect(len, len, len / 2.0, 0.0, [255; 4]);
                a.kind = AttachKind::Image;
                new_att = Some(("imagen".into(), a, true));
            }
            if ui.button("✚ Trayectoria").clicked() {
                new_att = Some(("trayectoria".into(), Attachment::path_line(len.max(80.0) * 2.0), false));
            }
            let rect_poly = vec![[0.0, -len * 0.3], [len, -len * 0.3], [len, len * 0.3], [0.0, len * 0.3]];
            if ui.button("✚ Colisión").on_hover_text("Caja de colisión (polígono para el juego)").clicked() {
                new_att = Some(("colision".into(), Attachment::polygon(AttachKind::BoundingBox, rect_poly.clone()), false));
            }
            if ui.button("✚ Recorte").on_hover_text("Polígono que recorta los slots que se dibujan después").clicked() {
                new_att = Some(("recorte".into(), Attachment::polygon(AttachKind::Clipping, rect_poly), false));
            }
            if ui.button("✚ Punto").on_hover_text("Punto con dirección (para disparos, partículas...)").clicked() {
                new_att = Some(("punto".into(), Attachment::point(len, 0.0, 0.0), false));
            }
            if let Some((base, att, pick)) = new_att {
                let taken = self.doc.att_names(sid);
                let n = unique(&base, |x| taken.iter().any(|t| t == x));
                self.doc.put_attachment(self.skin, sid, &n, att);
                if cur.is_none() {
                    self.set_visible_att(sid, Some(n.clone()));
                }
                self.sel_att = Some(n.clone());
                if pick {
                    self.pick_image(sid, n);
                }
            }
        });
        if let Some(n) = editing {
            ui.separator();
            self.attachment_editor(ui, sid, slot.bone, &n);
        }
    }

    fn attachment_editor(&mut self, ui: &mut egui::Ui, sid: u32, slot_bone: u32, name: &str) {
        let Some((owner, orig)) = self.doc.find_att(self.skin, sid, name).map(|(i, a)| (i, a.clone())) else {
            ui.label(RichText::new("Este adjunto no existe en la skin activa ni en la predeterminada.").weak());
            if ui.button("Crear aquí una copia").clicked() {
                let copy = self.doc.skins.iter().flat_map(|s| s.entries.iter()).find(|e| e.slot == sid && e.name == name).map(|e| e.att.clone());
                if let Some(a) = copy {
                    self.doc.put_attachment(self.skin, sid, name, a);
                }
            }
            return;
        };
        let mut a = orig.clone();
        let owner_name = self.doc.skins[owner].name.clone();
        ui.label(RichText::new(format!("Adjunto «{name}»")).strong());
        let mut new_name = name.to_string();
        let mut rename = false;
        let mut pick_image = false;
        let mut delete = false;
        let mut copy_to_skin = false;
        let mut action: Option<&'static str> = None;
        egui::Grid::new("att_grid").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
            ui.label("Nombre");
            let r = ui.text_edit_singleline(&mut new_name);
            rename = r.lost_focus() && new_name != name;
            ui.end_row();
            ui.label("Skin");
            ui.horizontal(|ui| {
                ui.label(&owner_name);
                if owner != self.skin && self.skin > 0 {
                    let tgt = self.doc.skins[self.skin].name.clone();
                    if ui.small_button(format!("Copiar a «{tgt}»")).on_hover_text("Para que esta skin tenga su propia versión").clicked() {
                        copy_to_skin = true;
                    }
                }
            });
            ui.end_row();
            ui.label("Tipo");
            match a.kind {
                AttachKind::Rect | AttachKind::Ellipse | AttachKind::Image => {
                    egui::ComboBox::from_id_salt("att_kind").selected_text(a.kind.label()).show_ui(ui, |ui| {
                        for k in [AttachKind::Rect, AttachKind::Ellipse, AttachKind::Image] {
                            ui.selectable_value(&mut a.kind, k, k.label());
                        }
                    });
                }
                k => {
                    ui.label(k.label());
                }
            }
            ui.end_row();
            if matches!(a.kind, AttachKind::Image | AttachKind::Mesh) {
                ui.label("Archivo");
                ui.horizontal(|ui| {
                    let short = std::path::Path::new(&a.image).file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or("(ninguno)".into());
                    ui.label(short).on_hover_text(&a.image);
                    if ui.small_button("Elegir…").clicked() {
                        pick_image = true;
                    }
                });
                ui.end_row();
            }
            if matches!(a.kind, AttachKind::Rect | AttachKind::Ellipse | AttachKind::Image) {
                ui.label("Tamaño");
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut a.w).speed(0.5).prefix("an ").range(1.0..=10000.0).max_decimals(1));
                    ui.add(egui::DragValue::new(&mut a.h).speed(0.5).prefix("al ").range(1.0..=10000.0).max_decimals(1));
                });
                ui.end_row();
                ui.label("Desplazar");
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut a.ox).speed(0.5).prefix("x ").max_decimals(1));
                    ui.add(egui::DragValue::new(&mut a.oy).speed(0.5).prefix("y ").max_decimals(1));
                });
                ui.end_row();
                ui.label("Girar");
                ui.add(egui::DragValue::new(&mut a.rot).speed(0.5).suffix("°").max_decimals(1));
                ui.end_row();
            }
            if a.kind == AttachKind::Point {
                ui.label("Posición");
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut a.ox).speed(0.5).prefix("x ").max_decimals(1));
                    ui.add(egui::DragValue::new(&mut a.oy).speed(0.5).prefix("y ").max_decimals(1));
                });
                ui.end_row();
                ui.label("Girar");
                ui.add(egui::DragValue::new(&mut a.rot).speed(0.5).suffix("°").max_decimals(1));
                ui.end_row();
            }
            ui.label(if matches!(a.kind, AttachKind::BoundingBox | AttachKind::Clipping | AttachKind::Point | AttachKind::Path) { "Color" } else { "Tinte" });
            ui.color_edit_button_srgba_unmultiplied(&mut a.tint);
            ui.end_row();
        });

        // ---- mallas
        match a.kind {
            AttachKind::Rect | AttachKind::Image => {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label("Rejilla");
                    ui.add(egui::DragValue::new(&mut self.grid_cols).range(1..=40).prefix("col "));
                    ui.add(egui::DragValue::new(&mut self.grid_rows).range(1..=40).prefix("fil "));
                });
                if ui.button("✏ Convertir en malla deformable").on_hover_text("Crea vértices que puedes mover y deformar").clicked() {
                    action = Some("to_mesh");
                }
            }
            AttachKind::Mesh => {
                let (nv, nt, weighted, hull) = a.mesh.as_ref().map(|m| (m.verts.len(), m.tris.len(), m.weighted(), m.hull)).unwrap_or((0, 0, false, 0));
                ui.label(RichText::new(format!("{nv} vértices · {nt} triángulos · contorno {hull}")).small());
                ui.label(RichText::new("Herramienta Vértices (M): arrastra los puntos. En modo animar se crean claves de deformación.").weak().small());
                ui.horizontal(|ui| {
                    ui.label("Rejilla");
                    ui.add(egui::DragValue::new(&mut self.grid_cols).range(1..=40).prefix("col "));
                    ui.add(egui::DragValue::new(&mut self.grid_rows).range(1..=40).prefix("fil "));
                    if ui.small_button("Rehacer").on_hover_text("Vuelve a crear la rejilla (pierde pesos y deformaciones)").clicked() {
                        action = Some("regrid");
                    }
                });
                ui.add_space(4.0);
                ui.label(RichText::new(if weighted { "Pesos: sí (la malla se dobla con varios huesos)" } else { "Pesos: no (sigue solo a su hueso)" }).strong());
                ui.label(RichText::new("Marca los huesos que deben doblar la malla:").small());
                let bones: Vec<(u32, String)> = self.doc.bones.iter().map(|b| (b.id, b.name.clone())).collect();
                if self.bind_bones.is_empty() {
                    if let Some(m) = &a.mesh {
                        for ws in &m.weights {
                            for w in ws {
                                if !self.bind_bones.contains(&w.bone) {
                                    self.bind_bones.push(w.bone);
                                }
                            }
                        }
                    }
                    if self.bind_bones.is_empty() {
                        self.bind_bones.push(slot_bone);
                    }
                }
                egui::ScrollArea::vertical().id_salt("bind_scroll").max_height(130.0).show(ui, |ui| {
                    for (id, n) in bones {
                        let mut on = self.bind_bones.contains(&id);
                        if ui.checkbox(&mut on, n).changed() {
                            if on {
                                self.bind_bones.push(id);
                            } else {
                                self.bind_bones.retain(|b| *b != id);
                            }
                        }
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    if ui.button("⚖ Calcular pesos").on_hover_text("Reparte cada vértice entre los huesos marcados según la distancia").clicked() {
                        action = Some("weights");
                    }
                    if ui.add_enabled(weighted, egui::Button::new("Quitar pesos")).clicked() {
                        action = Some("unweight");
                    }
                    if ui.button("Volver a imagen").clicked() {
                        action = Some("to_region");
                    }
                });
            }
            AttachKind::Path => {
                if let Some(p) = a.path.as_mut() {
                    ui.checkbox(&mut p.closed, "Cerrada");
                    ui.label(RichText::new(format!("{} puntos. Herramienta Vértices (M) para moverlos.", p.pts.len())).small());
                    ui.horizontal(|ui| {
                        if ui.button("✚ Punto").clicked() {
                            let last = p.pts.last().copied().unwrap_or(PathPt { p: [0.0, 0.0], hin: [-20.0, 0.0], hout: [20.0, 0.0] });
                            p.pts.push(PathPt { p: [last.p[0] + 60.0, last.p[1]], hin: [-20.0, 0.0], hout: [20.0, 0.0] });
                        }
                        if ui.add_enabled(p.pts.len() > 2, egui::Button::new("➖ Punto")).clicked() {
                            p.pts.pop();
                        }
                    });
                }
            }
            AttachKind::BoundingBox | AttachKind::Clipping => {
                let n = a.mesh.as_ref().map(|m| m.verts.len()).unwrap_or(0);
                ui.label(RichText::new(format!("{n} vértices. Herramienta Vértices (M) para moverlos.")).small());
                if a.kind == AttachKind::Clipping {
                    let slot_opts: Vec<(u32, String)> = self.doc.draw_order.iter().filter_map(|s| self.doc.slot(*s).map(|x| (x.id, x.name.clone()))).collect();
                    let txt = a.end_slot.and_then(|s| self.doc.slot(s)).map(|s| s.name.clone()).unwrap_or_else(|| "(hasta el final)".into());
                    ui.horizontal(|ui| {
                        ui.label("Recorta hasta");
                        egui::ComboBox::from_id_salt("clip_end").selected_text(txt).show_ui(ui, |ui| {
                            ui.selectable_value(&mut a.end_slot, None, "(hasta el final)");
                            for (id, n) in &slot_opts {
                                ui.selectable_value(&mut a.end_slot, Some(*id), n);
                            }
                        });
                    });
                    ui.label(RichText::new("Afecta a los slots dibujados después de este, hasta el elegido.").weak().small());
                }
                if let Some(m) = a.mesh.as_mut() {
                    ui.horizontal(|ui| {
                        if ui.small_button("✚ Vértice").clicked() {
                            let (p0, p1) = (m.verts[m.verts.len() - 1], m.verts[0]);
                            m.verts.push([(p0[0] + p1[0]) / 2.0, (p0[1] + p1[1]) / 2.0]);
                            m.uvs.push([0.0, 0.0]);
                            m.weights.clear();
                            m.hull = m.verts.len();
                        }
                        if m.verts.len() > 3 && ui.small_button("➖ Vértice").clicked() {
                            m.verts.pop();
                            m.uvs.pop();
                            m.weights.clear();
                            m.hull = m.verts.len();
                        }
                    });
                }
            }
            AttachKind::Ellipse | AttachKind::Point => {}
        }
        ui.add_space(4.0);
        if ui.button("🗑 Borrar adjunto").clicked() {
            delete = true;
        }

        if a != orig {
            if let Some(t) = self.doc.find_att_mut(self.skin, sid, name) {
                *t = a.clone();
            }
        }
        if let Some(act) = action {
            match act {
                "to_mesh" | "regrid" => {
                    let mut base = a.clone();
                    if act == "regrid" {
                        // recupera el rectángulo a partir de las uv del contorno
                        base.kind = AttachKind::Image;
                    }
                    let m = crate::mesh::grid(&base, self.grid_cols, self.grid_rows);
                    if let Some(t) = self.doc.find_att_mut(self.skin, sid, name) {
                        if act == "to_mesh" && t.kind == AttachKind::Rect && t.image.is_empty() {
                            t.tint = a.tint;
                        }
                        t.kind = AttachKind::Mesh;
                        t.mesh = Some(m);
                    }
                    for an in &mut self.doc.animations {
                        an.deform_keys.retain(|k| !(k.slot == sid && k.att == name));
                    }
                    self.tool = Tool::Vertices;
                    self.status = "Malla creada: usa la herramienta Vértices (M)".into();
                }
                "weights" => {
                    let bones = self.bind_bones.clone();
                    if let Some(mut m) = a.mesh.clone() {
                        crate::mesh::auto_weights(&self.doc, slot_bone, &mut m, &bones);
                        if let Some(t) = self.doc.find_att_mut(self.skin, sid, name) {
                            t.mesh = Some(m);
                        }
                        self.status = format!("Pesos calculados con {} huesos", bones.len());
                    }
                }
                "unweight" => {
                    if let Some(t) = self.doc.find_att_mut(self.skin, sid, name) {
                        if let Some(m) = t.mesh.as_mut() {
                            m.weights.clear();
                        }
                    }
                }
                "to_region" => {
                    if let Some(t) = self.doc.find_att_mut(self.skin, sid, name) {
                        t.kind = if t.image.is_empty() { AttachKind::Rect } else { AttachKind::Image };
                        t.mesh = None;
                    }
                    for an in &mut self.doc.animations {
                        an.deform_keys.retain(|k| !(k.slot == sid && k.att == name));
                    }
                }
                _ => {}
            }
        }
        if copy_to_skin {
            self.doc.put_attachment(self.skin, sid, name, a.clone());
        }
        if pick_image {
            self.pick_image(sid, name.to_string());
        }
        if rename {
            self.rename_att(sid, name, &new_name);
        }
        if delete {
            let si = owner;
            self.doc.skins[si].entries.retain(|e| !(e.slot == sid && e.name == name));
            if self.doc.att_names(sid).iter().all(|n| n != name) {
                if let Some(s) = self.doc.slot_mut(sid) {
                    if s.attachment.as_deref() == Some(name) {
                        s.attachment = None;
                    }
                }
                for an in &mut self.doc.animations {
                    an.deform_keys.retain(|k| !(k.slot == sid && k.att == name));
                    for k in an.att_keys.iter_mut().filter(|k| k.slot == sid && k.name.as_deref() == Some(name)) {
                        k.name = None;
                    }
                }
            }
            self.sel_att = None;
        }
    }

    // ------------------------------------------------------------------ restricción

    fn constraint_ui(&mut self, ui: &mut egui::Ui, idx: usize) {
        let Some(orig) = self.doc.constraints.get(idx).cloned() else {
            self.sel_constraint = None;
            return;
        };
        let mut c = orig.clone();
        ui.heading(format!("{} Restricción: {}", c.kind.icon(), c.kind.label()));
        let mut delete = false;
        let mut move_by: i32 = 0;
        egui::Grid::new("con_grid").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
            ui.label("Nombre");
            ui.text_edit_singleline(&mut c.name);
            ui.end_row();
            ui.label("Activa");
            ui.checkbox(&mut c.enabled, "");
            ui.end_row();
            ui.label("Orden");
            ui.horizontal(|ui| {
                ui.label(format!("{} de {}", idx + 1, self.doc.constraints.len()));
                if ui.small_button("⬆").on_hover_text("Aplicar antes").clicked() {
                    move_by = -1;
                }
                if ui.small_button("⬇").on_hover_text("Aplicar después").clicked() {
                    move_by = 1;
                }
            });
            ui.end_row();
        });

        if self.mode == Mode::Animate {
            let f = self.key_frame();
            let anim_vals = self.doc.animations.get(self.anim).and_then(|a| a.con_at(&orig.name, f as f32));
            let has_key = self.doc.animations.get(self.anim).map(|a| a.con_keys.iter().any(|k| k.con == orig.name && k.frame == f)).unwrap_or(false);
            let (mut vals, mut bend) = anim_vals.unwrap_or_else(|| orig.anim_values());
            let before = (vals.clone(), bend);
            let labels = orig.value_labels();
            let is_ik = matches!(orig.kind, ConstraintKind::Ik { .. });
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("Animación · fotograma {f}")).color(ACCENT).strong());
                    if key_btn(ui, has_key, egui::Color32::from_rgb(250, 90, 160)) {
                        if let Some(a) = self.doc.animations.get_mut(self.anim) {
                            if has_key {
                                a.con_keys.retain(|k| !(k.con == orig.name && k.frame == f));
                            } else {
                                a.set_con_key(&orig.name, f, vals.clone(), bend);
                            }
                        }
                    }
                });
                ui.label(RichText::new("Cambiar estos valores crea una clave en este fotograma.").weak().small());
                egui::Grid::new("con_anim").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
                    for (i, l) in labels.iter().enumerate() {
                        let Some(v) = vals.get_mut(i) else { continue };
                        ui.label(*l);
                        if is_ik && i >= 2 {
                            let mut on = *v > 0.5;
                            if ui.checkbox(&mut on, "").changed() {
                                *v = if on { 1.0 } else { 0.0 };
                            }
                        } else if matches!(orig.kind, ConstraintKind::Path { .. }) && i < 2 {
                            ui.add(egui::DragValue::new(v).speed(0.005).max_decimals(3));
                        } else {
                            ui.add(egui::Slider::new(v, 0.0..=1.0).max_decimals(2));
                        }
                        ui.end_row();
                    }
                    if is_ik {
                        ui.label("Doblez positivo");
                        ui.checkbox(&mut bend, "");
                        ui.end_row();
                    }
                });
            });
            if (vals.clone(), bend) != before {
                if let Some(a) = self.doc.animations.get_mut(self.anim) {
                    a.set_con_key(&orig.name, f, vals, bend);
                }
            }
            ui.label(RichText::new("Valores base (sin animar):").small());
        }
        ui.add_space(4.0);
        ui.label(RichText::new("Huesos controlados").strong());
        let max_bones = match c.kind {
            ConstraintKind::Ik { .. } => 2,
            ConstraintKind::Physics { .. } => 1,
            _ => 64,
        };
        let mut remove = None;
        for i in 0..c.bones.len() {
            ui.horizontal(|ui| {
                let mut b = c.bones[i];
                bone_combo(ui, ("cb", i), &self.doc, &mut b, &[]);
                c.bones[i] = b;
                if c.bones.len() > 1 && ui.small_button("✖").clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            c.bones.remove(i);
        }
        if c.bones.len() < max_bones && ui.small_button("✚ Añadir hueso").clicked() {
            let next = self.selected.filter(|s| !c.bones.contains(s)).or_else(|| self.doc.bones.iter().map(|b| b.id).find(|b| !c.bones.contains(b)));
            if let Some(b) = next {
                c.bones.push(b);
            }
        }
        if matches!(c.kind, ConstraintKind::Ik { .. }) {
            ui.label(RichText::new("Con 2 huesos: primero el padre (p. ej. muslo) y luego el hijo (pierna).").weak().small());
        }
        ui.add_space(4.0);

        let path_slots: Vec<(u32, String)> = self
            .doc
            .slots
            .iter()
            .filter(|s| self.doc.att_names(s.id).iter().any(|n| self.doc.find_att(self.skin, s.id, n).map(|(_, a)| a.kind == AttachKind::Path).unwrap_or(false)))
            .map(|s| (s.id, s.name.clone()))
            .collect();
        let bones_now = c.bones.clone();
        egui::Grid::new("con_kind").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| match &mut c.kind {
            ConstraintKind::Ik { target, mix, bend_positive, softness, stretch, compress, uniform } => {
                ui.label("Objetivo");
                bone_combo(ui, "ik_t", &self.doc, target, &bones_now);
                ui.end_row();
                mix_slider(ui, "Mezcla", mix);
                ui.label("Suavidad");
                ui.add(egui::DragValue::new(softness).speed(0.5).range(0.0..=1000.0)).on_hover_text("La cadena se estira poco a poco al acercarse a su largo máximo");
                ui.end_row();
                ui.label("Ajustar largo");
                ui.horizontal(|ui| {
                    ui.checkbox(stretch, "Estirar");
                    ui.checkbox(compress, "Comprimir");
                    ui.checkbox(uniform, "Uniforme");
                });
                ui.end_row();
                ui.label("Doblez");
                ui.horizontal(|ui| {
                    ui.radio_value(bend_positive, true, "＋");
                    ui.radio_value(bend_positive, false, "－");
                });
                ui.end_row();
            }
            ConstraintKind::Transform { target, mix_rotate, mix_translate, mix_scale, off_rot, off_x, off_y, off_sx, off_sy, mix_y, mix_scale_y, mix_shear, off_shy } => {
                ui.label("Objetivo");
                bone_combo(ui, "tc_t", &self.doc, target, &bones_now);
                ui.end_row();
                mix_slider(ui, "Mezcla rotación", mix_rotate);
                mix_slider(ui, "Mezcla posición X", mix_translate);
                let mut my = mix_y.unwrap_or(*mix_translate);
                mix_slider(ui, "Mezcla posición Y", &mut my);
                if mix_y.is_some() || my != *mix_translate {
                    *mix_y = Some(my);
                }
                mix_slider(ui, "Mezcla escala X", mix_scale);
                let mut msy = mix_scale_y.unwrap_or(*mix_scale);
                mix_slider(ui, "Mezcla escala Y", &mut msy);
                if mix_scale_y.is_some() || msy != *mix_scale {
                    *mix_scale_y = Some(msy);
                }
                mix_slider(ui, "Mezcla cizalla", mix_shear);
                ui.label("Desfase cizalla");
                ui.add(egui::DragValue::new(off_shy).speed(0.5).suffix("°"));
                ui.end_row();
                ui.label("Desfase rotación");
                ui.add(egui::DragValue::new(off_rot).speed(0.5).suffix("°"));
                ui.end_row();
                ui.label("Desfase posición");
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(off_x).speed(0.5).prefix("x "));
                    ui.add(egui::DragValue::new(off_y).speed(0.5).prefix("y "));
                });
                ui.end_row();
                ui.label("Desfase escala");
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(off_sx).speed(0.01).prefix("x "));
                    ui.add(egui::DragValue::new(off_sy).speed(0.01).prefix("y "));
                });
                ui.end_row();
            }
            ConstraintKind::Path { slot, position, spacing, spacing_mode, rotate_mode, off_rot, mix_rotate, mix_translate } => {
                ui.label("Slot con trayectoria");
                let txt = self.doc.slot(*slot).map(|s| s.name.clone()).unwrap_or_else(|| "(ninguno)".into());
                egui::ComboBox::from_id_salt("pc_slot").selected_text(txt).show_ui(ui, |ui| {
                    for (id, n) in &path_slots {
                        ui.selectable_value(slot, *id, n);
                    }
                });
                ui.end_row();
                ui.label("Posición");
                ui.add(egui::Slider::new(position, 0.0..=1.0).max_decimals(3));
                ui.end_row();
                ui.label("Espaciado");
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("pc_sm")
                        .selected_text(match spacing_mode {
                            SpacingMode::Length => "Largo hueso",
                            SpacingMode::Fixed => "Fijo",
                            SpacingMode::Percent => "Porcentaje",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(spacing_mode, SpacingMode::Length, "Largo hueso");
                            ui.selectable_value(spacing_mode, SpacingMode::Fixed, "Fijo");
                            ui.selectable_value(spacing_mode, SpacingMode::Percent, "Porcentaje");
                        });
                    let speed = if *spacing_mode == SpacingMode::Percent { 0.005 } else { 0.5 };
                    ui.add(egui::DragValue::new(spacing).speed(speed));
                });
                ui.end_row();
                ui.label("Rotación");
                ui.horizontal(|ui| {
                    ui.radio_value(rotate_mode, RotateMode::Tangent, "Tangente");
                    ui.radio_value(rotate_mode, RotateMode::Chain, "Cadena");
                });
                ui.end_row();
                ui.label("Desfase rotación");
                ui.add(egui::DragValue::new(off_rot).speed(0.5).suffix("°"));
                ui.end_row();
                mix_slider(ui, "Mezcla rotación", mix_rotate);
                mix_slider(ui, "Mezcla posición", mix_translate);
            }
            ConstraintKind::Physics { inertia, strength, damping, mass, gravity, wind, mix } => {
                mix_slider(ui, "Inercia", inertia);
                ui.label("Rigidez");
                ui.add(egui::Slider::new(strength, 1.0..=400.0));
                ui.end_row();
                ui.label("Amortiguación");
                ui.add(egui::Slider::new(damping, 0.0..=40.0));
                ui.end_row();
                ui.label("Masa");
                ui.add(egui::Slider::new(mass, 0.1..=10.0));
                ui.end_row();
                ui.label("Gravedad");
                ui.add(egui::Slider::new(gravity, -200.0..=200.0));
                ui.end_row();
                ui.label("Viento");
                ui.add(egui::Slider::new(wind, -200.0..=200.0));
                ui.end_row();
                mix_slider(ui, "Mezcla", mix);
            }
        });
        if matches!(c.kind, ConstraintKind::Physics { .. }) {
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.phys_on, "Simular");
                if ui.button("Reiniciar").clicked() {
                    self.phys.reset();
                }
            });
        }
        ui.add_space(6.0);
        if ui.button("🗑 Borrar restricción").clicked() {
            delete = true;
        }
        if c.name != orig.name {
            if c.name.is_empty() || self.doc.constraints.iter().enumerate().any(|(i, x)| i != idx && x.name == c.name) {
                c.name = orig.name.clone();
            } else {
                for a in &mut self.doc.animations {
                    for k in a.con_keys.iter_mut().filter(|k| k.con == orig.name) {
                        k.con = c.name.clone();
                    }
                }
            }
        }
        if c != orig {
            self.doc.constraints[idx] = c;
        }
        if delete {
            let n = self.doc.constraints.remove(idx).name;
            for a in &mut self.doc.animations {
                a.con_keys.retain(|k| k.con != n);
            }
            self.sel_constraint = None;
            self.phys.reset();
        } else if move_by != 0 {
            let j = idx as i32 + move_by;
            if j >= 0 && (j as usize) < self.doc.constraints.len() {
                self.doc.constraints.swap(idx, j as usize);
                self.sel_constraint = Some(j as usize);
                self.phys.reset();
            }
        }
    }

    // ------------------------------------------------------------------ clave

    fn key_props_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Clave");
        let Some(key) = self.selected_key.clone() else {
            ui.label(RichText::new("Haz clic en un rombo de la línea de tiempo.").weak());
            return;
        };
        let names_ev: Vec<String> = self.doc.events.iter().map(|e| e.name.clone()).collect();
        let slot_name = |doc: &Doc, s: u32| doc.slot(s).map(|s| s.name.clone()).unwrap_or_default();
        let mut found = false;
        let mut delete = false;
        let att_names = match &key {
            KeyRef::Att(s, _) => self.doc.att_names(*s),
            _ => Vec::new(),
        };
        let bname = match &key {
            KeyRef::Bone(b, _, _) => self.doc.bone(*b).map(|b| b.name.clone()).unwrap_or_default(),
            KeyRef::Att(s, _) | KeyRef::Color(s, _) | KeyRef::Deform(s, _, _) => slot_name(&self.doc, *s),
            KeyRef::Event(..) | KeyRef::Draw(_) => String::new(),
            KeyRef::Con(n, _) => n.clone(),
        };
        let (con_labels, con_is_ik): (Vec<&'static str>, bool) = match &key {
            KeyRef::Con(n, _) => self
                .doc
                .constraints
                .iter()
                .find(|c| c.name == *n)
                .map(|c| (c.value_labels().to_vec(), matches!(c.kind, ConstraintKind::Ik { .. })))
                .unwrap_or_default(),
            _ => (Vec::new(), false),
        };
        let slot_names: std::collections::HashMap<u32, String> = self.doc.slots.iter().map(|s| (s.id, s.name.clone())).collect();
        let Some(a) = self.doc.animations.get_mut(self.anim) else { return };
        match &key {
            KeyRef::Bone(bone, ch, frame) => {
                if let Some(k) = a.tracks.iter_mut().find(|t| t.bone == *bone && t.channel == *ch).and_then(|t| t.keys.iter_mut().find(|k| k.frame == *frame)) {
                    found = true;
                    ui.label(format!("{bname} · {} · fotograma {frame}", ch.label()));
                    egui::Grid::new("key_grid").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
                        ui.label("Valor");
                        ui.horizontal(|ui| {
                            ui.add(egui::DragValue::new(&mut k.v[0]).speed(0.5).max_decimals(2));
                            if *ch != Channel::Rotate {
                                ui.add(egui::DragValue::new(&mut k.v[1]).speed(0.5).max_decimals(2));
                            }
                        });
                        ui.end_row();
                    });
                    curve_editor(ui, &mut k.curve);
                }
            }
            KeyRef::Att(slot, frame) => {
                if let Some(k) = a.att_keys.iter_mut().find(|k| k.slot == *slot && k.frame == *frame) {
                    found = true;
                    ui.label(format!("{bname} · adjunto · fotograma {frame}"));
                    let txt = k.name.clone().unwrap_or_else(|| "(ninguno)".into());
                    egui::ComboBox::from_id_salt("attkey").selected_text(txt).show_ui(ui, |ui| {
                        ui.selectable_value(&mut k.name, None, "(ninguno)");
                        for n in &att_names {
                            ui.selectable_value(&mut k.name, Some(n.clone()), n);
                        }
                    });
                }
            }
            KeyRef::Color(slot, frame) => {
                if let Some(k) = a.color_keys.iter_mut().find(|k| k.slot == *slot && k.frame == *frame) {
                    found = true;
                    ui.label(format!("{bname} · color · fotograma {frame}"));
                    ui.color_edit_button_srgba_unmultiplied(&mut k.color);
                    curve_editor(ui, &mut k.curve);
                }
            }
            KeyRef::Deform(slot, att, frame) => {
                if let Some(k) = a.deform_keys.iter_mut().find(|k| k.slot == *slot && k.att == *att && k.frame == *frame) {
                    found = true;
                    ui.label(format!("{bname} / {att} · deformación · fotograma {frame}"));
                    if ui.button("Restablecer vértices").clicked() {
                        for o in &mut k.offsets {
                            *o = [0.0, 0.0];
                        }
                    }
                    curve_editor(ui, &mut k.curve);
                }
            }
            KeyRef::Con(name, frame) => {
                if let Some(k) = a.con_keys.iter_mut().find(|k| k.con == *name && k.frame == *frame) {
                    found = true;
                    ui.label(format!("Restricción «{bname}» · fotograma {frame}"));
                    egui::Grid::new("con_key").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
                        for (i, l) in con_labels.iter().enumerate() {
                            if let Some(v) = k.v.get_mut(i) {
                                ui.label(*l);
                                if i >= 2 && con_is_ik {
                                    let mut on = *v > 0.5;
                                    if ui.checkbox(&mut on, "").changed() {
                                        *v = if on { 1.0 } else { 0.0 };
                                    }
                                } else {
                                    ui.add(egui::DragValue::new(v).speed(0.01).max_decimals(3));
                                }
                                ui.end_row();
                            }
                        }
                        if con_is_ik {
                            ui.label("Doblez positivo");
                            ui.checkbox(&mut k.bend, "");
                            ui.end_row();
                        }
                    });
                    curve_editor(ui, &mut k.curve);
                }
            }
            KeyRef::Draw(frame) => {
                if let Some(k) = a.draw_keys.iter_mut().find(|k| k.frame == *frame) {
                    found = true;
                    ui.label(format!("Orden de dibujo · fotograma {frame}"));
                    ui.label(RichText::new("De atrás hacia delante. Para cambiarlo, usa «Orden de dibujo» en el panel izquierdo en modo animar.").weak().small());
                    egui::ScrollArea::vertical().id_salt("draw_key_list").max_height(160.0).show(ui, |ui| {
                        for s in &k.order {
                            ui.label(format!("▣ {}", slot_names.get(s).cloned().unwrap_or_default()));
                        }
                    });
                }
            }
            KeyRef::Event(frame, name) => {
                if let Some(k) = a.events.iter_mut().find(|k| k.frame == *frame && k.name == *name) {
                    found = true;
                    ui.label(format!("Evento · fotograma {frame}"));
                    let mut new_name = k.name.clone();
                    egui::Grid::new("ev_key").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
                        ui.label("Evento");
                        egui::ComboBox::from_id_salt("evk").selected_text(&new_name).show_ui(ui, |ui| {
                            for n in &names_ev {
                                ui.selectable_value(&mut new_name, n.clone(), n);
                            }
                        });
                        ui.end_row();
                        ui.label("Entero");
                        ui.add(egui::DragValue::new(&mut k.int));
                        ui.end_row();
                        ui.label("Decimal");
                        ui.add(egui::DragValue::new(&mut k.float).speed(0.05));
                        ui.end_row();
                        ui.label("Texto");
                        ui.text_edit_singleline(&mut k.string);
                        ui.end_row();
                    });
                    if new_name != k.name {
                        k.name = new_name.clone();
                        self.selected_key = Some(KeyRef::Event(*frame, new_name));
                    }
                }
            }
        }
        if found {
            delete = ui.button("🗑 Borrar clave (Supr)").clicked();
        } else {
            self.selected_key = None;
        }
        if delete {
            self.delete_selected_key();
        }
    }
}
