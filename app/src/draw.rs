//! Utilidades de dibujo y estilo.

use crate::app::{ACCENT, PANEL, SEL};
use crate::model::*;
use egui::{pos2, vec2, Align2, Color32, FontId, Pos2, Rect, Sense, Shape, Stroke};

#[derive(Clone, Copy)]
pub struct View {
    pub center: Pos2,
    pub zoom: f32,
}

impl View {
    pub fn to_screen(&self, p: [f32; 2]) -> Pos2 {
        pos2(self.center.x + p[0] * self.zoom, self.center.y - p[1] * self.zoom)
    }
    pub fn to_world(&self, p: Pos2) -> [f32; 2] {
        [(p.x - self.center.x) / self.zoom, (self.center.y - p.y) / self.zoom]
    }
}

pub fn ch_color(ch: Channel) -> Color32 {
    match ch {
        Channel::Rotate => Color32::from_rgb(110, 210, 110),
        Channel::Translate => Color32::from_rgb(90, 160, 255),
        Channel::Scale => Color32::from_rgb(240, 100, 100),
        Channel::Shear => Color32::from_rgb(230, 200, 90),
    }
}

pub fn c32(c: [u8; 4]) -> Color32 {
    Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3])
}

pub fn wrap_deg(mut a: f32) -> f32 {
    while a > 180.0 {
        a -= 360.0;
    }
    while a < -180.0 {
        a += 360.0;
    }
    a
}

pub fn setup_style(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    v.panel_fill = PANEL;
    v.window_fill = Color32::from_rgb(52, 54, 60);
    v.extreme_bg_color = Color32::from_rgb(30, 31, 34);
    v.selection.bg_fill = Color32::from_rgb(50, 110, 170);
    v.widgets.inactive.bg_fill = Color32::from_rgb(62, 64, 70);
    v.widgets.inactive.weak_bg_fill = Color32::from_rgb(62, 64, 70);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(76, 80, 88);
    ctx.set_visuals(v);
    ctx.style_mut(|s| {
        s.spacing.item_spacing = vec2(6.0, 5.0);
        s.spacing.button_padding = vec2(7.0, 3.0);
    });
}

pub fn key_btn(ui: &mut egui::Ui, keyed: bool, col: Color32) -> bool {
    let (r, p) = ui.allocate_painter(vec2(18.0, 18.0), Sense::click());
    let c = r.rect.center();
    let hover = r.hovered();
    if keyed {
        diamond(&p, c, 6.0, col, hover);
    } else {
        p.add(Shape::closed_line(
            vec![c + vec2(0.0, -6.0), c + vec2(6.0, 0.0), c + vec2(0.0, 6.0), c + vec2(-6.0, 0.0)],
            Stroke::new(1.5f32, if hover { Color32::WHITE } else { col.gamma_multiply(0.7) }),
        ));
    }
    r.on_hover_text(if keyed { "Quitar clave" } else { "Fijar clave" }).clicked()
}

pub fn diamond(p: &egui::Painter, c: Pos2, s: f32, col: Color32, outline: bool) {
    let pts = vec![c + vec2(0.0, -s), c + vec2(s, 0.0), c + vec2(0.0, s), c + vec2(-s, 0.0)];
    let stroke = if outline { Stroke::new(1.5f32, Color32::WHITE) } else { Stroke::new(1.0f32, Color32::from_black_alpha(160)) };
    p.add(Shape::convex_polygon(pts, col, stroke));
}

/// Selector de curva con editor Bézier interactivo. Devuelve true si cambió.
pub fn curve_editor(ui: &mut egui::Ui, curve: &mut Curve) -> bool {
    let before = *curve;
    ui.horizontal(|ui| {
        ui.label("Curva");
        egui::ComboBox::from_id_salt("curve_kind").selected_text(curve.label()).show_ui(ui, |ui| {
            for c in [Curve::Linear, Curve::Smooth, Curve::Stepped] {
                ui.selectable_value(curve, c, c.label());
            }
            let bz = Curve::Bezier(curve.as_bezier().unwrap_or([0.25, 0.1, 0.25, 1.0]));
            if ui.selectable_label(matches!(curve, Curve::Bezier(_)), "Bézier").clicked() {
                *curve = bz;
            }
        });
    });
    let presets: [(&str, [f32; 4]); 4] = [
        ("Entrada", [0.42, 0.0, 1.0, 1.0]),
        ("Salida", [0.0, 0.0, 0.58, 1.0]),
        ("Rebote", [0.3, 1.6, 0.6, 1.0]),
        ("Anticipar", [0.5, -0.5, 0.7, 1.0]),
    ];
    ui.horizontal_wrapped(|ui| {
        for (n, c) in presets {
            if ui.small_button(n).clicked() {
                *curve = Curve::Bezier(c);
            }
        }
    });
    let w = ui.available_width().min(240.0);
    let (r, p) = ui.allocate_painter(vec2(w, 150.0), Sense::hover());
    let rect = r.rect.shrink2(vec2(10.0, 8.0));
    p.rect_filled(r.rect, 4.0f32, Color32::from_gray(32));
    // y visible: -0.5 .. 1.5
    let to_s = |x: f32, y: f32| pos2(rect.left() + x * rect.width(), rect.bottom() - (y + 0.5) / 2.0 * rect.height());
    let from_s = |q: Pos2| ((q.x - rect.left()) / rect.width(), (rect.bottom() - q.y) / rect.height() * 2.0 - 0.5);
    p.line_segment([to_s(0.0, 0.0), to_s(1.0, 0.0)], Stroke::new(1.0f32, Color32::from_gray(70)));
    p.line_segment([to_s(0.0, 1.0), to_s(1.0, 1.0)], Stroke::new(1.0f32, Color32::from_gray(70)));
    let pts: Vec<Pos2> = (0..=60)
        .map(|i| {
            let t = i as f32 / 60.0;
            let v = if *curve == Curve::Stepped { if t < 1.0 { 0.0 } else { 1.0 } } else { curve.ease(t) };
            to_s(t, v)
        })
        .collect();
    p.add(Shape::line(pts, Stroke::new(2.0f32, ACCENT)));
    if let Curve::Bezier(c) = curve {
        let a0 = to_s(0.0, 0.0);
        let a1 = to_s(1.0, 1.0);
        for (k, anchor) in [(0usize, a0), (1usize, a1)] {
            let hp = to_s(c[k * 2], c[k * 2 + 1]);
            p.line_segment([anchor, hp], Stroke::new(1.0f32, Color32::from_gray(180)));
            let hr = Rect::from_center_size(hp, vec2(14.0, 14.0));
            let resp = ui.interact(hr, ui.id().with(("bz_handle", k)), Sense::drag());
            if resp.dragged() {
                if let Some(m) = resp.interact_pointer_pos() {
                    let (x, y) = from_s(m);
                    c[k * 2] = x.clamp(0.0, 1.0);
                    c[k * 2 + 1] = y.clamp(-1.0, 2.0);
                }
            }
            let col = if resp.hovered() || resp.dragged() { Color32::WHITE } else { SEL };
            p.circle_filled(hp, 5.0, col);
        }
    }
    *curve != before
}

pub fn draw_grid(p: &egui::Painter, rect: Rect, view: &View) {
    let mut step = 50.0;
    while step * view.zoom < 25.0 {
        step *= 2.0;
    }
    while step * view.zoom > 120.0 {
        step /= 2.0;
    }
    let tl = view.to_world(rect.left_top());
    let br = view.to_world(rect.right_bottom());
    let mut x = (tl[0] / step).floor() * step;
    while x <= br[0] {
        let sx = view.to_screen([x, 0.0]).x;
        p.line_segment([pos2(sx, rect.top()), pos2(sx, rect.bottom())], Stroke::new(1.0f32, Color32::from_white_alpha(8)));
        x += step;
    }
    let mut y = (br[1] / step).floor() * step;
    while y <= tl[1] {
        let sy = view.to_screen([0.0, y]).y;
        p.line_segment([pos2(rect.left(), sy), pos2(rect.right(), sy)], Stroke::new(1.0f32, Color32::from_white_alpha(8)));
        y += step;
    }
    let o = view.to_screen([0.0, 0.0]);
    p.line_segment([pos2(rect.left(), o.y), pos2(rect.right(), o.y)], Stroke::new(1.0f32, Color32::from_rgba_unmultiplied(220, 80, 80, 70)));
    p.line_segment([pos2(o.x, rect.top()), pos2(o.x, rect.bottom())], Stroke::new(1.0f32, Color32::from_rgba_unmultiplied(80, 220, 80, 70)));
}

pub fn draw_bone(p: &egui::Painter, view: &View, m: &Aff, b: &Bone, selected: bool, names: bool) {
    let o = view.to_screen(m.apply(0.0, 0.0));
    let tip = view.to_screen(m.apply(b.length, 0.0));
    let col = c32(b.color);
    let len_px = (tip - o).length();
    let outline = if selected { Stroke::new(2.0f32, SEL) } else { Stroke::new(1.0f32, Color32::from_black_alpha(200)) };
    let r = if len_px > 4.0 { 3.5 } else { 6.0 };
    if len_px > 4.0 {
        let dir = (tip - o) / len_px;
        let perp = vec2(-dir.y, dir.x);
        let w = (len_px * 0.12).clamp(3.0, 9.0);
        let mid = o + dir * (len_px * 0.18).min(18.0);
        let pts = vec![o, mid + perp * w, tip, mid - perp * w];
        let fill = if selected { SEL.gamma_multiply(0.85) } else { col.gamma_multiply(0.75) };
        p.add(Shape::convex_polygon(pts, fill, outline));
    } else {
        p.line_segment([o - vec2(10.0, 0.0), o + vec2(10.0, 0.0)], Stroke::new(1.0f32, col));
        p.line_segment([o - vec2(0.0, 10.0), o + vec2(0.0, 10.0)], Stroke::new(1.0f32, col));
    }
    p.circle_filled(o, r, if selected { SEL } else { col });
    p.circle_stroke(o, r, Stroke::new(1.0f32, Color32::from_black_alpha(200)));
    if names || selected {
        p.text(
            o.lerp(tip, 0.5) + vec2(8.0, -8.0),
            Align2::LEFT_BOTTOM,
            &b.name,
            FontId::proportional(12.0),
            if selected { SEL } else { Color32::from_gray(210) },
        );
    }
    if !b.visible {
        p.circle_stroke(o, 10.0, Stroke::new(1.0f32, Color32::from_gray(90)));
    }
}

pub fn dist_seg(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let l2 = ab.length_sq();
    if l2 < 1e-6 {
        return (p - a).length();
    }
    let t = ((p - a).dot(ab) / l2).clamp(0.0, 1.0);
    (p - (a + ab * t)).length()
}
