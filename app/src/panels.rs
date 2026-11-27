//! Barra superior y panel izquierdo (jerarquía, restricciones, skins, eventos, orden de dibujo).

use crate::app::*;
use crate::draw::*;
use crate::model::*;
use crate::sample::sample_doc;
use egui::{vec2, Align2, Color32, RichText, Sense};

impl EditorApp {
    pub fn top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("menu").frame(egui::Frame::default().fill(PANEL).inner_margin(6.0)).show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("Archivo", |ui| {
                    if ui.button("Nuevo proyecto").clicked() {
                        self.confirm_new = true;
                        ui.close_menu();
                    }
                    if ui.button("Cargar ejemplo").clicked() {
                        self.set_doc(sample_doc());
                        self.mode = Mode::Animate;
                        self.status = "Ejemplo cargado".into();
                        ui.close_menu();
                    }
                    if ui.button("Cargar ejemplo de restricciones").on_hover_text("IK, transformación, trayectoria, física y malla con pesos").clicked() {
                        self.set_doc(crate::sample::demo_doc());
                        self.mode = Mode::Animate;
                        self.pan = vec2(0.0, 0.0);
                        self.zoom = 1.0;
                        self.status = "Ejemplo de restricciones cargado. Pulsa Espacio para reproducir.".into();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Abrir…   Ctrl+O").clicked() {
                        ui.close_menu();
                        self.open_dialog();
                    }
                    if ui.button("Guardar   Ctrl+S").clicked() {
                        ui.close_menu();
                        self.save(false);
                    }
                    if ui.button("Guardar como…").clicked() {
                        ui.close_menu();
                        self.save(true);
                    }
                    ui.separator();
                    if ui.button("Importar de Spine (.json)…").clicked() {
                        ui.close_menu();
                        self.import_dialog();
                    }
                    if ui.button("Exportar a Spine (JSON + atlas)…").clicked() {
                        ui.close_menu();
                        self.export_dialog();
                    }
                    ui.separator();
                    if ui.button("Salir").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.menu_button("Editar", |ui| {
                    if ui.add_enabled(!self.undo.is_empty(), egui::Button::new("Deshacer   Ctrl+Z")).clicked() {
                        self.do_undo();
                        ui.close_menu();
                    }
                    if ui.add_enabled(!self.redo.is_empty(), egui::Button::new("Rehacer   Ctrl+Y")).clicked() {
                        self.do_redo();
                        ui.close_menu();
                    }
                });
                ui.menu_button("Ver", |ui| {
                    ui.checkbox(&mut self.show_bones, "Mostrar huesos");
                    ui.checkbox(&mut self.show_images, "Mostrar imágenes");
                    ui.checkbox(&mut self.show_mesh, "Mostrar mallas");
                    ui.checkbox(&mut self.show_names, "Mostrar nombres");
                    ui.checkbox(&mut self.onion, "Papel cebolla");
                    ui.checkbox(&mut self.graph_view, "Vista de gráfico (G)");
                    ui.separator();
                    ui.checkbox(&mut self.phys_on, "Simular física");
                    if ui.button("Reiniciar física").clicked() {
                        self.phys.reset();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Centrar vista").clicked() {
                        self.pan = vec2(0.0, 150.0);
                        self.zoom = 1.25;
                        ui.close_menu();
                    }
                });
                ui.menu_button("Ayuda", |ui| {
                    ui.label("Atajos:");
                    ui.label("V seleccionar · R rotar · T mover · S escalar · C crear · M vértices");
                    ui.label("Espacio reproducir · ←/→ fotograma · K fijar clave · G gráfico");
                    ui.label("Tab cambiar modo · Supr borrar · rueda zoom");
                    ui.label("Botón derecho o central arrastrando: mover la vista");
                    ui.separator();
                    ui.label("Mayús al mover un asa de trayectoria: romper la simetría");
                });

                ui.add_space(16.0);
                let setup = self.mode == Mode::Setup;
                let b1 = egui::Button::new(RichText::new("  CONFIGURAR  ").strong())
                    .fill(if setup { Color32::from_rgb(70, 110, 160) } else { Color32::from_gray(60) });
                let b2 = egui::Button::new(RichText::new("  ANIMAR  ").strong())
                    .fill(if !setup { Color32::from_rgb(190, 100, 30) } else { Color32::from_gray(60) });
                if ui.add(b1).on_hover_text("Editar la pose base del esqueleto (Tab)").clicked() {
                    self.mode = Mode::Setup;
                    self.playing = false;
                }
                if ui.add(b2).on_hover_text("Crear fotogramas clave (Tab)").clicked() {
                    self.mode = Mode::Animate;
                }

                ui.add_space(14.0);
                for t in Tool::ALL {
                    let on = self.tool == t;
                    let mut tip = format!("Atajo: {}", t.shortcut());
                    if !t.tip().is_empty() {
                        tip = format!("{}\n{tip}", t.tip());
                    }
                    if ui.selectable_label(on, t.label()).on_hover_text(tip).clicked() {
                        self.tool = t;
                    }
                }

                ui.add_space(14.0);
                ui.label("Skin");
                let cur = self.doc.skins.get(self.skin).map(|s| s.name.clone()).unwrap_or_default();
                let mut pick = self.skin;
                egui::ComboBox::from_id_salt("skin_top").selected_text(cur).show_ui(ui, |ui| {
                    for (i, s) in self.doc.skins.iter().enumerate() {
                        ui.selectable_value(&mut pick, i, &s.name);
                    }
                });
                self.skin = pick;

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let name = self
                        .path
                        .as_ref()
                        .and_then(|p| p.file_name())
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| "sin título".into());
                    ui.label(RichText::new(format!("{}{}", name, if self.dirty { " •" } else { "" })).weak());
                });
            });
        });
    }

    pub fn left_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("tree")
            .resizable(true)
            .default_width(250.0)
            .frame(egui::Frame::default().fill(PANEL).inner_margin(8.0))
            .show(ctx, |ui| {
                ui.heading("Jerarquía");
                ui.horizontal_wrapped(|ui| {
                    if ui.button("✚ Hueso").on_hover_text("Añadir un hueso hijo del seleccionado").clicked() {
                        let parent = self.selected;
                        let x = parent.and_then(|p| self.doc.bone(p)).map(|b| b.length).unwrap_or(0.0);
                        let name = self.doc.unique_name("hueso");
                        let id = self.doc.add_bone(parent, name, x, 0.0, 0.0, 40.0);
                        self.selected = Some(id);
                        self.sel_slot = None;
                        self.mode = Mode::Setup;
                    }
                    if ui.add_enabled(self.selected.is_some(), egui::Button::new("✚ Slot")).on_hover_text("Añadir un slot (capa de dibujo) al hueso seleccionado").clicked() {
                        if let Some(b) = self.selected {
                            let base = self.doc.bone(b).map(|b| b.name.clone()).unwrap_or_else(|| "slot".into());
                            let name = self.doc.unique_slot_name(&base);
                            let sid = self.doc.add_slot(b, name.clone());
                            let len = self.doc.bone(b).map(|b| b.length).unwrap_or(30.0).max(30.0);
                            self.doc.put_attachment(0, sid, &name, Attachment::rect(len, len * 0.4, len / 2.0, 0.0, [200, 200, 200, 255]));
                            if let Some(s) = self.doc.slot_mut(sid) {
                                s.attachment = Some(name);
                            }
                            self.sel_slot = Some(sid);
                        }
                    }
                    let can_del = self.selected.is_some() || self.sel_slot.is_some();
                    if ui.add_enabled(can_del, egui::Button::new("🗑 Borrar")).on_hover_text("Borra el slot o el hueso seleccionado").clicked() {
                        if let Some(s) = self.sel_slot.take() {
                            self.doc.delete_slot(s);
                        } else if let Some(b) = self.selected.take() {
                            self.doc.delete_bone(b);
                        }
                        self.fix_selection();
                    }
                });
                ui.separator();
                let rows = self.tree_rows();
                let h = (ui.available_height() * 0.45).max(120.0);
                egui::ScrollArea::vertical().id_salt("tree_scroll").max_height(h).show(ui, |ui| {
                    ui.label(RichText::new("☰ Esqueleto").strong());
                    for (id, depth) in rows {
                        let Some(b) = self.doc.bone(id) else { continue };
                        let name = b.name.clone();
                        let col = c32(b.color);
                        let mut vis = b.visible;
                        let constrained: Vec<&'static str> =
                            self.doc.constraints.iter().filter(|c| c.bones.contains(&id)).map(|c| c.kind.icon()).collect();
                        ui.horizontal(|ui| {
                            ui.add_space(depth as f32 * 12.0 + 2.0);
                            if ui.checkbox(&mut vis, "").on_hover_text("Visible").changed() {
                                if let Some(b) = self.doc.bone_mut(id) {
                                    b.visible = vis;
                                }
                            }
                            let (r, p) = ui.allocate_painter(vec2(10.0, 14.0), Sense::hover());
                            p.circle_filled(r.rect.center(), 4.0, col);
                            let txt = if constrained.is_empty() { name } else { format!("{name}  {}", constrained.join("")) };
                            if ui.selectable_label(self.selected == Some(id) && self.sel_slot.is_none(), txt).clicked() {
                                self.selected = Some(id);
                                self.sel_slot = None;
                                self.sel_constraint = None;
                            }
                        });
                        for sid in self.doc.slots_of(id) {
                            let Some(s) = self.doc.slot(sid) else { continue };
                            let att = self.slot_att(sid).unwrap_or_else(|| "—".into());
                            let txt = format!("▣ {}  ·  {}", s.name, att);
                            ui.horizontal(|ui| {
                                ui.add_space(depth as f32 * 12.0 + 40.0);
                                let r = ui.selectable_label(self.sel_slot == Some(sid), RichText::new(txt).color(Color32::from_rgb(200, 200, 150)));
                                if r.clicked() {
                                    self.sel_slot = Some(sid);
                                    self.selected = Some(id);
                                    self.sel_constraint = None;
                                }
                            });
                        }
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical().id_salt("left_sections").show(ui, |ui| {
                    self.constraints_list(ui);
                    self.skins_list(ui);
                    self.events_list(ui);
                    self.draw_order_list(ui);
                });
            });
    }

    fn constraints_list(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new(format!("Restricciones ({})", self.doc.constraints.len())).default_open(true).show(ui, |ui| {
            let mut pick = None;
            for (i, c) in self.doc.constraints.iter().enumerate() {
                let txt = format!("{} {}  ({})", c.kind.icon(), c.name, c.kind.label());
                let mut rt = RichText::new(txt);
                if !c.enabled {
                    rt = rt.weak();
                }
                if ui.selectable_label(self.sel_constraint == Some(i), rt).clicked() {
                    pick = Some(i);
                }
            }
            if let Some(i) = pick {
                self.sel_constraint = Some(i);
                self.sel_slot = None;
            }
            ui.horizontal_wrapped(|ui| {
                let has = self.selected.is_some();
                let tip = "Selecciona primero el hueso que quieres controlar";
                if ui.add_enabled(has, egui::Button::new("✚ IK")).on_hover_text("Cinemática inversa: el hueso (y su padre) apuntan a un objetivo").on_disabled_hover_text(tip).clicked() {
                    self.add_ik();
                }
                if ui.add_enabled(has, egui::Button::new("✚ Transformación")).on_hover_text("El hueso copia rotación, posición o escala de otro").on_disabled_hover_text(tip).clicked() {
                    self.add_transform();
                }
                if ui.add_enabled(has, egui::Button::new("✚ Trayectoria")).on_hover_text("El hueso y sus hijos siguen una curva").on_disabled_hover_text(tip).clicked() {
                    self.add_path_constraint();
                }
                if ui.add_enabled(has, egui::Button::new("✚ Física")).on_hover_text("El hueso se balancea con inercia, muelle y gravedad").on_disabled_hover_text(tip).clicked() {
                    self.add_physics();
                }
            });
        });
    }

    fn add_constraint(&mut self, c: Constraint) {
        self.doc.constraints.push(c);
        self.sel_constraint = Some(self.doc.constraints.len() - 1);
        self.sel_slot = None;
        self.phys.reset();
    }

    fn add_ik(&mut self) {
        let Some(sel) = self.selected else { return };
        let world = self.doc.world(None, 0.0);
        let Some(w) = world.get(&sel).copied() else { return };
        let len = self.doc.bone(sel).map(|b| b.length).unwrap_or(0.0);
        let tip = w.apply(len.max(10.0), 0.0);
        let parent = self.doc.bone(sel).and_then(|b| b.parent);
        let root = self.doc.children(None).first().copied();
        let rw = root.and_then(|r| world.get(&r).copied()).unwrap_or(Aff::IDENTITY);
        let lp = rw.inv_point(tip[0], tip[1]);
        let tname = self.doc.unique_name("objetivo_ik");
        let target = self.doc.add_bone(root, tname, lp[0], lp[1], 0.0, 0.0);
        if let Some(b) = self.doc.bone_mut(target) {
            b.color = [250, 90, 160, 255];
        }
        let bones = match parent {
            Some(p) if Some(p) != root => vec![p, sel],
            _ => vec![sel],
        };
        // dirección de doblez que respeta la pose actual
        let bend_positive = if bones.len() == 2 {
            let p0 = world[&bones[0]].origin();
            let c0 = world[&sel].origin();
            let cross = (c0[0] - p0[0]) * (tip[1] - p0[1]) - (c0[1] - p0[1]) * (tip[0] - p0[0]);
            cross > 0.0
        } else {
            true
        };
        let name = self.doc.unique_constraint_name("ik");
        self.add_constraint(Constraint { name, enabled: true, bones, kind: ConstraintKind::Ik { target, mix: 1.0, bend_positive, softness: 0.0, stretch: false, compress: false, uniform: false } });
        self.selected = Some(target);
        self.tool = Tool::Translate;
        self.status = "IK creada: mueve el hueso «objetivo_ik» (herramienta Mover)".into();
    }

    fn add_transform(&mut self) {
        let Some(sel) = self.selected else { return };
        let world = self.doc.world(None, 0.0);
        let target = self.doc.bone(sel).and_then(|b| b.parent).or_else(|| self.doc.bones.iter().map(|b| b.id).find(|id| *id != sel));
        let Some(target) = target else { return };
        let (Some(bw), Some(tw)) = (world.get(&sel).copied(), world.get(&target).copied()) else { return };
        let off = tw.inv_point(bw.tx, bw.ty);
        let name = self.doc.unique_constraint_name("transformación");
        self.add_constraint(Constraint {
            name,
            enabled: true,
            bones: vec![sel],
            kind: ConstraintKind::Transform {
                target,
                mix_rotate: 1.0,
                mix_translate: 1.0,
                mix_scale: 0.0,
                off_rot: wrap_deg(bw.angle_deg() - tw.angle_deg()),
                off_x: off[0],
                off_y: off[1],
                off_sx: 0.0,
                off_sy: 0.0,
                mix_y: None,
                mix_scale_y: None,
                mix_shear: 0.0,
                off_shy: 0.0,
            },
        });
        self.status = "Restricción de transformación creada: elige el hueso objetivo en Propiedades".into();
    }

    fn add_path_constraint(&mut self) {
        let Some(sel) = self.selected else { return };
        let world = self.doc.world(None, 0.0);
        let Some(w) = world.get(&sel).copied() else { return };
        // cadena: el hueso y su primer hijo, sucesivamente
        let mut chain = vec![sel];
        let mut cur = sel;
        let mut total = self.doc.bone(sel).map(|b| b.length).unwrap_or(0.0);
        while let Some(c) = self.doc.children(Some(cur)).first().copied() {
            chain.push(c);
            total += self.doc.bone(c).map(|b| b.length).unwrap_or(0.0);
            cur = c;
            if chain.len() > 64 {
                break;
            }
        }
        let root = self.doc.children(None).first().copied();
        let rw = root.and_then(|r| world.get(&r).copied()).unwrap_or(Aff::IDENTITY);
        let lp = rw.inv_point(w.tx, w.ty);
        let rot = wrap_deg(w.angle_deg() - rw.angle_deg());
        let bname = self.doc.unique_name("trayectoria");
        let pb = self.doc.add_bone(root, bname.clone(), lp[0], lp[1], rot, 0.0);
        let sname = self.doc.unique_slot_name(&bname);
        let sid = self.doc.add_slot(pb, sname.clone());
        self.doc.put_attachment(0, sid, &sname, Attachment::path_line(total.max(80.0) * 1.2));
        if let Some(s) = self.doc.slot_mut(sid) {
            s.attachment = Some(sname);
        }
        let name = self.doc.unique_constraint_name("trayectoria");
        self.add_constraint(Constraint {
            name,
            enabled: true,
            bones: chain,
            kind: ConstraintKind::Path {
                slot: sid,
                position: 0.0,
                spacing: 0.0,
                spacing_mode: SpacingMode::Length,
                rotate_mode: RotateMode::Tangent,
                off_rot: 0.0,
                mix_rotate: 1.0,
                mix_translate: 1.0,
            },
        });
        self.tool = Tool::Vertices;
        self.sel_slot = Some(sid);
        self.mode = Mode::Setup;
        self.status = "Trayectoria creada: arrastra sus puntos y asas con la herramienta Vértices".into();
    }

    fn add_physics(&mut self) {
        let Some(sel) = self.selected else { return };
        let name = self.doc.unique_constraint_name("física");
        self.add_constraint(Constraint {
            name,
            enabled: true,
            bones: vec![sel],
            kind: ConstraintKind::Physics { inertia: 0.6, strength: 80.0, damping: 6.0, mass: 1.0, gravity: 0.0, wind: 0.0, mix: 1.0 },
        });
        self.status = "Física añadida: reproduce la animación o mueve un hueso padre para verla".into();
    }

    fn skins_list(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new(format!("Skins ({})", self.doc.skins.len())).default_open(false).show(ui, |ui| {
            ui.label(RichText::new("Una skin cambia los adjuntos (ropa, colores…) del mismo esqueleto.").weak().small());
            let mut pick = None;
            for (i, s) in self.doc.skins.iter().enumerate() {
                let n = s.entries.len();
                if ui.selectable_label(self.skin == i, format!("👕 {}  ({n} adjuntos)", s.name)).clicked() {
                    pick = Some(i);
                }
            }
            if let Some(i) = pick {
                self.skin = i;
            }
            ui.horizontal_wrapped(|ui| {
                if ui.button("✚ Nueva").clicked() {
                    let name = unique("skin", |n| self.doc.skins.iter().any(|s| s.name == n));
                    self.doc.skins.push(Skin { name, entries: Vec::new() });
                    self.skin = self.doc.skins.len() - 1;
                }
                if ui.button("⎘ Duplicar").clicked() {
                    if let Some(s) = self.doc.skins.get(self.skin).cloned() {
                        let base = format!("{} copia", s.name);
                        let name = unique(&base, |n| self.doc.skins.iter().any(|s| s.name == n));
                        self.doc.skins.push(Skin { name, entries: s.entries });
                        self.skin = self.doc.skins.len() - 1;
                    }
                }
                if ui.add_enabled(self.skin > 0, egui::Button::new("🗑")).on_hover_text("Borrar skin (la predeterminada no se puede borrar)").clicked() {
                    self.doc.skins.remove(self.skin);
                    self.skin = 0;
                }
            });
            if self.skin > 0 {
                if let Some(s) = self.doc.skins.get_mut(self.skin) {
                    ui.horizontal(|ui| {
                        ui.label("Nombre");
                        ui.text_edit_singleline(&mut s.name);
                    });
                }
                ui.label(RichText::new("Los adjuntos que no estén en esta skin se toman de la predeterminada.").weak().small());
            }
        });
    }

    fn events_list(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new(format!("Eventos ({})", self.doc.events.len())).default_open(false).show(ui, |ui| {
            ui.label(RichText::new("Se disparan al reproducir (sonidos, pasos, golpes…).").weak().small());
            let mut remove = None;
            let mut renames: Vec<(String, String)> = Vec::new();
            for (i, e) in self.doc.events.iter_mut().enumerate() {
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    egui::Grid::new(("evgrid", i)).num_columns(2).spacing([6.0, 3.0]).show(ui, |ui| {
                        ui.label("Nombre");
                        let old = e.name.clone();
                        ui.horizontal(|ui| {
                            if ui.add(egui::TextEdit::singleline(&mut e.name).desired_width(110.0)).changed() {
                                renames.push((old, e.name.clone()));
                            }
                            if ui.small_button("🗑").clicked() {
                                remove = Some(i);
                            }
                        });
                        ui.end_row();
                        ui.label("Entero");
                        ui.add(egui::DragValue::new(&mut e.int));
                        ui.end_row();
                        ui.label("Decimal");
                        ui.add(egui::DragValue::new(&mut e.float).speed(0.05));
                        ui.end_row();
                        ui.label("Texto");
                        ui.add(egui::TextEdit::singleline(&mut e.string).desired_width(130.0));
                        ui.end_row();
                        ui.label("Audio");
                        ui.add(egui::TextEdit::singleline(&mut e.audio).desired_width(130.0).hint_text("archivo.ogg"));
                        ui.end_row();
                    });
                });
            }
            for (old, new) in renames {
                for a in &mut self.doc.animations {
                    for k in a.events.iter_mut().filter(|k| k.name == old) {
                        k.name = new.clone();
                    }
                }
            }
            if let Some(i) = remove {
                let n = self.doc.events.remove(i).name;
                for a in &mut self.doc.animations {
                    a.events.retain(|k| k.name != n);
                }
            }
            if ui.button("✚ Nuevo evento").clicked() {
                let name = unique("evento", |n| self.doc.events.iter().any(|e| e.name == n));
                self.doc.events.push(EventDef { name, ..Default::default() });
            }
        });
    }

    fn draw_order_list(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("Orden de dibujo").default_open(false).show(ui, |ui| {
            ui.label(RichText::new("Lo de abajo se dibuja encima").weak().small());
            let animating = self.mode == Mode::Animate;
            if animating {
                ui.label(RichText::new("Modo animar: cambiar el orden crea una clave").color(ACCENT).small());
            }
            let order = self.cur_draw_order();
            let mut new_order: Option<Vec<u32>> = None;
            for (i, id) in order.iter().enumerate() {
                let Some(s) = self.doc.slot(*id) else { continue };
                let name = s.name.clone();
                ui.horizontal(|ui| {
                    if ui.small_button("⬆").clicked() && i > 0 {
                        let mut o = order.clone();
                        o.swap(i, i - 1);
                        new_order = Some(o);
                    }
                    if ui.small_button("⬇").clicked() && i + 1 < order.len() {
                        let mut o = order.clone();
                        o.swap(i, i + 1);
                        new_order = Some(o);
                    }
                    if ui.selectable_label(self.sel_slot == Some(*id), format!("▣ {name}")).clicked() {
                        self.sel_slot = Some(*id);
                        self.selected = self.doc.slot(*id).map(|s| s.bone);
                        self.sel_constraint = None;
                    }
                });
            }
            if let Some(o) = new_order {
                if animating {
                    let f = self.key_frame();
                    if let Some(a) = self.doc.animations.get_mut(self.anim) {
                        a.set_draw_key(f, o);
                    }
                } else {
                    self.doc.draw_order = o;
                }
            }
        });
    }

    pub fn new_dialog(&mut self, ctx: &egui::Context) {
        if !self.confirm_new {
            return;
        }
        egui::Window::new("Nuevo proyecto")
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.label("¿Empezar un proyecto vacío? Los cambios sin guardar se perderán.");
                ui.horizontal(|ui| {
                    if ui.button("Sí, nuevo").clicked() {
                        self.set_doc(Doc::empty());
                        self.mode = Mode::Setup;
                        self.tool = Tool::Create;
                        self.selected = Some(0);
                        self.status = "Proyecto nuevo. Arrastra en el lienzo para crear huesos.".into();
                        self.confirm_new = false;
                    }
                    if ui.button("Cancelar").clicked() {
                        self.confirm_new = false;
                    }
                });
            });
    }
}
