//! Cálculo de la pose: huesos con restricciones, física, estado de slots y mallas.

use crate::model::*;
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct SlotPose {
    pub att: Option<String>,
    pub color: [u8; 4],
}

pub struct Pose {
    pub world: HashMap<u32, Aff>,
    pub slots: HashMap<u32, SlotPose>,
    /// Orden de dibujo en este fotograma (ids de slots).
    pub draw_order: Vec<u32>,
}

/// Estado de la simulación física (vive en el editor, no en el proyecto).
#[derive(Default)]
pub struct Physics {
    pub states: HashMap<usize, PhysState>,
    /// Segundos a simular en esta llamada.
    pub dt: f32,
}

#[derive(Clone, Copy, Default)]
pub struct PhysState {
    pub tip: [f32; 2],
    pub vel: [f32; 2],
    pub base: [f32; 2],
    pub init: bool,
}

impl Physics {
    pub fn reset(&mut self) {
        self.states.clear();
    }
}

fn wrap(mut a: f32) -> f32 {
    while a > 180.0 {
        a -= 360.0;
    }
    while a < -180.0 {
        a += 360.0;
    }
    a
}

/// Inverso de `Aff::child`: valores locales que producen la transformación de mundo `w`
/// (copia de `Bone.updateAppliedTransform` de Spine).
pub fn applied_local(w: &Aff, parent: Option<&Aff>, inherit: Inherit, own_rot: f32) -> Local {
    let Some(p) = parent else {
        return Local {
            x: w.tx,
            y: w.ty,
            rot: w.c.atan2(w.a).to_degrees(),
            sx: (w.a * w.a + w.c * w.c).sqrt(),
            sy: (w.b * w.b + w.d * w.d).sqrt(),
            shx: 0.0,
            shy: (w.a * w.b + w.c * w.d).atan2(w.a * w.d - w.b * w.c).to_degrees(),
        };
    };
    let (mut pa, mut pb, mut pc, mut pd) = (p.a, p.b, p.c, p.d);
    let mut pid = 1.0 / (pa * pd - pb * pc);
    let (mut ia, mut ib, mut ic, mut id) = (pd * pid, pb * pid, pc * pid, pa * pid);
    let (dx, dy) = (w.tx - p.tx, w.ty - p.ty);
    let x = dx * ia - dy * ib;
    let y = dy * id - dx * ic;
    let (ra, rb, rc, rd);
    if inherit == Inherit::OnlyTranslation {
        ra = w.a;
        rb = w.b;
        rc = w.c;
        rd = w.d;
    } else {
        match inherit {
            Inherit::NoRotationOrReflection => {
                let s = (pa * pd - pb * pc).abs() / (pa * pa + pc * pc);
                pb = -pc * s;
                pd = pa * s;
                pid = 1.0 / (pa * pd - pb * pc);
                ia = pd * pid;
                ib = pb * pid;
            }
            Inherit::NoScale | Inherit::NoScaleOrReflection => {
                let (sn, cs) = own_rot.to_radians().sin_cos();
                pa = pa * cs + pb * sn;
                pc = pc * cs + pd * sn;
                let mut s = (pa * pa + pc * pc).sqrt();
                if s > 0.00001 {
                    s = 1.0 / s;
                }
                pa *= s;
                pc *= s;
                let mut s = (pa * pa + pc * pc).sqrt();
                if inherit == Inherit::NoScale && pid < 0.0 {
                    s = -s;
                }
                let r = std::f32::consts::FRAC_PI_2 + pc.atan2(pa);
                pb = r.cos() * s;
                pd = r.sin() * s;
                pid = 1.0 / (pa * pd - pb * pc);
                ia = pd * pid;
                ib = pb * pid;
                ic = pc * pid;
                id = pa * pid;
            }
            _ => {}
        }
        ra = ia * w.a - ib * w.c;
        rb = ia * w.b - ib * w.d;
        rc = id * w.c - ic * w.a;
        rd = id * w.d - ic * w.b;
    }
    let mut sx = (ra * ra + rc * rc).sqrt();
    let (sy, shy, rot);
    if sx > 0.0001 {
        let det = ra * rd - rb * rc;
        sy = det / sx;
        shy = -(ra * rb + rc * rd).atan2(det).to_degrees();
        rot = rc.atan2(ra).to_degrees();
    } else {
        sx = 0.0;
        sy = (rb * rb + rd * rd).sqrt();
        shy = 0.0;
        rot = 90.0 - rd.atan2(rb).to_degrees();
    }
    Local { x, y, rot, sx, sy, shx: 0.0, shy }
}

struct Solver<'a> {
    doc: &'a Doc,
    locals: HashMap<u32, Local>,
    inherit: HashMap<u32, Inherit>,
    world: HashMap<u32, Aff>,
    children: HashMap<u32, Vec<u32>>,
}

impl<'a> Solver<'a> {
    fn update_children(&mut self, id: u32) {
        let mut stack = vec![id];
        let mut guard = 0;
        while let Some(p) = stack.pop() {
            guard += 1;
            if guard > 100_000 {
                break;
            }
            let pw = self.world[&p];
            if let Some(ch) = self.children.get(&p).cloned() {
                for c in ch {
                    let l = self.locals[&c];
                    self.world.insert(c, Aff::child(&pw, &l, self.inherit[&c]));
                    stack.push(c);
                }
            }
        }
    }

    fn set_world(&mut self, id: u32, w: Aff) {
        self.world.insert(id, w);
        // guarda la transformación local "aplicada" para que, si un padre se vuelve a
        // calcular más tarde, este hueso conserve el efecto de la restricción (como Spine)
        let parent = self.doc.bone(id).and_then(|b| b.parent).and_then(|p| self.world.get(&p).copied());
        let inherit = self.inherit.get(&id).copied().unwrap_or_default();
        if let Some(l) = self.locals.get(&id).copied() {
            self.locals.insert(id, applied_local(&w, parent.as_ref(), inherit, l.rot));
        }
        self.update_children(id);
    }

    fn rotate_toward(&mut self, id: u32, target: [f32; 2], mix: f32) {
        let Some(w) = self.world.get(&id).copied() else { return };
        let want = (target[1] - w.ty).atan2(target[0] - w.tx).to_degrees();
        let mut cur = w.angle_deg();
        if w.det() < 0.0 {
            // con escala negativa el eje x apunta igual; nada que corregir
            cur = w.angle_deg();
        }
        let d = wrap(want - cur) * mix;
        self.set_world(id, w.rotated(d));
    }

    /// Equivale a `updateWorldTransformWith` de Spine: fija los valores locales y recalcula.
    fn set_local(&mut self, id: u32, l: Local) {
        let parent = self.doc.bone(id).and_then(|b| b.parent).and_then(|p| self.world.get(&p).copied()).unwrap_or(Aff::IDENTITY);
        let inherit = self.inherit.get(&id).copied().unwrap_or_default();
        self.world.insert(id, Aff::child(&parent, &l, inherit));
        self.locals.insert(id, l);
        self.update_children(id);
    }

    fn parent_world(&self, id: u32) -> Aff {
        self.doc.bone(id).and_then(|b| b.parent).and_then(|p| self.world.get(&p).copied()).unwrap_or(Aff::IDENTITY)
    }

    /// IK de un hueso (copia de `IkConstraint.apply1` de Spine).
    fn ik1(&mut self, bone: u32, t: [f32; 2], compress: bool, stretch: bool, uniform: bool, alpha: f32) {
        let (Some(w), Some(l)) = (self.world.get(&bone).copied(), self.locals.get(&bone).copied()) else { return };
        let p = self.parent_world(bone);
        let (pa, mut pb, pc, mut pd) = (p.a, p.b, p.c, p.d);
        let inherit = self.inherit.get(&bone).copied().unwrap_or_default();
        let mut rot_ik = -l.shx - l.rot;
        let (mut tx, mut ty);
        if inherit == Inherit::OnlyTranslation {
            tx = t[0] - w.tx;
            ty = t[1] - w.ty;
        } else {
            if inherit == Inherit::NoRotationOrReflection {
                let s = (pa * pd - pb * pc).abs() / (pa * pa + pc * pc).max(0.0001);
                pb = -pc * s;
                pd = pa * s;
                rot_ik += pc.atan2(pa).to_degrees();
            }
            let (x, y) = (t[0] - p.tx, t[1] - p.ty);
            let d = pa * pd - pb * pc;
            if d.abs() <= 0.0001 {
                tx = 0.0;
                ty = 0.0;
            } else {
                tx = (x * pd - y * pb) / d - l.x;
                ty = (y * pa - x * pc) / d - l.y;
            }
        }
        rot_ik += ty.atan2(tx).to_degrees();
        if l.sx < 0.0 {
            rot_ik += 180.0;
        }
        rot_ik = wrap(rot_ik);
        let (mut sx, mut sy) = (l.sx, l.sy);
        if compress || stretch {
            if matches!(inherit, Inherit::NoScale | Inherit::NoScaleOrReflection) {
                tx = t[0] - w.tx;
                ty = t[1] - w.ty;
            }
            let b = self.doc.bone(bone).map(|b| b.length).unwrap_or(0.0) * sx;
            if b > 0.0001 {
                let dd = tx * tx + ty * ty;
                if (compress && dd < b * b) || (stretch && dd > b * b) {
                    let s = (dd.sqrt() / b - 1.0) * alpha + 1.0;
                    sx *= s;
                    if uniform {
                        sy *= s;
                    }
                }
            }
        }
        self.set_local(bone, Local { rot: l.rot + rot_ik * alpha, sx, sy, ..l });
    }

    /// IK de dos huesos (copia de `IkConstraint.apply2` de Spine).
    #[allow(clippy::too_many_arguments)]
    fn ik2(&mut self, parent: u32, child: u32, t: [f32; 2], bend_positive: bool, stretch: bool, uniform: bool, softness: f32, alpha: f32) {
        let inh = |id: u32| self.inherit.get(&id).copied().unwrap_or_default();
        if inh(parent) != Inherit::Normal || inh(child) != Inherit::Normal {
            return;
        }
        let (Some(pw), Some(pl), Some(cl)) = (self.world.get(&parent).copied(), self.locals.get(&parent).copied(), self.locals.get(&child).copied()) else { return };
        let bend_dir = if bend_positive { 1.0 } else { -1.0 };
        let (px, py) = (pl.x, pl.y);
        let (mut psx, mut psy) = (pl.sx, pl.sy);
        let (mut sx, mut sy) = (psx, psy);
        let mut csx = cl.sx;
        let (os1, mut s2) = if psx < 0.0 {
            psx = -psx;
            (180.0, -1.0)
        } else {
            (0.0, 1.0)
        };
        if psy < 0.0 {
            psy = -psy;
            s2 = -s2;
        }
        let os2 = if csx < 0.0 {
            csx = -csx;
            180.0
        } else {
            0.0
        };
        let cx = cl.x;
        let u = (psx - psy).abs() <= 0.0001;
        let (cy, cwx, cwy) = if !u || stretch {
            (0.0, pw.a * cx + pw.tx, pw.c * cx + pw.ty)
        } else {
            let cy = cl.y;
            (cy, pw.a * cx + pw.b * cy + pw.tx, pw.c * cx + pw.d * cy + pw.ty)
        };
        let pp = self.parent_world(parent);
        let (a, b, c, d) = (pp.a, pp.b, pp.c, pp.d);
        let mut id = a * d - b * c;
        id = if id.abs() <= 0.0001 { 0.0 } else { 1.0 / id };
        let (x, y) = (cwx - pp.tx, cwy - pp.ty);
        let dx = (x * d - y * b) * id - px;
        let dy = (y * a - x * c) * id - py;
        let l1 = (dx * dx + dy * dy).sqrt();
        let mut l2 = self.doc.bone(child).map(|b| b.length).unwrap_or(0.0) * csx;
        if l1 < 0.0001 {
            self.ik1(parent, t, false, stretch, false, alpha);
            let pw2 = self.world.get(&parent).copied().unwrap_or(pw);
            let _ = pw2;
            self.set_local(child, Local { x: cx, y: cy, ..cl });
            return;
        }
        let (x, y) = (t[0] - pp.tx, t[1] - pp.ty);
        let mut tx = (x * d - y * b) * id - px;
        let mut ty = (y * a - x * c) * id - py;
        let mut dd = tx * tx + ty * ty;
        if softness != 0.0 {
            let soft = softness * psx * (csx + 1.0) * 0.5;
            let td = dd.sqrt();
            let sd = td - l1 - l2 * psx + soft;
            if sd > 0.0 {
                let q = (sd / (soft * 2.0)).min(1.0) - 1.0;
                let q = (sd - soft * (1.0 - q * q)) / td;
                tx -= q * tx;
                ty -= q * ty;
                dd = tx * tx + ty * ty;
            }
        }
        let (a1, a2);
        if u {
            l2 *= psx;
            let mut cos = (dd - l1 * l1 - l2 * l2) / (2.0 * l1 * l2);
            let aa2;
            if cos < -1.0 {
                cos = -1.0;
                aa2 = std::f32::consts::PI * bend_dir;
            } else if cos > 1.0 {
                cos = 1.0;
                aa2 = 0.0;
                if stretch {
                    let f = (dd.sqrt() / (l1 + l2) - 1.0) * alpha + 1.0;
                    sx *= f;
                    if uniform {
                        sy *= f;
                    }
                }
            } else {
                aa2 = cos.acos() * bend_dir;
            }
            let aa = l1 + l2 * cos;
            let bb = l2 * aa2.sin();
            a1 = (ty * aa - tx * bb).atan2(tx * aa + ty * bb);
            a2 = aa2;
        } else {
            let aa0 = psx * l2;
            let bb0 = psy * l2;
            let (aa, bb, ta) = (aa0 * aa0, bb0 * bb0, ty.atan2(tx));
            let cc = bb * l1 * l1 + aa * dd - aa * bb;
            let c1 = -2.0 * bb * l1;
            let c2 = bb - aa;
            let disc = c1 * c1 - 4.0 * c2 * cc;
            let mut solved = None;
            if disc >= 0.0 {
                let mut q = disc.sqrt();
                if c1 < 0.0 {
                    q = -q;
                }
                q = -(c1 + q) * 0.5;
                let (r0, r1) = (q / c2, cc / q);
                let r = if r0.abs() < r1.abs() { r0 } else { r1 };
                let r0 = dd - r * r;
                if r0 >= 0.0 {
                    let yy = r0.sqrt() * bend_dir;
                    solved = Some((ta - yy.atan2(r), (yy / psy).atan2((r - l1) / psx)));
                }
            }
            match solved {
                Some((x1, x2)) => {
                    a1 = x1;
                    a2 = x2;
                }
                None => {
                    let (mut min_angle, mut min_x, mut min_y) = (std::f32::consts::PI, l1 - aa0, 0.0f32);
                    let mut min_dist = min_x * min_x;
                    let (mut max_angle, mut max_x, mut max_y) = (0.0f32, l1 + aa0, 0.0f32);
                    let mut max_dist = max_x * max_x;
                    let cc = -aa0 * l1 / (aa - bb);
                    if (-1.0..=1.0).contains(&cc) {
                        let cc = cc.acos();
                        let xx = aa0 * cc.cos() + l1;
                        let yy = bb0 * cc.sin();
                        let dd2 = xx * xx + yy * yy;
                        if dd2 < min_dist {
                            min_angle = cc;
                            min_dist = dd2;
                            min_x = xx;
                            min_y = yy;
                        }
                        if dd2 > max_dist {
                            max_angle = cc;
                            max_dist = dd2;
                            max_x = xx;
                            max_y = yy;
                        }
                    }
                    if dd <= (min_dist + max_dist) * 0.5 {
                        a1 = ta - (min_y * bend_dir).atan2(min_x);
                        a2 = min_angle * bend_dir;
                    } else {
                        a1 = ta - (max_y * bend_dir).atan2(max_x);
                        a2 = max_angle * bend_dir;
                    }
                }
            }
        }
        let os = cy.atan2(cx) * s2;
        let rotation = pl.rot;
        let a1 = wrap((a1 - os).to_degrees() + os1 - rotation);
        self.set_local(parent, Local { x: px, y: py, rot: rotation + a1 * alpha, sx, sy, shx: 0.0, shy: 0.0 });
        let rotation = cl.rot;
        let a2 = wrap(((a2 + os).to_degrees() - cl.shx) * s2 + os2 - rotation);
        self.set_local(child, Local { x: cx, y: cy, rot: rotation + a2 * alpha, ..cl });
    }

    #[allow(clippy::too_many_arguments)]
    fn ik(&mut self, bones: &[u32], target: u32, mix: f32, bend_positive: bool, softness: f32, stretch: bool, compress: bool, uniform: bool) {
        if mix == 0.0 {
            return;
        }
        let Some(t) = self.world.get(&target).map(|w| w.origin()) else { return };
        match bones {
            [one] => self.ik1(*one, t, compress, stretch, uniform, mix),
            [p, c, ..] => {
                let direct = self.doc.bone(*c).and_then(|b| b.parent) == Some(*p);
                if direct {
                    self.ik2(*p, *c, t, bend_positive, stretch, uniform, softness, mix);
                } else {
                    self.ik1(*p, t, false, false, false, mix);
                    self.ik1(*c, t, compress, stretch, uniform, mix);
                }
            }
            _ => {}
        }
    }

    /// Restricción de transformación en espacio de mundo (como la de Spine).
    fn transform(&mut self, bones: &[u32], target: u32, k: &ConstraintKind) {
        let ConstraintKind::Transform { mix_rotate, mix_translate, mix_scale, off_rot, off_x, off_y, off_sx, off_sy, mix_y, mix_scale_y, mix_shear, off_shy, .. } = k else { return };
        let Some(tw) = self.world.get(&target).copied() else { return };
        let (mr, mx, my) = (*mix_rotate, *mix_translate, mix_y.unwrap_or(*mix_translate));
        let (msx, msy, msh) = (*mix_scale, mix_scale_y.unwrap_or(*mix_scale), *mix_shear);
        let reflect = if tw.det() > 0.0 { 1.0 } else { -1.0 };
        let orot = off_rot * reflect;
        let oshy = off_shy * reflect;
        let tp = tw.apply(*off_x, *off_y);
        for &b in bones {
            let Some(mut w) = self.world.get(&b).copied() else { continue };
            if mr != 0.0 {
                let r = wrap(tw.c.atan2(tw.a).to_degrees() - w.c.atan2(w.a).to_degrees() + orot) * mr;
                w = w.rotated(r);
            }
            if mx != 0.0 || my != 0.0 {
                w.tx += (tp[0] - w.tx) * mx;
                w.ty += (tp[1] - w.ty) * my;
            }
            if msx != 0.0 {
                let s0 = w.scale_x();
                if s0 > 1e-6 {
                    let f = (s0 + (tw.scale_x() - s0 + off_sx) * msx) / s0;
                    w.a *= f;
                    w.c *= f;
                }
            }
            if msy != 0.0 {
                let s0 = w.scale_y();
                if s0 > 1e-6 {
                    let f = (s0 + (tw.scale_y() - s0 + off_sy) * msy) / s0;
                    w.b *= f;
                    w.d *= f;
                }
            }
            if msh > 0.0 {
                let by = w.d.atan2(w.b);
                let mut r = tw.d.atan2(tw.b) - tw.c.atan2(tw.a) - (by - w.c.atan2(w.a));
                r = wrap(r.to_degrees()).to_radians();
                let r = by + (r + oshy.to_radians()) * msh;
                let s0 = w.scale_y();
                w.b = r.cos() * s0;
                w.d = r.sin() * s0;
            }
            self.set_world(b, w);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn path(&mut self, bones: &[u32], slot: u32, skin: usize, slots: &HashMap<u32, SlotPose>, c: (f32, f32, SpacingMode, RotateMode, f32, f32, f32)) {
        let (position, spacing, smode, rmode, orot, mr, mt) = c;
        let Some(s) = self.doc.slot(slot) else { return };
        let Some(name) = slots.get(&slot).and_then(|p| p.att.clone()) else { return };
        let Some((_, att)) = self.doc.find_att(skin, slot, &name) else { return };
        let Some(path) = &att.path else { return };
        let Some(bw) = self.world.get(&s.bone).copied() else { return };
        let segs = path.segments(|p| bw.apply(p[0], p[1]));
        if segs.is_empty() {
            return;
        }
        // polilínea con longitudes acumuladas (y el parámetro de cada muestra)
        const N: usize = 48;
        let mut pts: Vec<[f32; 2]> = vec![segs[0][0]];
        let mut par: Vec<(usize, f32)> = vec![(0, 0.0)];
        let mut acc: Vec<f32> = vec![0.0];
        for (si, sg) in segs.iter().enumerate() {
            for i in 1..=N {
                let t = i as f32 / N as f32;
                let p = bezier_pt(sg, t);
                let last = *pts.last().unwrap();
                let l = ((p[0] - last[0]).powi(2) + (p[1] - last[1]).powi(2)).sqrt();
                acc.push(acc.last().unwrap() + l);
                pts.push(p);
                par.push((si, t));
            }
        }
        let total = *acc.last().unwrap();
        if total < 1e-4 {
            return;
        }
        let deriv = |sg: &[[f32; 2]; 4], t: f32| -> [f32; 2] {
            let u = 1.0 - t;
            let d = |k: usize| 3.0 * u * u * (sg[1][k] - sg[0][k]) + 6.0 * u * t * (sg[2][k] - sg[1][k]) + 3.0 * t * t * (sg[3][k] - sg[2][k]);
            [d(0), d(1)]
        };
        let at = |mut d: f32| -> ([f32; 2], f32) {
            if path.closed {
                d = d.rem_euclid(total);
            } else {
                d = d.clamp(0.0, total);
            }
            let i = acc.partition_point(|v| *v < d).clamp(1, pts.len() - 1);
            let span = (acc[i] - acc[i - 1]).max(1e-6);
            let f = ((d - acc[i - 1]) / span).clamp(0.0, 1.0);
            let (s0, t0) = par[i - 1];
            let (s1, t1) = par[i];
            let (si, t) = if s0 == s1 { (s1, t0 + (t1 - t0) * f) } else { (s1, t1 * f) };
            let sg = &segs[si];
            let p = bezier_pt(sg, t);
            let dv = deriv(sg, t);
            (p, dv[1].atan2(dv[0]).to_degrees())
        };
        let mut dist = position * total;
        let mut placed: Vec<[f32; 2]> = Vec::new();
        for (i, &b) in bones.iter().enumerate() {
            let (p, tang) = at(dist);
            placed.push(p);
            let Some(mut w) = self.world.get(&b).copied() else { continue };
            w.tx += (p[0] - w.tx) * mt;
            w.ty += (p[1] - w.ty) * mt;
            let blen = self.doc.bone(b).map(|bb| bb.length).unwrap_or(0.0) * w.scale_x();
            let step = match smode {
                SpacingMode::Length => blen + spacing,
                SpacingMode::Fixed => spacing,
                SpacingMode::Percent => spacing * total,
            };
            let ang = match rmode {
                RotateMode::Tangent => tang,
                RotateMode::Chain => {
                    let (np, _) = at(dist + step.max(1e-3));
                    (np[1] - p[1]).atan2(np[0] - p[0]).to_degrees()
                }
            } + orot;
            if mr != 0.0 {
                w = w.rotated(wrap(ang - w.angle_deg()) * mr);
            }
            self.set_world(b, w);
            dist += step;
            let _ = i;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn physics_step(st: &mut PhysState, base: [f32; 2], target_tip: [f32; 2], dt: f32, inertia: f32, strength: f32, damping: f32, mass: f32, gravity: f32, wind: f32) {
    if !st.init {
        st.tip = target_tip;
        st.vel = [0.0, 0.0];
        st.base = base;
        st.init = true;
        return;
    }
    // la punta conserva parte de su posición cuando la base se mueve (inercia)
    let mv = [base[0] - st.base[0], base[1] - st.base[1]];
    st.tip[0] += mv[0] * (1.0 - inertia);
    st.tip[1] += mv[1] * (1.0 - inertia);
    st.base = base;
    let m = mass.max(0.01);
    let steps = ((dt / (1.0 / 120.0)).ceil() as usize).clamp(1, 32);
    let h = dt / steps as f32;
    for _ in 0..steps {
        let fx = strength * (target_tip[0] - st.tip[0]) - damping * st.vel[0] + wind * 10.0;
        let fy = strength * (target_tip[1] - st.tip[1]) - damping * st.vel[1] - gravity * 10.0;
        st.vel[0] += fx / m * h;
        st.vel[1] += fy / m * h;
        st.tip[0] += st.vel[0] * h;
        st.tip[1] += st.vel[1] * h;
    }
    if !(st.tip[0].is_finite() && st.tip[1].is_finite()) {
        st.init = false;
    }
}

/// Calcula la pose completa. `skin` es el índice de la skin activa.
pub fn solve(doc: &Doc, anim: Option<&Animation>, frame: f32, skin: usize, mut phys: Option<&mut Physics>) -> Pose {
    let order = doc.bone_order();
    let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
    let mut locals = HashMap::new();
    for b in &doc.bones {
        locals.insert(b.id, doc.local_pose(b, anim, frame));
        if let Some(p) = b.parent {
            children.entry(p).or_default().push(b.id);
        }
    }
    let mut world = HashMap::new();
    let mut inherit = HashMap::new();
    for id in &order {
        let b = doc.bone(*id).unwrap();
        inherit.insert(*id, b.inherit);
        let parent = b.parent.and_then(|p| world.get(&p).copied()).unwrap_or(Aff::IDENTITY);
        world.insert(*id, Aff::child(&parent, &locals[id], b.inherit));
    }

    // estado de slots
    let mut slots = HashMap::new();
    for s in &doc.slots {
        let mut att = s.attachment.clone();
        let mut color = s.color;
        if let Some(a) = anim {
            if let Some(n) = a.att_at(s.id, frame) {
                att = n;
            }
            if let Some(c) = a.color_at(s.id, frame) {
                color = c;
            }
        }
        slots.insert(s.id, SlotPose { att, color });
    }

    let mut draw_order = anim.and_then(|a| a.draw_at(frame)).cloned().unwrap_or_else(|| doc.draw_order.clone());
    for s in &doc.draw_order {
        if !draw_order.contains(s) {
            draw_order.push(*s); // slots creados después de la clave
        }
    }
    let mut sv = Solver { doc, locals, inherit, world, children };
    for (ci, c0) in doc.constraints.iter().enumerate() {
        if !c0.enabled || c0.bones.is_empty() {
            continue;
        }
        let animated;
        let c = match anim.and_then(|a| a.con_at(&c0.name, frame)) {
            Some((v, bend)) => {
                let mut cc = c0.clone();
                cc.set_anim_values(&v, bend);
                animated = cc;
                &animated
            }
            None => c0,
        };
        match &c.kind {
            ConstraintKind::Ik { target, mix, bend_positive, softness, stretch, compress, uniform } => sv.ik(&c.bones, *target, *mix, *bend_positive, *softness, *stretch, *compress, *uniform),
            ConstraintKind::Transform { target, .. } => sv.transform(&c.bones, *target, &c.kind),
            ConstraintKind::Path { slot, position, spacing, spacing_mode, rotate_mode, off_rot, mix_rotate, mix_translate } => sv.path(
                &c.bones,
                *slot,
                skin,
                &slots,
                (*position, *spacing, *spacing_mode, *rotate_mode, *off_rot, *mix_rotate, *mix_translate),
            ),
            ConstraintKind::Physics { inertia, strength, damping, mass, gravity, wind, mix } => {
                let Some(ph) = phys.as_deref_mut() else { continue };
                let b = c.bones[0];
                let Some(w) = sv.world.get(&b).copied() else { continue };
                let len = doc.bone(b).map(|bb| bb.length).unwrap_or(0.0).max(1.0);
                let base = w.origin();
                let tip = w.apply(len, 0.0);
                let dt = ph.dt;
                let st = ph.states.entry(ci).or_default();
                if dt > 0.0 {
                    physics_step(st, base, tip, dt, *inertia, *strength, *damping, *mass, *gravity, *wind);
                } else if !st.init {
                    physics_step(st, base, tip, 0.0, *inertia, *strength, *damping, *mass, *gravity, *wind);
                }
                if st.init {
                    let st = *st;
                    sv.rotate_toward(b, [base[0] + (st.tip[0] - base[0]), base[1] + (st.tip[1] - base[1])], *mix);
                }
            }
        }
    }
    Pose { world: sv.world, slots, draw_order }
}

/// Posiciones de mundo de los vértices de una malla, con pesos y deformación.
pub fn mesh_world(
    doc: &Doc,
    world: &HashMap<u32, Aff>,
    setup: &HashMap<u32, Aff>,
    slot: &Slot,
    mesh: &MeshData,
    deform: Option<&[[f32; 2]]>,
) -> Vec<[f32; 2]> {
    let d = |i: usize| deform.and_then(|d| d.get(i)).copied().unwrap_or([0.0, 0.0]);
    let sb = world.get(&slot.bone).copied().unwrap_or(Aff::IDENTITY);
    if !mesh.weighted() {
        return mesh.verts.iter().enumerate().map(|(i, v)| {
            let o = d(i);
            sb.apply(v[0] + o[0], v[1] + o[1])
        }).collect();
    }
    // desplazamiento en espacio de malla -> espacio de cada hueso (en pose base)
    let sbs = setup.get(&slot.bone).copied().unwrap_or(Aff::IDENTITY);
    let _ = doc;
    mesh.weights
        .iter()
        .enumerate()
        .map(|(i, ws)| {
            let o = d(i);
            let ow = sbs.apply_vec(o[0], o[1]);
            let mut p = [0.0f32, 0.0];
            for w in ws {
                let bw = world.get(&w.bone).copied().unwrap_or(Aff::IDENTITY);
                let bs = setup.get(&w.bone).copied().unwrap_or(Aff::IDENTITY);
                let ol = bs.inv_vec(ow[0], ow[1]);
                let q = bw.apply(w.x + ol[0], w.y + ol[1]);
                p[0] += q[0] * w.w;
                p[1] += q[1] * w.w;
            }
            p
        })
        .collect()
}
