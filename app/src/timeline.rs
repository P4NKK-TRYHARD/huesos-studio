//! Línea de tiempo: controles de reproducción, hoja de claves y vista de gráfico.

use crate::app::*;
use crate::draw::*;
use crate::model::*;
use egui::{pos2, vec2, Align2, Color32, FontId, Pos2, Rect, RichText, Sense, Stroke};

#[derive(Clone, Debug)]
enum Row {
    Events,
    Bone(u32, usize),
    Channel(u32, Channel, usize),
    Slot(u32, usize),
    SlotAtt(u32, usize),
    SlotColor(u32, usize),
    Deform(u32, String, usize),
    Con(usize),
    Draw,
}

const EV_COL: Color32 = Color32::from_rgb(255, 215, 90);
const ATT_COL: Color32 = Color32::from_rgb(220, 160, 255);
const COLOR_COL: Color32 = Color32::from_rgb(230, 200, 90);
const DEF_COL: Color32 = Color32::from_rgb(255, 130, 200);
const CON_COL: Color32 = Color32::from_rgb(250, 90, 160);
const DRAW_COL: Color32 = Color32::from_rgb(140, 220, 220);

impl EditorApp {
    pub fn timeline(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("timeline")
            .resizable(true)
            .default_height(260.0)
            .min_height(140.0)
            .frame(egui::Frame::default().fill(PANEL).inner_margin(6.0))
            .show(ctx, |ui| {
                let dur = self.doc.animations.get(self.anim).map(|a| a.duration).unwrap_or(30);
                let fps = self.doc.animations.get(self.anim).map(|a| a.fps).unwrap_or(30.0);
                let aname = self.doc.animations.get(self.anim).map(|a| a.name.clone()).unwrap_or_default();
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(format!("Línea de tiempo — {aname}")).strong());
                    ui.add_space(8.0);
                    if ui.button("⏮").on_hover_text("Inicio").clicked() {
                        self.frame = 0.0;
                        self.playing = false;
                        self.phys.reset();
                    }
                    if ui.button("⏪").on_hover_text("Clave anterior").clicked() {
                        self.jump_key(-1);
                    }
                    if ui.button("◀").on_hover_text("Fotograma anterior").clicked() {
                        self.step_frame(-1.0);
                    }
                    let play_txt = if self.playing { "⏸ Pausa" } else { "▶ Reproducir" };
                    if ui.add(egui::Button::new(play_txt).fill(Color32::from_rgb(60, 95, 60))).clicked() {
                        self.playing = !self.playing;
                        self.mode = Mode::Animate;
                    }
                    if ui.button("▶|").on_hover_text("Fotograma siguiente").clicked() {
                        self.step_frame(1.0);
                    }
                    if ui.button("⏩").on_hover_text("Clave siguiente").clicked() {
                        self.jump_key(1);
                    }
                    if ui.button("⏭").on_hover_text("Final").clicked() {
                        self.frame = dur as f32;
                        self.playing = false;
                    }
                    ui.toggle_value(&mut self.looping, "🔁 Repetir");
                    ui.label("Velocidad");
                    ui.add(egui::Slider::new(&mut self.speed, 0.1..=3.0).max_decimals(1).suffix("×"));
                    let mut f = self.frame.round() as i32;
                    ui.label("Fotograma");
                    if ui.add(egui::DragValue::new(&mut f).range(0..=dur as i32)).changed() {
                        self.frame = f as f32;
                        self.playing = false;
                    }
                    ui.label(RichText::new(format!("/ {dur}   {:.2} s", self.frame / fps.max(1.0))).weak());
                    ui.checkbox(&mut self.onion, "Papel cebolla");
                    ui.toggle_value(&mut self.graph_view, "📈 Gráfico").on_hover_text("Ver y editar las curvas del hueso seleccionado (G)");
                    ui.menu_button("✚ Evento", |ui| {
                        if self.doc.events.is_empty() {
                            ui.label("Primero crea un evento en el panel izquierdo (Eventos).");
                            if ui.button("Crear «evento»").clicked() {
                                self.doc.events.push(EventDef { name: "evento".into(), ..Default::default() });
                            }
                        }
                        let defs = self.doc.events.clone();
                        for e in defs {
                            if ui.button(format!("⚡ {}", e.name)).clicked() {
                                let fr = self.key_frame();
                                if let Some(a) = self.doc.animations.get_mut(self.anim) {
                                    a.events.push(EventKey { frame: fr, name: e.name.clone(), int: e.int, float: e.float, string: e.string.clone() });
                                    a.events.sort_by_key(|k| k.frame);
                                    a.grow(fr);
                                }
                                self.selected_key = Some(KeyRef::Event(fr, e.name.clone()));
                                self.mode = Mode::Animate;
                                ui.close_menu();
                            }
                        }
                    });
                });
                ui.add_space(2.0);
                if self.graph_view {
                    self.graph(ui, dur);
                } else {
                    self.dopesheet(ui, dur);
                }
            });
    }

    fn rows(&self) -> Vec<Row> {
        let anim = self.doc.animations.get(self.anim);
        let mut rows = Vec::new();
        if !self.doc.events.is_empty() || anim.map(|a| !a.events.is_empty()).unwrap_or(false) {
            rows.push(Row::Events);
        }
        if anim.map(|a| !a.draw_keys.is_empty()).unwrap_or(false) {
            rows.push(Row::Draw);
        }
        for (i, c) in self.doc.constraints.iter().enumerate() {
            if self.sel_constraint == Some(i) || anim.map(|a| a.con_keys.iter().any(|k| k.con == c.name)).unwrap_or(false) {
                rows.push(Row::Con(i));
            }
        }
        for (id, depth) in self.tree_rows() {
            rows.push(Row::Bone(id, depth));
            if self.selected == Some(id) && self.sel_slot.is_none() {
                for ch in Channel::ALL {
                    rows.push(Row::Channel(id, ch, depth + 1));
                }
            }
            for sid in self.doc.slots_of(id) {
                let keyed = anim
                    .map(|a| a.att_keys.iter().any(|k| k.slot == sid) || a.color_keys.iter().any(|k| k.slot == sid) || a.deform_keys.iter().any(|k| k.slot == sid))
                    .unwrap_or(false);
                let sel = self.sel_slot == Some(sid);
                if keyed || sel {
                    rows.push(Row::Slot(sid, depth + 1));
                }
                if sel {
                    rows.push(Row::SlotAtt(sid, depth + 2));
                    rows.push(Row::SlotColor(sid, depth + 2));
                    let mut atts: Vec<String> = anim.map(|a| a.deform_keys.iter().filter(|k| k.slot == sid).map(|k| k.att.clone()).collect()).unwrap_or_default();
                    if let Some(n) = self.slot_att(sid) {
                        if self.doc.find_att(self.skin, sid, &n).map(|(_, a)| a.kind == AttachKind::Mesh).unwrap_or(false) {
                            atts.push(n);
                        }
                    }
                    atts.sort();
                    atts.dedup();
                    for n in atts {
                        rows.push(Row::Deform(sid, n, depth + 2));
                    }
                }
            }
        }
        rows
    }

    /// Claves dibujadas en una fila: (fotograma, color, referencia seleccionable).
    fn row_keys(&self, a: &Animation, row: &Row) -> Vec<(u32, Color32, Option<KeyRef>)> {
        match row {
            Row::Events => a.events.iter().map(|k| (k.frame, EV_COL, Some(KeyRef::Event(k.frame, k.name.clone())))).collect(),
            Row::Bone(id, _) => {
                let mut f: Vec<u32> = a.tracks.iter().filter(|t| t.bone == *id).flat_map(|t| t.keys.iter().map(|k| k.frame)).collect();
                f.sort();
                f.dedup();
                f.into_iter().map(|f| (f, Color32::from_gray(200), None)).collect()
            }
            Row::Channel(id, ch, _) => a
                .track(*id, *ch)
                .map(|t| t.keys.iter().map(|k| (k.frame, ch_color(*ch), Some(KeyRef::Bone(*id, *ch, k.frame)))).collect())
                .unwrap_or_default(),
            Row::Slot(sid, _) => {
                let mut f: Vec<u32> = a.att_keys.iter().filter(|k| k.slot == *sid).map(|k| k.frame).collect();
                f.extend(a.color_keys.iter().filter(|k| k.slot == *sid).map(|k| k.frame));
                f.extend(a.deform_keys.iter().filter(|k| k.slot == *sid).map(|k| k.frame));
                f.sort();
                f.dedup();
                f.into_iter().map(|f| (f, Color32::from_gray(190), None)).collect()
            }
            Row::SlotAtt(sid, _) => a.att_keys.iter().filter(|k| k.slot == *sid).map(|k| (k.frame, ATT_COL, Some(KeyRef::Att(*sid, k.frame)))).collect(),
            Row::SlotColor(sid, _) => a
                .color_keys
                .iter()
                .filter(|k| k.slot == *sid)
                .map(|k| (k.frame, c32([k.color[0], k.color[1], k.color[2], 255]), Some(KeyRef::Color(*sid, k.frame))))
                .collect(),
            Row::Deform(sid, n, _) => a
                .deform_keys
                .iter()
                .filter(|k| k.slot == *sid && k.att == *n)
                .map(|k| (k.frame, DEF_COL, Some(KeyRef::Deform(*sid, n.clone(), k.frame))))
                .collect(),
            Row::Con(i) => {
                let Some(c) = self.doc.constraints.get(*i) else { return Vec::new() };
                a.con_keys.iter().filter(|k| k.con == c.name).map(|k| (k.frame, CON_COL, Some(KeyRef::Con(c.name.clone(), k.frame)))).collect()
            }
            Row::Draw => a.draw_keys.iter().map(|k| (k.frame, DRAW_COL, Some(KeyRef::Draw(k.frame)))).collect(),
        }
    }

    fn ruler(&mut self, ui: &mut egui::Ui, dur: u32, label_w: f32, fw: f32) -> u32 {
        let ruler_h = 22.0;
        let avail = ui.available_size();
        let (rr, rp) = ui.allocate_painter(vec2(avail.x, ruler_h), Sense::click_and_drag());
        let rect = rr.rect;
        let gx0 = rect.left() + label_w;
        rp.rect_filled(rect, 0.0f32, Color32::from_gray(34));
        let title = if self.graph_view { "Gráfico" } else { "Huesos y slots" };
        rp.text(pos2(rect.left() + 8.0, rect.center().y), Align2::LEFT_CENTER, title, FontId::proportional(11.5), Color32::from_gray(150));
        let step = [1u32, 2, 5, 10, 20, 50, 100, 200, 500].into_iter().find(|s| *s as f32 * fw >= 36.0).unwrap_or(1000);
        for f in 0..=dur {
            let x = gx0 + f as f32 * fw + fw * 0.5;
            if x > rect.right() {
                break;
            }
            let major = f % step == 0;
            let h = if major { 10.0 } else { 4.0 };
            if major || fw >= 6.0 {
                rp.line_segment([pos2(x, rect.bottom() - h), pos2(x, rect.bottom())], Stroke::new(1.0f32, Color32::from_gray(110)));
            }
            if major {
                rp.text(pos2(x + 2.0, rect.top() + 1.0), Align2::LEFT_TOP, f.to_string(), FontId::proportional(11.0), Color32::from_gray(190));
            }
        }
        let px = gx0 + self.frame * fw + fw * 0.5;
        rp.rect_filled(Rect::from_center_size(pos2(px, rect.center().y), vec2(fw.max(14.0) + 8.0, ruler_h - 2.0)), 3.0, ACCENT);
        rp.text(pos2(px, rect.center().y), Align2::CENTER_CENTER, format!("{}", self.frame.round()), FontId::proportional(11.0), Color32::BLACK);
        if rr.dragged() || rr.clicked() {
            if let Some(p) = rr.interact_pointer_pos() {
                if p.x >= gx0 {
                    self.frame = ((p.x - gx0) / fw - 0.5).round().clamp(0.0, dur as f32);
                    self.playing = false;
                    self.mode = Mode::Animate;
                }
            }
        }
        step
    }

    fn dopesheet(&mut self, ui: &mut egui::Ui, dur: u32) {
        let label_w = 190.0;
        let row_h = 20.0;
        let avail = ui.available_size();
        let grid_w = (avail.x - label_w - 10.0).max(100.0);
        let fw = (grid_w / (dur as f32 + 1.0)).clamp(3.0, 40.0);
        let rows = self.rows();
        let step = self.ruler(ui, dur, label_w, fw);

        let anim = self.doc.animations.get(self.anim).cloned();
        egui::ScrollArea::vertical().id_salt("dope_scroll").auto_shrink([false, false]).show(ui, |ui| {
            let total_h = (rows.len() as f32 * row_h).max(ui.available_height());
            let (resp, p) = ui.allocate_painter(vec2(avail.x, total_h), Sense::click_and_drag());
            let r = resp.rect;
            let gx0 = r.left() + label_w;
            p.rect_filled(r, 0.0f32, Color32::from_gray(42));
            let end_x = gx0 + (dur as f32 + 1.0) * fw;
            if end_x < r.right() {
                p.rect_filled(Rect::from_min_max(pos2(end_x, r.top()), r.right_bottom()), 0.0, Color32::from_gray(30));
            }
            let mut keys_per_row: Vec<Vec<(u32, Color32, Option<KeyRef>)>> = Vec::new();
            for (i, row) in rows.iter().enumerate() {
                let y = r.top() + i as f32 * row_h;
                let rr = Rect::from_min_size(pos2(r.left(), y), vec2(r.width(), row_h));
                let cy = y + row_h / 2.0;
                let (sel, sub) = match row {
                    Row::Bone(id, _) => (self.selected == Some(*id) && self.sel_slot.is_none(), false),
                    Row::Slot(s, _) => (self.sel_slot == Some(*s), false),
                    Row::Events | Row::Draw => (false, false),
                    Row::Con(i) => (self.sel_constraint == Some(*i), false),
                    _ => (false, true),
                };
                let bg = if sub {
                    Color32::from_gray(48)
                } else if sel {
                    Color32::from_rgb(45, 70, 95)
                } else if i % 2 == 0 {
                    Color32::from_gray(44)
                } else {
                    Color32::from_gray(40)
                };
                p.rect_filled(rr, 0.0f32, bg);
                let font = FontId::proportional(12.0);
                match row {
                    Row::Events => {
                        p.text(pos2(r.left() + 8.0, cy), Align2::LEFT_CENTER, "⚡ Eventos", font, EV_COL);
                    }
                    Row::Bone(id, depth) => {
                        if let Some(b) = self.doc.bone(*id) {
                            let tx = r.left() + 8.0 + *depth as f32 * 10.0;
                            p.circle_filled(pos2(tx + 4.0, cy), 4.0, c32(b.color));
                            p.text(pos2(tx + 12.0, cy), Align2::LEFT_CENTER, &b.name, font, Color32::from_gray(225));
                        }
                    }
                    Row::Channel(_, c, depth) => {
                        p.text(pos2(r.left() + 20.0 + *depth as f32 * 10.0, cy), Align2::LEFT_CENTER, c.label(), FontId::proportional(11.5), ch_color(*c));
                    }
                    Row::Slot(s, depth) => {
                        let n = self.doc.slot(*s).map(|s| s.name.clone()).unwrap_or_default();
                        p.text(pos2(r.left() + 12.0 + *depth as f32 * 10.0, cy), Align2::LEFT_CENTER, format!("▣ {n}"), font, Color32::from_rgb(200, 200, 150));
                    }
                    Row::SlotAtt(_, depth) => {
                        p.text(pos2(r.left() + 12.0 + *depth as f32 * 10.0, cy), Align2::LEFT_CENTER, "Adjunto", FontId::proportional(11.5), ATT_COL);
                    }
                    Row::SlotColor(_, depth) => {
                        p.text(pos2(r.left() + 12.0 + *depth as f32 * 10.0, cy), Align2::LEFT_CENTER, "Color", FontId::proportional(11.5), COLOR_COL);
                    }
                    Row::Deform(_, n, depth) => {
                        p.text(pos2(r.left() + 12.0 + *depth as f32 * 10.0, cy), Align2::LEFT_CENTER, format!("Deformar {n}"), FontId::proportional(11.5), DEF_COL);
                    }
                    Row::Con(i) => {
                        let (ic, n) = self.doc.constraints.get(*i).map(|c| (c.kind.icon(), c.name.clone())).unwrap_or(("", String::new()));
                        p.text(pos2(r.left() + 8.0, cy), Align2::LEFT_CENTER, format!("{ic} {n}"), font, CON_COL);
                    }
                    Row::Draw => {
                        p.text(pos2(r.left() + 8.0, cy), Align2::LEFT_CENTER, "☰ Orden de dibujo", font, DRAW_COL);
                    }
                }
                let keys = anim.as_ref().map(|a| self.row_keys(a, row)).unwrap_or_default();
                for (ki, (f, col, kr)) in keys.iter().enumerate() {
                    let x = gx0 + *f as f32 * fw + fw * 0.5;
                    if kr.is_some() {
                        if let Some((nf, _, _)) = keys.get(ki + 1) {
                            let nx = gx0 + *nf as f32 * fw + fw * 0.5;
                            if !matches!(row, Row::Events | Row::SlotAtt(..) | Row::Draw) {
                                p.line_segment([pos2(x, cy), pos2(nx, cy)], Stroke::new(2.0f32, col.gamma_multiply(0.35)));
                            }
                        }
                    }
                    let is_sel = kr.is_some() && self.selected_key == *kr;
                    let size = if kr.is_some() { 6.0 } else { 5.0 };
                    diamond(&p, pos2(x, cy), size, if is_sel { Color32::YELLOW } else { *col }, is_sel);
                    if let (Row::Events, Some(a)) = (row, anim.as_ref()) {
                        if let Some(e) = a.events.iter().find(|e| e.frame == *f) {
                            p.text(pos2(x + 8.0, cy), Align2::LEFT_CENTER, &e.name, FontId::proportional(10.5), EV_COL.gamma_multiply(0.8));
                        }
                    }
                }
                keys_per_row.push(keys);
            }
            let mut f = 0;
            while f <= dur {
                let x = gx0 + f as f32 * fw + fw * 0.5;
                p.line_segment([pos2(x, r.top()), pos2(x, r.bottom())], Stroke::new(1.0f32, Color32::from_white_alpha(10)));
                f += step;
            }
            p.line_segment([pos2(gx0, r.top()), pos2(gx0, r.bottom())], Stroke::new(1.0f32, Color32::from_gray(70)));
            let px = gx0 + self.frame * fw + fw * 0.5;
            p.line_segment([pos2(px, r.top()), pos2(px, r.bottom())], Stroke::new(1.5f32, ACCENT));

            // interacción
            let hit = |pos: Pos2| -> Option<(usize, f32)> {
                let i = ((pos.y - r.top()) / row_h).floor();
                if i < 0.0 || i as usize >= rows.len() {
                    return None;
                }
                Some((i as usize, (pos.x - gx0) / fw - 0.5))
            };
            if resp.drag_started() || resp.clicked() {
                let pos = ui.input(|i| i.pointer.press_origin()).or(resp.interact_pointer_pos());
                if let Some((ri, fpos)) = pos.and_then(hit) {
                    let pos = pos.unwrap();
                    match &rows[ri] {
                        Row::Bone(id, _) | Row::Channel(id, _, _) => {
                            self.selected = Some(*id);
                            self.sel_slot = None;
                        }
                        Row::Slot(s, _) | Row::SlotAtt(s, _) | Row::SlotColor(s, _) | Row::Deform(s, _, _) => {
                            self.sel_slot = Some(*s);
                            self.selected = self.doc.slot(*s).map(|s| s.bone);
                        }
                        Row::Events | Row::Draw => {}
                        Row::Con(i) => {
                            self.sel_constraint = Some(*i);
                            self.sel_slot = None;
                        }
                    }
                    if !matches!(rows[ri], Row::Con(_)) {
                        self.sel_constraint = None;
                    }
                    if pos.x >= gx0 {
                        let grabbed = keys_per_row[ri]
                            .iter()
                            .filter_map(|(f, _, k)| k.clone().map(|k| (*f, k)))
                            .find(|(f, _)| ((*f as f32 - fpos) * fw).abs() <= 7.0);
                        if let Some((f, k)) = grabbed {
                            self.selected_key = Some(k.clone());
                            self.frame = f as f32;
                            self.drag = Drag::Key(k);
                        } else {
                            self.selected_key = None;
                            self.frame = fpos.round().clamp(0.0, dur as f32);
                        }
                        self.playing = false;
                        self.mode = Mode::Animate;
                    }
                }
            }
            if resp.dragged() {
                if let (Drag::Key(k), Some(pos)) = (&self.drag, resp.interact_pointer_pos()) {
                    let k = k.clone();
                    let nf = ((pos.x - gx0) / fw - 0.5).round().clamp(0.0, 100000.0) as u32;
                    if nf != k.frame() {
                        if let Some(nk) = self.move_key(&k, nf) {
                            self.drag = Drag::Key(nk.clone());
                            self.selected_key = Some(nk);
                            self.frame = nf as f32;
                        }
                    }
                }
            }
            if resp.drag_stopped() && matches!(self.drag, Drag::Key(_)) {
                self.drag = Drag::None;
            }
        });
    }

    /// Vista de gráfico: valor de un canal del hueso seleccionado a lo largo del tiempo.
    fn graph(&mut self, ui: &mut egui::Ui, dur: u32) {
        let label_w = 190.0;
        let avail = ui.available_size();
        let grid_w = (avail.x - label_w - 10.0).max(100.0);
        let fw = (grid_w / (dur as f32 + 1.0)).clamp(3.0, 40.0);
        self.ruler(ui, dur, label_w, fw);
        let Some(bone) = self.selected else {
            ui.label(RichText::new("Selecciona un hueso para ver sus curvas.").weak());
            return;
        };
        let bname = self.doc.bone(bone).map(|b| b.name.clone()).unwrap_or_default();
        let size = ui.available_size();
        let (resp, p) = ui.allocate_painter(vec2(size.x, size.y.max(80.0)), Sense::click_and_drag());
        let r = resp.rect;
        let gx0 = r.left() + label_w;
        p.rect_filled(r, 0.0, Color32::from_gray(36));
        p.rect_filled(Rect::from_min_max(r.min, pos2(gx0, r.bottom())), 0.0, Color32::from_gray(44));

        // selector de canal (en la columna izquierda)
        let opts: [(Channel, usize, &str); 5] = [
            (Channel::Rotate, 0, "Rotación"),
            (Channel::Translate, 0, "Posición X"),
            (Channel::Translate, 1, "Posición Y"),
            (Channel::Scale, 0, "Escala X"),
            (Channel::Scale, 1, "Escala Y"),
        ];
        p.text(pos2(r.left() + 8.0, r.top() + 6.0), Align2::LEFT_TOP, &bname, FontId::proportional(12.5), Color32::WHITE);
        let anim = self.doc.animations.get(self.anim).cloned();
        for (i, (ch, dim, name)) in opts.iter().enumerate() {
            let rr = Rect::from_min_size(pos2(r.left() + 6.0, r.top() + 26.0 + i as f32 * 21.0), vec2(label_w - 14.0, 19.0));
            let on = self.graph_ch == *ch && self.graph_dim == *dim;
            let has = anim.as_ref().and_then(|a| a.track(bone, *ch)).is_some();
            let resp_o = ui.interact(rr, ui.id().with(("gopt", i)), Sense::click());
            p.rect_filled(rr, 3.0, if on { Color32::from_rgb(50, 90, 130) } else if resp_o.hovered() { Color32::from_gray(60) } else { Color32::TRANSPARENT });
            let col = if has { ch_color(*ch) } else { Color32::from_gray(120) };
            p.text(rr.left_center() + vec2(6.0, 0.0), Align2::LEFT_CENTER, *name, FontId::proportional(12.0), col);
            if resp_o.clicked() {
                self.graph_ch = *ch;
                self.graph_dim = *dim;
            }
        }
        let (ch, dim) = (self.graph_ch, self.graph_dim);
        let Some(a) = anim else { return };
        let track = a.track(bone, ch).cloned();
        let setup = self.doc.bone(bone).map(|b| ch.get(&Doc::setup_local(b))[dim]).unwrap_or(0.0);
        let samples: Vec<(f32, f32)> = (0..=dur * 2)
            .map(|i| {
                let f = i as f32 * 0.5;
                let v = track.as_ref().and_then(|t| t.sample(f)).map(|v| v[dim]).unwrap_or(setup);
                (f, v)
            })
            .collect();
        let (mut lo, mut hi) = samples.iter().fold((f32::MAX, f32::MIN), |(lo, hi), (_, v)| (lo.min(*v), hi.max(*v)));
        if let Some(t) = &track {
            for k in &t.keys {
                lo = lo.min(k.v[dim]);
                hi = hi.max(k.v[dim]);
            }
        }
        if (hi - lo).abs() < 1e-3 {
            lo -= 1.0;
            hi += 1.0;
        }
        let pad = (hi - lo) * 0.15;
        lo -= pad;
        hi += pad;
        let top = r.top() + 10.0;
        let bot = r.bottom() - 10.0;
        let to_y = |v: f32| bot - (v - lo) / (hi - lo) * (bot - top);
        let from_y = |y: f32| lo + (bot - y) / (bot - top) * (hi - lo);
        let to_x = |f: f32| gx0 + f * fw + fw * 0.5;
        // guías
        for i in 0..=4 {
            let v = lo + (hi - lo) * i as f32 / 4.0;
            let y = to_y(v);
            p.line_segment([pos2(gx0, y), pos2(r.right(), y)], Stroke::new(1.0f32, Color32::from_white_alpha(12)));
            p.text(pos2(gx0 + 4.0, y - 1.0), Align2::LEFT_BOTTOM, format!("{v:.1}"), FontId::proportional(10.0), Color32::from_gray(130));
        }
        let col = ch_color(ch);
        let line: Vec<Pos2> = samples.iter().map(|(f, v)| pos2(to_x(*f), to_y(*v))).collect();
        p.add(egui::Shape::line(line, Stroke::new(2.0f32, col)));
        let px = to_x(self.frame);
        p.line_segment([pos2(px, r.top()), pos2(px, r.bottom())], Stroke::new(1.5f32, ACCENT));
        if track.is_none() {
            p.text(pos2(gx0 + 12.0, r.center().y), Align2::LEFT_CENTER, "Sin claves en este canal: anima el hueso para crear la curva.", FontId::proportional(12.0), Color32::from_gray(150));
        }
        if let Some(t) = &track {
            for k in &t.keys {
                let kp = pos2(to_x(k.frame as f32), to_y(k.v[dim]));
                let sel = self.selected_key == Some(KeyRef::Bone(bone, ch, k.frame));
                diamond(&p, kp, 6.0, if sel { Color32::YELLOW } else { col }, sel);
            }
        }
        // interacción: arrastrar claves en tiempo y valor
        if resp.drag_started() || resp.clicked() {
            if let (Some(pos), Some(t)) = (ui.input(|i| i.pointer.press_origin()).or(resp.interact_pointer_pos()), &track) {
                let hitk = t.keys.iter().find(|k| (pos2(to_x(k.frame as f32), to_y(k.v[dim])) - pos).length() < 9.0);
                if let Some(k) = hitk {
                    self.selected_key = Some(KeyRef::Bone(bone, ch, k.frame));
                    self.frame = k.frame as f32;
                    self.mode = Mode::Animate;
                    self.drag = Drag::GraphKey { bone, ch, frame: k.frame, dim };
                } else if pos.x > gx0 {
                    self.frame = ((pos.x - gx0) / fw - 0.5).round().clamp(0.0, dur as f32);
                    self.playing = false;
                }
            }
        }
        if resp.dragged() {
            if let (Drag::GraphKey { bone, ch, frame, dim }, Some(pos)) = (&self.drag, resp.interact_pointer_pos()) {
                let (bone, ch, frame, dim) = (*bone, *ch, *frame, *dim);
                let v = from_y(pos.y);
                let nf = ((pos.x - gx0) / fw - 0.5).round().clamp(0.0, 100000.0) as u32;
                let mut cur = KeyRef::Bone(bone, ch, frame);
                if nf != frame {
                    if let Some(nk) = self.move_key(&cur, nf) {
                        cur = nk;
                    }
                }
                let f2 = cur.frame();
                if let Some(a) = self.doc.animations.get_mut(self.anim) {
                    if let Some(k) = a.tracks.iter_mut().find(|t| t.bone == bone && t.channel == ch).and_then(|t| t.keys.iter_mut().find(|k| k.frame == f2)) {
                        k.v[dim] = v;
                    }
                }
                self.drag = Drag::GraphKey { bone, ch, frame: f2, dim };
                self.selected_key = Some(cur);
                self.frame = f2 as f32;
            }
        }
        if resp.drag_stopped() && matches!(self.drag, Drag::GraphKey { .. }) {
            self.drag = Drag::None;
        }
    }
}
