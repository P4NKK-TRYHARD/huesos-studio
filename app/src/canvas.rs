//! Lienzo: dibuja el esqueleto, slots, mallas y trayectorias, y maneja las herramientas.

use crate::app::*;
use crate::draw::*;
use crate::model::*;
use crate::pose::{mesh_world, Pose};
use egui::{pos2, vec2, Align2, Color32, FontId, Mesh, PointerButton, Pos2, Sense, Shape, Stroke, StrokeKind, TextureId};
use std::collections::HashMap;

/// Geometría de un adjunto ya colocado en el mundo.
pub enum Geom {
    Quad([[f32; 2]; 4]),
    Ellipse(Vec<[f32; 2]>),
    Tris(Vec<[f32; 2]>, Vec<[u32; 3]>),
    Line(Vec<[f32; 2]>),
    /// Polígono cerrado (caja de colisión o recorte).
    Poly(Vec<[f32; 2]>),
    /// Punto y su ángulo en grados.
    Point([f32; 2], f32),
}

fn mul_color(a: [u8; 4], b: [u8; 4]) -> [u8; 4] {
    let m = |x: u8, y: u8| ((x as u16 * y as u16 + 127) / 255) as u8;
    [m(a[0], b[0]), m(a[1], b[1]), m(a[2], b[2]), m(a[3], b[3])]
}

fn in_tri(p: [f32; 2], a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> bool {
    let s = |p1: [f32; 2], p2: [f32; 2], p3: [f32; 2]| (p1[0] - p3[0]) * (p2[1] - p3[1]) - (p2[0] - p3[0]) * (p1[1] - p3[1]);
    let d1 = s(p, a, b);
    let d2 = s(p, b, c);
    let d3 = s(p, c, a);
    let neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(neg && pos)
}

impl EditorApp {
    /// Geometría del adjunto visible de un slot.
    pub fn att_geom(&self, slot_id: u32, pose: &Pose, setup: &HashMap<u32, Aff>) -> Option<(String, Attachment, Geom)> {
        let slot = self.doc.slot(slot_id)?;
        let name = pose.slots.get(&slot_id)?.att.clone()?;
        let (_, att) = self.doc.find_att(self.skin, slot_id, &name)?;
        let bw = pose.world.get(&slot.bone).copied()?;
        let geom = match att.kind {
            AttachKind::Rect | AttachKind::Image | AttachKind::Ellipse => {
                let m = bw.mul(&att.local_aff());
                let (hw, hh) = (att.w / 2.0, att.h / 2.0);
                if att.kind == AttachKind::Ellipse {
                    Geom::Ellipse(
                        (0..32)
                            .map(|i| {
                                let t = i as f32 / 32.0 * std::f32::consts::TAU;
                                m.apply(t.cos() * hw, t.sin() * hh)
                            })
                            .collect(),
                    )
                } else {
                    Geom::Quad([m.apply(-hw, hh), m.apply(hw, hh), m.apply(hw, -hh), m.apply(-hw, -hh)])
                }
            }
            AttachKind::Mesh => {
                let mesh = att.mesh.as_ref()?;
                let deform = self.cur_anim().and_then(|a| a.deform_at(slot_id, &name, self.frame));
                let v = mesh_world(&self.doc, &pose.world, setup, slot, mesh, deform.as_deref());
                Geom::Tris(v, mesh.tris.clone())
            }
            AttachKind::Path => {
                let path = att.path.as_ref()?;
                let mut pts = Vec::new();
                for sg in path.segments(|p| bw.apply(p[0], p[1])) {
                    for i in 0..=20 {
                        pts.push(bezier_pt(&sg, i as f32 / 20.0));
                    }
                }
                Geom::Line(pts)
            }
            AttachKind::BoundingBox | AttachKind::Clipping => {
                let mesh = att.mesh.as_ref()?;
                let deform = self.cur_anim().and_then(|a| a.deform_at(slot_id, &name, self.frame));
                Geom::Poly(mesh_world(&self.doc, &pose.world, setup, slot, mesh, deform.as_deref()))
            }
            AttachKind::Point => {
                let m = bw.mul(&att.local_aff());
                Geom::Point(m.origin(), m.angle_deg())
            }
        };
        Some((name, att.clone(), geom))
    }

    pub fn canvas(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().frame(egui::Frame::default().fill(BG)).show(ctx, |ui| {
            let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
            let rect = resp.rect;
            let painter = painter.with_clip_rect(rect);

            if resp.hovered() {
                let scroll = ui.input(|i| i.smooth_scroll_delta.y);
                if scroll != 0.0 {
                    if let Some(mp) = resp.hover_pos() {
                        let view = View { center: rect.center() + self.pan, zoom: self.zoom };
                        let before = view.to_world(mp);
                        self.zoom = (self.zoom * (scroll * 0.002).exp()).clamp(0.05, 20.0);
                        let nv = View { center: rect.center() + self.pan, zoom: self.zoom };
                        self.pan += mp - nv.to_screen(before);
                    }
                }
            }
            let view = View { center: rect.center() + self.pan, zoom: self.zoom };

            draw_grid(&painter, rect, &view);

            let anim = self.cur_anim().cloned();
            let dt = ui.input(|i| i.stable_dt).min(0.1);
            self.phys.dt = if self.phys_on { dt } else { 0.0 };
            let pose = if self.phys_on {
                crate::pose::solve(&self.doc, anim.as_ref(), self.frame, self.skin, Some(&mut self.phys))
            } else {
                crate::pose::solve(&self.doc, anim.as_ref(), self.frame, self.skin, None)
            };
            let setup = self.doc.world_fk(None, 0.0);

            // texturas de los adjuntos visibles
            let mut tex: HashMap<String, TextureId> = HashMap::new();
            let paths: Vec<String> = self
                .doc
                .draw_order
                .iter()
                .filter_map(|s| {
                    let n = pose.slots.get(s)?.att.clone()?;
                    let (_, a) = self.doc.find_att(self.skin, *s, &n)?;
                    (!a.image.is_empty()).then(|| a.image.clone())
                })
                .collect();
            for p in paths {
                if let Some(t) = self.texture(ctx, &p) {
                    tex.insert(p, t);
                }
            }

            if self.onion {
                if let Some(a) = anim.as_ref() {
                    let dur = a.duration as f32;
                    let ghosts = [
                        (-6.0, Color32::from_rgba_unmultiplied(255, 80, 80, 50)),
                        (-3.0, Color32::from_rgba_unmultiplied(255, 80, 80, 90)),
                        (3.0, Color32::from_rgba_unmultiplied(80, 255, 120, 90)),
                        (6.0, Color32::from_rgba_unmultiplied(80, 255, 120, 50)),
                    ];
                    for (off, col) in ghosts {
                        let mut f = self.frame + off;
                        if self.looping {
                            f = f.rem_euclid(dur.max(1.0));
                        }
                        let w = self.doc.world(Some(a), f);
                        for b in &self.doc.bones {
                            if let Some(m) = w.get(&b.id) {
                                let p0 = view.to_screen(m.apply(0.0, 0.0));
                                let p1 = view.to_screen(m.apply(b.length, 0.0));
                                painter.line_segment([p0, p1], Stroke::new(3.0f32, col));
                            }
                        }
                    }
                }
            }

            if self.show_images {
                self.draw_attachments(&painter, &view, &pose, &setup, &tex);
            }

            self.draw_constraints(&painter, &view, &pose);

            if self.show_bones {
                for b in &self.doc.bones {
                    if let Some(m) = pose.world.get(&b.id) {
                        draw_bone(&painter, &view, m, b, self.selected == Some(b.id), self.show_names);
                    }
                }
            }

            if self.tool == Tool::Vertices {
                self.draw_handles(&painter, &view, &pose, &setup);
            }

            if let Drag::Create { start, cur, .. } = &self.drag {
                let p0 = view.to_screen(*start);
                let p1 = view.to_screen(*cur);
                painter.line_segment([p0, p1], Stroke::new(3.0f32, SEL));
                painter.circle_stroke(p0, 5.0, Stroke::new(2.0f32, SEL));
            }

            let mode_txt = match self.mode {
                Mode::Setup => "CONFIGURACIÓN".to_string(),
                Mode::Animate => format!(
                    "ANIMACIÓN · {} · fotograma {}",
                    anim.as_ref().map(|a| a.name.as_str()).unwrap_or(""),
                    self.frame.round()
                ),
            };
            let mc = if self.mode == Mode::Animate { ACCENT } else { Color32::from_rgb(120, 170, 230) };
            painter.text(rect.left_top() + vec2(12.0, 10.0), Align2::LEFT_TOP, mode_txt, FontId::proportional(15.0), mc);
            let skin_name = self.doc.skins.get(self.skin).map(|s| s.name.clone()).unwrap_or_default();
            painter.text(
                rect.left_top() + vec2(12.0, 30.0),
                Align2::LEFT_TOP,
                format!("Herramienta: {}   ·   Skin: {}   ·   Zoom {:.0}%", self.tool.label(), skin_name, self.zoom * 100.0),
                FontId::proportional(12.0),
                Color32::from_gray(170),
            );
            if self.mode == Mode::Animate {
                painter.rect_stroke(rect.shrink(1.0), 0.0f32, Stroke::new(2.0f32, ACCENT.gamma_multiply(0.6)), StrokeKind::Inside);
            }
            // eventos disparados
            for (i, (txt, _)) in self.fired.iter().rev().take(4).enumerate() {
                let p = rect.right_top() + vec2(-14.0, 14.0 + i as f32 * 24.0);
                let galley = painter.layout_no_wrap(txt.clone(), FontId::proportional(14.0), Color32::BLACK);
                let r = egui::Rect::from_min_size(p - vec2(galley.size().x + 12.0, 0.0), galley.size() + vec2(12.0, 6.0));
                painter.rect_filled(r, 4.0, Color32::from_rgb(255, 215, 90));
                painter.galley(r.min + vec2(6.0, 3.0), galley, Color32::BLACK);
            }
            painter.text(rect.left_bottom() + vec2(12.0, -10.0), Align2::LEFT_BOTTOM, &self.status, FontId::proportional(12.0), Color32::from_gray(160));

            self.canvas_input(ui, &resp, &view, &pose, &setup);
        });
    }

    fn draw_attachments(&self, painter: &egui::Painter, view: &View, pose: &Pose, setup: &HashMap<u32, Aff>, tex: &HashMap<String, TextureId>) {
        // recorte activo: triángulos del polígono y último slot afectado
        let mut clip: Option<(Vec<[[f32; 2]; 3]>, Option<u32>)> = None;
        for sid in &pose.draw_order {
            let Some(slot) = self.doc.slot(*sid) else { continue };
            let visible = self.doc.bone(slot.bone).map(|b| b.visible).unwrap_or(false);
            let geom = if visible { self.att_geom(*sid, pose, setup) } else { None };
            if let Some((_, att, geom)) = geom {
                let scol = pose.slots.get(sid).map(|s| s.color).unwrap_or([255; 4]);
                let tint = c32(mul_color(att.tint, scol));
                let selected = self.sel_slot == Some(*sid);
                let t = tex.get(&att.image).copied();
                // triángulos con uv a dibujar
                let mut tris: Vec<[crate::clip::V; 3]> = Vec::new();
                let mut textured = false;
                match &geom {
                    Geom::Quad(c) => {
                        let uv = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
                        if att.kind == AttachKind::Image && t.is_none() {
                            let pts: Vec<Pos2> = c.iter().map(|p| view.to_screen(*p)).collect();
                            let mc = Color32::from_rgb(220, 90, 220);
                            painter.add(Shape::convex_polygon(pts.clone(), Color32::from_rgba_unmultiplied(200, 60, 200, 60), Stroke::new(1.0f32, mc)));
                            painter.line_segment([pts[0], pts[2]], Stroke::new(1.0f32, mc));
                            painter.line_segment([pts[1], pts[3]], Stroke::new(1.0f32, mc));
                        } else {
                            textured = att.kind == AttachKind::Image;
                            tris.push([(c[0], uv[0]), (c[1], uv[1]), (c[2], uv[2])]);
                            tris.push([(c[0], uv[0]), (c[2], uv[2]), (c[3], uv[3])]);
                        }
                        if selected {
                            let pts: Vec<Pos2> = c.iter().map(|p| view.to_screen(*p)).collect();
                            painter.add(Shape::closed_line(pts, Stroke::new(1.0f32, SEL.gamma_multiply(0.8))));
                        }
                    }
                    Geom::Ellipse(e) => {
                        let c = [e.iter().map(|p| p[0]).sum::<f32>() / e.len() as f32, e.iter().map(|p| p[1]).sum::<f32>() / e.len() as f32];
                        for i in 0..e.len() {
                            tris.push([(c, [0.5, 0.5]), (e[i], [0.5, 0.5]), (e[(i + 1) % e.len()], [0.5, 0.5])]);
                        }
                        if selected {
                            let pts: Vec<Pos2> = e.iter().map(|p| view.to_screen(*p)).collect();
                            painter.add(Shape::closed_line(pts, Stroke::new(1.0f32, SEL.gamma_multiply(0.8))));
                        }
                    }
                    Geom::Tris(v, ts) => {
                        let m = att.mesh.as_ref().unwrap();
                        textured = t.is_some();
                        for tr in ts {
                            if tr.iter().all(|i| (*i as usize) < v.len()) {
                                let g = |i: u32| (v[i as usize], m.uvs.get(i as usize).copied().unwrap_or([0.0, 0.0]));
                                tris.push([g(tr[0]), g(tr[1]), g(tr[2])]);
                            }
                        }
                    }
                    Geom::Line(pts) => {
                        let sp: Vec<Pos2> = pts.iter().map(|p| view.to_screen(*p)).collect();
                        painter.add(Shape::line(sp, Stroke::new(if selected { 3.0f32 } else { 2.0f32 }, tint)));
                    }
                    Geom::Poly(pts) => {
                        if att.kind == AttachKind::Clipping {
                            clip = Some((crate::clip::triangulate(pts), att.end_slot));
                        }
                        if self.show_bones || selected {
                            let sp: Vec<Pos2> = pts.iter().map(|p| view.to_screen(*p)).collect();
                            let col = c32(att.tint).gamma_multiply(if selected { 1.0 } else { 0.6 });
                            painter.add(Shape::closed_line(sp, Stroke::new(if selected { 2.0f32 } else { 1.0f32 }, col)));
                        }
                    }
                    Geom::Point(p, ang) => {
                        if self.show_bones || selected {
                            let sp = view.to_screen(*p);
                            let col = c32(att.tint);
                            let (s, c) = ang.to_radians().sin_cos();
                            painter.circle_stroke(sp, 5.0, Stroke::new(1.5f32, col));
                            painter.line_segment([sp, sp + vec2(c, -s) * 16.0], Stroke::new(1.5f32, col));
                        }
                    }
                }
                if !tris.is_empty() {
                    let active_clip = clip.as_ref().filter(|_| att.kind != AttachKind::Clipping);
                    let draw = match active_clip {
                        Some((ct, _)) => crate::clip::clip_triangles(&tris, ct),
                        None => tris,
                    };
                    let mut mesh = if textured { Mesh::with_texture(t.unwrap()) } else { Mesh::default() };
                    for tri in &draw {
                        let base = mesh.vertices.len() as u32;
                        for (p, uv) in tri {
                            let uv = if textured { pos2(uv[0], uv[1]) } else { egui::epaint::WHITE_UV };
                            mesh.vertices.push(egui::epaint::Vertex { pos: view.to_screen(*p), uv, color: tint });
                        }
                        mesh.add_triangle(base, base + 1, base + 2);
                    }
                    painter.add(Shape::mesh(mesh));
                    if matches!(att.kind, AttachKind::Rect | AttachKind::Ellipse) && active_clip.is_none() {
                        // borde suave como antes
                        if let Geom::Quad(c) = &geom {
                            let pts: Vec<Pos2> = c.iter().map(|p| view.to_screen(*p)).collect();
                            painter.add(Shape::closed_line(pts, Stroke::new(1.0f32, Color32::from_black_alpha(90))));
                        }
                    }
                }
                if let (Geom::Tris(v, ts), true) = (&geom, self.show_mesh && (selected || self.tool == Tool::Vertices)) {
                    let m = att.mesh.as_ref().unwrap();
                    let col = if selected { SEL.gamma_multiply(0.7) } else { Color32::from_white_alpha(40) };
                    for tr in ts {
                        for k in 0..3 {
                            let (a, b) = (tr[k] as usize, tr[(k + 1) % 3] as usize);
                            if a < v.len() && b < v.len() {
                                painter.line_segment([view.to_screen(v[a]), view.to_screen(v[b])], Stroke::new(1.0f32, col));
                            }
                        }
                    }
                    if m.hull > 1 && selected {
                        let hull: Vec<Pos2> = v.iter().take(m.hull).map(|p| view.to_screen(*p)).collect();
                        painter.add(Shape::closed_line(hull, Stroke::new(1.5f32, SEL)));
                    }
                }
            }
            if let Some((_, end)) = &clip {
                if *end == Some(*sid) {
                    clip = None;
                }
            }
        }
    }

    fn draw_constraints(&self, painter: &egui::Painter, view: &View, pose: &Pose) {
        if !self.show_bones {
            return;
        }
        for (i, c) in self.doc.constraints.iter().enumerate() {
            let sel = self.sel_constraint == Some(i);
            let alpha = if c.enabled { 1.0 } else { 0.35 };
            match &c.kind {
                ConstraintKind::Ik { target, .. } | ConstraintKind::Transform { target, .. } => {
                    let col = if matches!(c.kind, ConstraintKind::Ik { .. }) { Color32::from_rgb(250, 90, 160) } else { Color32::from_rgb(120, 220, 220) };
                    let col = col.gamma_multiply(alpha);
                    let Some(tw) = pose.world.get(target) else { continue };
                    let tp = view.to_screen(tw.origin());
                    painter.circle_stroke(tp, if sel { 11.0 } else { 9.0 }, Stroke::new(2.0f32, col));
                    painter.line_segment([tp - vec2(13.0, 0.0), tp + vec2(13.0, 0.0)], Stroke::new(1.0f32, col));
                    painter.line_segment([tp - vec2(0.0, 13.0), tp + vec2(0.0, 13.0)], Stroke::new(1.0f32, col));
                    if let Some(last) = c.bones.last().and_then(|b| pose.world.get(b).map(|w| (w, *b))) {
                        let len = self.doc.bone(last.1).map(|b| b.length).unwrap_or(0.0);
                        let from = if matches!(c.kind, ConstraintKind::Ik { .. }) { last.0.apply(len, 0.0) } else { last.0.origin() };
                        dashed(painter, view.to_screen(from), tp, Stroke::new(1.0f32, col));
                    }
                }
                ConstraintKind::Physics { .. } => {
                    let Some(b) = c.bones.first() else { continue };
                    let Some(w) = pose.world.get(b) else { continue };
                    let len = self.doc.bone(*b).map(|b| b.length).unwrap_or(0.0);
                    let tip = view.to_screen(w.apply(len, 0.0));
                    let col = Color32::from_rgb(180, 140, 255).gamma_multiply(alpha);
                    painter.circle_stroke(tip, if sel { 7.0 } else { 5.0 }, Stroke::new(1.5f32, col));
                }
                ConstraintKind::Path { .. } => {}
            }
        }
    }

    fn draw_handles(&self, painter: &egui::Painter, view: &View, pose: &Pose, setup: &HashMap<u32, Aff>) {
        let Some(sid) = self.sel_slot else { return };
        let Some((_, att, geom)) = self.att_geom(sid, pose, setup) else { return };
        match (&geom, att.kind) {
            (Geom::Tris(v, _), _) | (Geom::Poly(v), _) => {
                for (i, p) in v.iter().enumerate() {
                    let sp = view.to_screen(*p);
                    let hull = att.mesh.as_ref().map(|m| i < m.hull).unwrap_or(false);
                    painter.circle_filled(sp, 4.0, if hull { SEL } else { Color32::from_rgb(255, 230, 120) });
                    painter.circle_stroke(sp, 4.0, Stroke::new(1.0f32, Color32::BLACK));
                }
            }
            (Geom::Line(_), AttachKind::Path) => {
                let Some(slot) = self.doc.slot(sid) else { return };
                let Some(bw) = pose.world.get(&slot.bone) else { return };
                let Some(path) = &att.path else { return };
                for pt in &path.pts {
                    let p = view.to_screen(bw.apply(pt.p[0], pt.p[1]));
                    let hi = view.to_screen(bw.apply(pt.p[0] + pt.hin[0], pt.p[1] + pt.hin[1]));
                    let ho = view.to_screen(bw.apply(pt.p[0] + pt.hout[0], pt.p[1] + pt.hout[1]));
                    painter.line_segment([hi, ho], Stroke::new(1.0f32, Color32::from_gray(200)));
                    painter.circle_filled(hi, 3.5, Color32::from_gray(230));
                    painter.circle_filled(ho, 3.5, Color32::from_gray(230));
                    painter.rect_filled(egui::Rect::from_center_size(p, vec2(9.0, 9.0)), 1.0, ACCENT);
                }
            }
            _ => {}
        }
    }

    fn hit_bone(&self, world: &HashMap<u32, Aff>, view: &View, p: Pos2) -> Option<u32> {
        let mut best: Option<(u32, f32)> = None;
        if self.show_bones {
            for b in &self.doc.bones {
                let Some(m) = world.get(&b.id) else { continue };
                let a = view.to_screen(m.apply(0.0, 0.0));
                let c = view.to_screen(m.apply(b.length, 0.0));
                let d = dist_seg(p, a, c);
                if d <= 8.0 && best.map(|(_, bd)| d <= bd).unwrap_or(true) {
                    best = Some((b.id, d));
                }
            }
        }
        best.map(|b| b.0)
    }

    fn hit_slot(&self, pose: &Pose, setup: &HashMap<u32, Aff>, view: &View, p: Pos2) -> Option<u32> {
        if !self.show_images {
            return None;
        }
        let wp = view.to_world(p);
        for sid in pose.draw_order.iter().rev() {
            let Some(slot) = self.doc.slot(*sid) else { continue };
            if !self.doc.bone(slot.bone).map(|b| b.visible).unwrap_or(false) {
                continue;
            }
            let Some((_, _, geom)) = self.att_geom(*sid, pose, setup) else { continue };
            let hit = match &geom {
                Geom::Quad(c) => in_tri(wp, c[0], c[1], c[2]) || in_tri(wp, c[0], c[2], c[3]),
                Geom::Ellipse(e) => (1..e.len() - 1).any(|i| in_tri(wp, e[0], e[i], e[i + 1])),
                Geom::Tris(v, t) => t.iter().any(|t| {
                    t.iter().all(|i| (*i as usize) < v.len()) && in_tri(wp, v[t[0] as usize], v[t[1] as usize], v[t[2] as usize])
                }),
                Geom::Line(pts) => pts.windows(2).any(|w| dist_seg(p, view.to_screen(w[0]), view.to_screen(w[1])) < 6.0),
                Geom::Poly(pts) => (0..pts.len()).any(|i| dist_seg(p, view.to_screen(pts[i]), view.to_screen(pts[(i + 1) % pts.len()])) < 6.0),
                Geom::Point(q, _) => (view.to_screen(*q) - p).length() < 8.0,
            };
            if hit {
                return Some(*sid);
            }
        }
        None
    }

    /// Mapa lineal de "desplazamiento en espacio de malla" a "desplazamiento en el mundo" para un vértice.
    fn deform_jacobian(&self, slot: &Slot, mesh: &MeshData, idx: usize, world: &HashMap<u32, Aff>, setup: &HashMap<u32, Aff>) -> Aff {
        let sb = world.get(&slot.bone).copied().unwrap_or(Aff::IDENTITY).linear();
        if !mesh.weighted() {
            return sb;
        }
        let sbs = setup.get(&slot.bone).copied().unwrap_or(Aff::IDENTITY).linear();
        let mut j = Aff { a: 0.0, b: 0.0, c: 0.0, d: 0.0, tx: 0.0, ty: 0.0 };
        for w in &mesh.weights[idx] {
            let bw = world.get(&w.bone).copied().unwrap_or(Aff::IDENTITY).linear();
            let bs = setup.get(&w.bone).copied().unwrap_or(Aff::IDENTITY).linear();
            let m = bw.mul(&bs.inverse()).mul(&sbs);
            j.a += m.a * w.w;
            j.b += m.b * w.w;
            j.c += m.c * w.w;
            j.d += m.d * w.w;
        }
        j
    }

    fn start_vertex_drag(&mut self, view: &View, pose: &Pose, setup: &HashMap<u32, Aff>, origin: Pos2) -> bool {
        let Some(sid) = self.sel_slot else { return false };
        let Some((name, att, geom)) = self.att_geom(sid, pose, setup) else { return false };
        match geom {
            Geom::Tris(v, _) | Geom::Poly(v) => {
                let best = v
                    .iter()
                    .enumerate()
                    .map(|(i, p)| (i, (view.to_screen(*p) - origin).length()))
                    .filter(|(_, d)| *d <= 9.0)
                    .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
                if let Some((idx, _)) = best {
                    self.drag = Drag::Vertex { slot: sid, att: name, idx, last: view.to_world(origin) };
                    return true;
                }
            }
            Geom::Line(_) => {
                let Some(slot) = self.doc.slot(sid) else { return false };
                let Some(bw) = pose.world.get(&slot.bone).copied() else { return false };
                let Some(path) = &att.path else { return false };
                let mut best: Option<(usize, u8, f32)> = None;
                for (i, pt) in path.pts.iter().enumerate() {
                    let cands = [
                        (0u8, pt.p),
                        (1u8, [pt.p[0] + pt.hin[0], pt.p[1] + pt.hin[1]]),
                        (2u8, [pt.p[0] + pt.hout[0], pt.p[1] + pt.hout[1]]),
                    ];
                    for (part, q) in cands {
                        let d = (view.to_screen(bw.apply(q[0], q[1])) - origin).length();
                        if d <= 9.0 && best.map(|b| d < b.2).unwrap_or(true) {
                            best = Some((i, part, d));
                        }
                    }
                }
                if let Some((idx, part, _)) = best {
                    if self.mode == Mode::Animate {
                        self.status = "Las trayectorias se editan en modo CONFIGURAR (Tab).".into();
                        return true;
                    }
                    self.drag = Drag::PathPt { slot: sid, att: name, idx, part };
                    return true;
                }
            }
            _ => {}
        }
        false
    }

    fn drag_vertex(&mut self, w: [f32; 2], pose: &Pose, setup: &HashMap<u32, Aff>, shift: bool) {
        match &mut self.drag {
            Drag::Vertex { slot, att, idx, last } => {
                let (sid, name, idx) = (*slot, att.clone(), *idx);
                let delta = [w[0] - last[0], w[1] - last[1]];
                *last = w;
                let Some(s) = self.doc.slot(sid).cloned() else { return };
                let Some(mesh) = self.doc.find_att(self.skin, sid, &name).and_then(|(_, a)| a.mesh.clone()) else { return };
                if idx >= mesh.verts.len() {
                    return;
                }
                match self.mode {
                    Mode::Setup => {
                        let cur = mesh_world(&self.doc, &pose.world, setup, &s, &mesh, None);
                        let target = [cur[idx][0] + delta[0], cur[idx][1] + delta[1]];
                        let mut m = mesh.clone();
                        crate::mesh::set_setup_vertex(&self.doc, s.bone, &mut m, idx, target);
                        if let Some(a) = self.doc.find_att_mut(self.skin, sid, &name) {
                            a.mesh = Some(m);
                        }
                    }
                    Mode::Animate => {
                        let f = self.key_frame();
                        self.frame = f as f32;
                        let j = self.deform_jacobian(&s, &mesh, idx, &pose.world, setup);
                        let dl = j.inv_vec(delta[0], delta[1]);
                        let Some(a) = self.doc.animations.get_mut(self.anim) else { return };
                        let mut offs = a.deform_at(sid, &name, f as f32).unwrap_or_default();
                        offs.resize(mesh.verts.len(), [0.0, 0.0]);
                        offs[idx][0] += dl[0];
                        offs[idx][1] += dl[1];
                        a.set_deform_key(sid, &name, f, offs);
                    }
                }
            }
            Drag::PathPt { slot, att, idx, part } => {
                let (sid, name, idx, part) = (*slot, att.clone(), *idx, *part);
                let Some(s) = self.doc.slot(sid) else { return };
                let Some(bw) = pose.world.get(&s.bone).copied() else { return };
                let l = bw.inv_point(w[0], w[1]);
                let Some(a) = self.doc.find_att_mut(self.skin, sid, &name) else { return };
                let Some(path) = a.path.as_mut() else { return };
                let Some(pt) = path.pts.get_mut(idx) else { return };
                match part {
                    0 => pt.p = l,
                    1 => {
                        pt.hin = [l[0] - pt.p[0], l[1] - pt.p[1]];
                        if !shift {
                            let lo = (pt.hout[0].powi(2) + pt.hout[1].powi(2)).sqrt();
                            let li = (pt.hin[0].powi(2) + pt.hin[1].powi(2)).sqrt().max(1e-4);
                            pt.hout = [-pt.hin[0] / li * lo, -pt.hin[1] / li * lo];
                        }
                    }
                    _ => {
                        pt.hout = [l[0] - pt.p[0], l[1] - pt.p[1]];
                        if !shift {
                            let li = (pt.hin[0].powi(2) + pt.hin[1].powi(2)).sqrt();
                            let lo = (pt.hout[0].powi(2) + pt.hout[1].powi(2)).sqrt().max(1e-4);
                            pt.hin = [-pt.hout[0] / lo * li, -pt.hout[1] / lo * li];
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn canvas_input(&mut self, ui: &egui::Ui, resp: &egui::Response, view: &View, pose: &Pose, setup: &HashMap<u32, Aff>) {
        let world = &pose.world;
        let parent_aff = |app: &Self, id: u32| -> Aff {
            app.doc.bone(id).and_then(|b| b.parent).and_then(|p| world.get(&p).copied()).unwrap_or(Aff::IDENTITY)
        };
        let origin_of = |id: u32| -> [f32; 2] { world.get(&id).map(|m| m.apply(0.0, 0.0)).unwrap_or([0.0, 0.0]) };

        if resp.clicked() {
            if let Some(p) = resp.interact_pointer_pos() {
                let hb = self.hit_bone(world, view, p);
                if hb.is_some() && self.tool != Tool::Vertices {
                    self.selected = hb;
                    self.sel_slot = None;
                } else if let Some(s) = self.hit_slot(pose, setup, view, p) {
                    self.sel_slot = Some(s);
                    self.selected = self.doc.slot(s).map(|s| s.bone);
                } else if hb.is_some() {
                    self.selected = hb;
                    self.sel_slot = None;
                } else {
                    self.selected = None;
                    self.sel_slot = None;
                }
                self.sel_constraint = None;
                self.selected_key = None;
            }
        }

        if resp.drag_started() {
            let origin = ui.input(|i| i.pointer.press_origin()).unwrap_or_default();
            let secondary = resp.dragged_by(PointerButton::Secondary) || resp.dragged_by(PointerButton::Middle);
            if secondary {
                self.drag = Drag::Pan;
            } else {
                let hit = self.hit_bone(world, view, origin);
                let w = view.to_world(origin);
                let angle_to = |id: u32| -> f32 {
                    let o = origin_of(id);
                    (w[1] - o[1]).atan2(w[0] - o[0]).to_degrees()
                };
                match self.tool {
                    Tool::Vertices => {
                        if !self.start_vertex_drag(view, pose, setup, origin) {
                            if let Some(s) = self.hit_slot(pose, setup, view, origin) {
                                self.sel_slot = Some(s);
                                self.selected = self.doc.slot(s).map(|s| s.bone);
                                self.start_vertex_drag(view, pose, setup, origin);
                            }
                            if matches!(self.drag, Drag::None) {
                                self.drag = Drag::Pan;
                            }
                        }
                    }
                    Tool::Select => match hit {
                        Some(h) => {
                            self.selected = Some(h);
                            self.drag = Drag::Rotate { bone: h, last: angle_to(h) };
                        }
                        None => self.drag = Drag::Pan,
                    },
                    Tool::Rotate | Tool::Translate | Tool::Scale => {
                        if hit.is_some() {
                            self.selected = hit;
                        }
                        match self.selected {
                            Some(id) if world.contains_key(&id) => {
                                self.drag = match self.tool {
                                    Tool::Rotate => Drag::Rotate { bone: id, last: angle_to(id) },
                                    Tool::Translate => Drag::Translate { bone: id, last: w },
                                    _ => {
                                        let o = origin_of(id);
                                        let l = self.current_local(id);
                                        let d = ((w[0] - o[0]).powi(2) + (w[1] - o[1]).powi(2)).sqrt().max(1.0);
                                        Drag::Scale { bone: id, start_dist: d, start: [l.sx, l.sy] }
                                    }
                                };
                            }
                            _ => self.drag = Drag::Pan,
                        }
                    }
                    Tool::Create => {
                        if self.mode == Mode::Animate {
                            self.status = "Para crear huesos cambia a modo CONFIGURAR (Tab).".into();
                            self.drag = Drag::None;
                        } else {
                            let parent = hit.or(self.selected);
                            self.drag = Drag::Create { parent, start: w, cur: w };
                        }
                    }
                }
            }
        }

        if resp.dragged() {
            let delta = resp.drag_delta();
            let mp = resp.interact_pointer_pos().unwrap_or_default();
            let w = view.to_world(mp);
            let shift = ui.input(|i| i.modifiers.shift);
            let mut change: Option<(u32, Channel, [f32; 3])> = None;
            if matches!(self.drag, Drag::Vertex { .. } | Drag::PathPt { .. }) {
                self.drag_vertex(w, pose, setup, shift);
            }
            match &mut self.drag {
                Drag::Pan => self.pan += delta,
                Drag::Rotate { bone, last } => {
                    let o = origin_of(*bone);
                    let ang = (w[1] - o[1]).atan2(w[0] - o[0]).to_degrees();
                    let d = wrap_deg(ang - *last);
                    *last = ang;
                    change = Some((*bone, Channel::Rotate, [d, 0.0, 0.0]));
                }
                Drag::Translate { bone, last } => {
                    let dw = [w[0] - last[0], w[1] - last[1]];
                    *last = w;
                    change = Some((*bone, Channel::Translate, [dw[0], dw[1], 0.0]));
                }
                Drag::Scale { bone, start_dist, start } => {
                    let o = origin_of(*bone);
                    let d = ((w[0] - o[0]).powi(2) + (w[1] - o[1]).powi(2)).sqrt();
                    let f = d / *start_dist;
                    let sy = if shift { start[1] } else { start[1] * f };
                    change = Some((*bone, Channel::Scale, [start[0] * f, sy, 0.0]));
                }
                Drag::Create { cur, .. } => *cur = w,
                _ => {}
            }
            if let Some((id, ch, v)) = change {
                let mut l = self.current_local(id);
                let pa = parent_aff(self, id);
                match ch {
                    Channel::Rotate => {
                        let flip = pa.det() < 0.0;
                        l.rot += if flip { -v[0] } else { v[0] };
                    }
                    Channel::Translate => {
                        let dl = pa.inv_vec(v[0], v[1]);
                        l.x += dl[0];
                        l.y += dl[1];
                    }
                    Channel::Scale => {
                        l.sx = v[0];
                        l.sy = v[1];
                    }
                    Channel::Shear => {}
                }
                self.write_local(id, l, ch);
            }
        }

        if resp.drag_stopped() {
            if let Drag::Create { parent, start, cur } = self.drag {
                let pa = parent.and_then(|p| world.get(&p).copied()).unwrap_or(Aff::IDENTITY);
                let lp = pa.inv_point(start[0], start[1]);
                let dl = pa.inv_vec(cur[0] - start[0], cur[1] - start[1]);
                let len = (dl[0] * dl[0] + dl[1] * dl[1]).sqrt();
                let rot = if len > 0.5 { dl[1].atan2(dl[0]).to_degrees() } else { 0.0 };
                let name = self.doc.unique_name("hueso");
                let id = self.doc.add_bone(parent, name.clone(), lp[0], lp[1], rot, len);
                self.selected = Some(id);
                self.sel_slot = None;
                self.status = format!("Creado «{name}»");
            }
            if !matches!(self.drag, Drag::Key(_) | Drag::GraphKey { .. }) {
                self.drag = Drag::None;
            }
        }
    }
}

fn dashed(p: &egui::Painter, a: Pos2, b: Pos2, s: Stroke) {
    let len = (b - a).length();
    if len < 1.0 {
        return;
    }
    let dir = (b - a) / len;
    let mut t = 0.0;
    while t < len {
        let e = (t + 6.0).min(len);
        p.line_segment([a + dir * t, a + dir * e], s);
        t += 11.0;
    }
}
