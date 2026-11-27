//! Exportar e importar el formato JSON de Spine (4.2) con atlas de texturas.
//!
//! Exportar escribe `nombre.json`, `nombre.atlas` y `nombre.png` (las imágenes y las
//! formas de color empaquetadas en una sola textura). Importar lee el JSON de Spine
//! (3.8 o 4.x) y busca las imágenes en la carpeta `images` o dentro del `.atlas`.

use crate::model::*;
use image::RgbaImage;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn hex(c: [u8; 4]) -> String {
    format!("{:02x}{:02x}{:02x}{:02x}", c[0], c[1], c[2], c[3])
}

fn parse_hex(s: &str) -> [u8; 4] {
    let s = s.trim_start_matches('#');
    let b = |i: usize| u8::from_str_radix(s.get(i..i + 2).unwrap_or("ff"), 16).unwrap_or(255);
    [b(0), b(2), b(4), if s.len() >= 8 { b(6) } else { 255 }]
}

fn r3(v: f32) -> Value {
    let v = (v * 1000.0).round() / 1000.0;
    json!(if v == 0.0 { 0.0 } else { v })
}

fn safe_name(s: &str) -> String {
    s.chars().map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c } else { '_' }).collect()
}

// ---------------------------------------------------------------------------
// Atlas
// ---------------------------------------------------------------------------

struct Packed {
    name: String,
    img: RgbaImage,
    x: u32,
    y: u32,
}

fn pack(mut items: Vec<(String, RgbaImage)>) -> (Vec<Packed>, u32, u32) {
    items.sort_by(|a, b| b.1.height().cmp(&a.1.height()));
    let pad = 2;
    let total: u64 = items.iter().map(|(_, i)| (i.width() + pad) as u64 * (i.height() + pad) as u64).sum();
    let widest = items.iter().map(|(_, i)| i.width() + pad * 2).max().unwrap_or(16);
    let mut w = ((total as f64).sqrt() as u32).max(widest).next_power_of_two().min(8192).max(widest);
    loop {
        let mut out = Vec::new();
        let (mut x, mut y, mut row_h) = (pad, pad, 0);
        for (n, img) in &items {
            if x + img.width() + pad > w {
                x = pad;
                y += row_h + pad;
                row_h = 0;
            }
            out.push(Packed { name: n.clone(), img: img.clone(), x, y });
            x += img.width() + pad;
            row_h = row_h.max(img.height());
        }
        let h = (y + row_h + pad).next_power_of_two();
        if h <= w * 2 || w >= 8192 {
            return (out, w, h.max(16));
        }
        w *= 2;
    }
}

/// Dibuja una forma de color blanco (el color va en el adjunto).
fn raster_shape(kind: AttachKind, w: f32, h: f32) -> RgbaImage {
    let s = (256.0 / w.max(h).max(1.0)).min(1.0) * 2.0;
    let (iw, ih) = (((w * s).round() as u32).clamp(2, 512), ((h * s).round() as u32).clamp(2, 512));
    let mut img = RgbaImage::new(iw, ih);
    for y in 0..ih {
        for x in 0..iw {
            let a = match kind {
                AttachKind::Ellipse => {
                    let dx = (x as f32 + 0.5) / iw as f32 * 2.0 - 1.0;
                    let dy = (y as f32 + 0.5) / ih as f32 * 2.0 - 1.0;
                    let d = (dx * dx + dy * dy).sqrt();
                    ((1.0 - d) * iw.min(ih) as f32 * 0.5 + 0.5).clamp(0.0, 1.0)
                }
                _ => 1.0,
            };
            img.put_pixel(x, y, image::Rgba([255, 255, 255, (a * 255.0) as u8]));
        }
    }
    img
}

// ---------------------------------------------------------------------------
// Exportar
// ---------------------------------------------------------------------------

fn curve_json(c: Curve, ta: f32, tb: f32, vals: &[(f32, f32)]) -> Option<Value> {
    match c {
        Curve::Linear => None,
        Curve::Stepped => Some(json!("stepped")),
        Curve::BezierAbs(b) => {
            let mut out = Vec::new();
            for (va, _) in vals {
                out.push(r3(ta + b[0] * (tb - ta)));
                out.push(r3(va + b[1]));
                out.push(r3(ta + b[2] * (tb - ta)));
                out.push(r3(va + b[3]));
            }
            Some(Value::Array(out))
        }
        _ => {
            let b = c.as_bezier()?;
            let mut out = Vec::new();
            for (va, vb) in vals {
                out.push(r3(ta + b[0] * (tb - ta)));
                out.push(r3(va + b[1] * (vb - va)));
                out.push(r3(ta + b[2] * (tb - ta)));
                out.push(r3(va + b[3] * (vb - va)));
            }
            Some(Value::Array(out))
        }
    }
}

fn path_lengths(p: &PathData) -> Vec<f32> {
    let mut lengths = Vec::new();
    let mut acc = 0.0;
    for sg in p.segments(|q| q) {
        let mut last = sg[0];
        for i in 1..=32 {
            let q = bezier_pt(&sg, i as f32 / 32.0);
            acc += ((q[0] - last[0]).powi(2) + (q[1] - last[1]).powi(2)).sqrt();
            last = q;
        }
        lengths.push(acc);
    }
    lengths
}

pub fn export(doc: &Doc, path: &Path) -> Result<usize, String> {
    let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "esqueleto".into());
    let dir = path.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("."));
    let bone_name = |id: u32| doc.bone(id).map(|b| b.name.clone()).unwrap_or_default();
    let slot_name = |id: u32| doc.slot(id).map(|s| s.name.clone()).unwrap_or_default();
    let order = doc.bone_order();
    let bone_index: HashMap<u32, usize> = order.iter().enumerate().map(|(i, b)| (*b, i)).collect();
    let setup = doc.world_fk(None, 0.0);

    // ---- huesos
    let mut bones = Vec::new();
    for id in &order {
        let b = doc.bone(*id).unwrap();
        let mut o = Map::new();
        o.insert("name".into(), json!(b.name));
        if let Some(p) = b.parent {
            o.insert("parent".into(), json!(bone_name(p)));
        }
        if b.length != 0.0 {
            o.insert("length".into(), r3(b.length));
        }
        if b.rotation != 0.0 {
            o.insert("rotation".into(), r3(b.rotation));
        }
        if b.x != 0.0 {
            o.insert("x".into(), r3(b.x));
        }
        if b.y != 0.0 {
            o.insert("y".into(), r3(b.y));
        }
        if b.sx != 1.0 {
            o.insert("scaleX".into(), r3(b.sx));
        }
        if b.sy != 1.0 {
            o.insert("scaleY".into(), r3(b.sy));
        }
        if b.shx != 0.0 {
            o.insert("shearX".into(), r3(b.shx));
        }
        if b.shy != 0.0 {
            o.insert("shearY".into(), r3(b.shy));
        }
        if b.inherit != Inherit::Normal {
            o.insert("inherit".into(), json!(b.inherit.spine()));
        }
        o.insert("color".into(), json!(hex(b.color)));
        bones.push(Value::Object(o));
    }

    // ---- slots (en orden de dibujo)
    let mut slots = Vec::new();
    for sid in &doc.draw_order {
        let Some(s) = doc.slot(*sid) else { continue };
        let mut o = Map::new();
        o.insert("name".into(), json!(s.name));
        o.insert("bone".into(), json!(bone_name(s.bone)));
        if s.color != [255; 4] {
            o.insert("color".into(), json!(hex(s.color)));
        }
        if let Some(a) = &s.attachment {
            o.insert("attachment".into(), json!(a));
        }
        slots.push(Value::Object(o));
    }

    // ---- regiones del atlas
    let mut regions: Vec<(String, RgbaImage)> = Vec::new();
    let mut by_image: HashMap<String, String> = HashMap::new();
    let mut region_for = |att: &Attachment, slot: &str, name: &str| -> Option<String> {
        if !att.image.is_empty() {
            if let Some(r) = by_image.get(&att.image) {
                return Some(r.clone());
            }
            let img = image::open(&att.image).ok()?.to_rgba8();
            let base = Path::new(&att.image).file_stem().map(|s| safe_name(&s.to_string_lossy())).unwrap_or_else(|| "imagen".into());
            let n = unique(&base, |x| regions.iter().any(|(r, _)| r == x));
            regions.push((n.clone(), img));
            by_image.insert(att.image.clone(), n.clone());
            return Some(n);
        }
        let kind = if att.kind == AttachKind::Ellipse { AttachKind::Ellipse } else { AttachKind::Rect };
        let base = format!("{}_{}", safe_name(slot), safe_name(name));
        let n = unique(&base, |x| regions.iter().any(|(r, _)| r == x));
        regions.push((n.clone(), raster_shape(kind, att.w, att.h)));
        Some(n)
    };

    // ---- skins
    let mut skins = Vec::new();
    for sk in &doc.skins {
        let mut atts: Map<String, Value> = Map::new();
        for e in &sk.entries {
            let Some(slot) = doc.slot(e.slot) else { continue };
            let a = &e.att;
            let mut o = Map::new();
            match a.kind {
                AttachKind::Rect | AttachKind::Ellipse | AttachKind::Image => {
                    let Some(r) = region_for(a, &slot.name, &e.name) else { continue };
                    if r != e.name {
                        o.insert("path".into(), json!(r));
                    }
                    o.insert("x".into(), r3(a.ox));
                    o.insert("y".into(), r3(a.oy));
                    if a.rot != 0.0 {
                        o.insert("rotation".into(), r3(a.rot));
                    }
                    o.insert("width".into(), r3(a.w));
                    o.insert("height".into(), r3(a.h));
                    if a.tint != [255; 4] {
                        o.insert("color".into(), json!(hex(a.tint)));
                    }
                }
                AttachKind::Mesh => {
                    let Some(m) = &a.mesh else { continue };
                    let Some(r) = region_for(a, &slot.name, &e.name) else { continue };
                    o.insert("type".into(), json!("mesh"));
                    if r != e.name {
                        o.insert("path".into(), json!(r));
                    }
                    let uvs: Vec<Value> = m.uvs.iter().flat_map(|u| [r3(u[0]), r3(u[1])]).collect();
                    let tris: Vec<Value> = m.tris.iter().flat_map(|t| t.iter().map(|i| json!(i))).collect();
                    let mut verts: Vec<Value> = Vec::new();
                    if m.weighted() {
                        for ws in &m.weights {
                            verts.push(json!(ws.len()));
                            for w in ws {
                                verts.push(json!(bone_index.get(&w.bone).copied().unwrap_or(0)));
                                verts.push(r3(w.x));
                                verts.push(r3(w.y));
                                verts.push(r3(w.w));
                            }
                        }
                    } else {
                        for v in &m.verts {
                            verts.push(r3(v[0]));
                            verts.push(r3(v[1]));
                        }
                    }
                    o.insert("uvs".into(), Value::Array(uvs));
                    o.insert("triangles".into(), Value::Array(tris));
                    o.insert("vertices".into(), Value::Array(verts));
                    o.insert("hull".into(), json!(m.hull));
                    o.insert("width".into(), r3(a.w));
                    o.insert("height".into(), r3(a.h));
                    if a.tint != [255; 4] {
                        o.insert("color".into(), json!(hex(a.tint)));
                    }
                }
                AttachKind::Path => {
                    let Some(p) = &a.path else { continue };
                    o.insert("type".into(), json!("path"));
                    if p.closed {
                        o.insert("closed".into(), json!(true));
                    }
                    let mut verts = Vec::new();
                    for pt in &p.pts {
                        for q in [[pt.p[0] + pt.hin[0], pt.p[1] + pt.hin[1]], pt.p, [pt.p[0] + pt.hout[0], pt.p[1] + pt.hout[1]]] {
                            verts.push(r3(q[0]));
                            verts.push(r3(q[1]));
                        }
                    }
                    o.insert("lengths".into(), Value::Array(path_lengths(p).into_iter().map(r3).collect()));
                    o.insert("vertexCount".into(), json!(p.pts.len() * 3));
                    o.insert("vertices".into(), Value::Array(verts));
                    o.insert("color".into(), json!(hex(a.tint)));
                }
                AttachKind::BoundingBox | AttachKind::Clipping => {
                    let Some(m) = &a.mesh else { continue };
                    o.insert("type".into(), json!(if a.kind == AttachKind::Clipping { "clipping" } else { "boundingbox" }));
                    if let Some(end) = a.end_slot.and_then(|s| doc.slot(s)) {
                        o.insert("end".into(), json!(end.name));
                    }
                    o.insert("vertexCount".into(), json!(m.verts.len()));
                    let mut verts: Vec<Value> = Vec::new();
                    if m.weighted() {
                        for ws in &m.weights {
                            verts.push(json!(ws.len()));
                            for w in ws {
                                verts.push(json!(bone_index.get(&w.bone).copied().unwrap_or(0)));
                                verts.push(r3(w.x));
                                verts.push(r3(w.y));
                                verts.push(r3(w.w));
                            }
                        }
                    } else {
                        for v in &m.verts {
                            verts.push(r3(v[0]));
                            verts.push(r3(v[1]));
                        }
                    }
                    o.insert("vertices".into(), Value::Array(verts));
                    o.insert("color".into(), json!(hex(a.tint)));
                }
                AttachKind::Point => {
                    o.insert("type".into(), json!("point"));
                    o.insert("x".into(), r3(a.ox));
                    o.insert("y".into(), r3(a.oy));
                    o.insert("rotation".into(), r3(a.rot));
                    o.insert("color".into(), json!(hex(a.tint)));
                }
            }
            atts.entry(slot.name.clone())
                .or_insert_with(|| Value::Object(Map::new()))
                .as_object_mut()
                .unwrap()
                .insert(e.name.clone(), Value::Object(o));
        }
        skins.push(json!({ "name": sk.name, "attachments": atts }));
    }

    // ---- restricciones
    let (mut ik, mut tc, mut pc, mut ph) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for (order, c) in doc.constraints.iter().enumerate() {
        let bones: Vec<Value> = c.bones.iter().map(|b| json!(bone_name(*b))).collect();
        let on = if c.enabled { 1.0 } else { 0.0 };
        match &c.kind {
            ConstraintKind::Ik { target, mix, bend_positive, softness, stretch, compress, uniform } => ik.push(json!({
                "name": c.name, "order": order, "bones": bones, "target": bone_name(*target),
                "mix": r3(mix * on), "bendPositive": bend_positive, "softness": r3(*softness),
                "stretch": stretch, "compress": compress, "uniform": uniform
            })),
            ConstraintKind::Transform { target, mix_rotate, mix_translate, mix_scale, off_rot, off_x, off_y, off_sx, off_sy, mix_y, mix_scale_y, mix_shear, off_shy } => tc.push(json!({
                "name": c.name, "order": order, "bones": bones, "target": bone_name(*target),
                "rotation": r3(*off_rot), "x": r3(*off_x), "y": r3(*off_y),
                "scaleX": r3(*off_sx), "scaleY": r3(*off_sy), "shearY": r3(*off_shy),
                "mixRotate": r3(mix_rotate * on), "mixX": r3(mix_translate * on), "mixY": r3(mix_y.unwrap_or(*mix_translate) * on),
                "mixScaleX": r3(mix_scale * on), "mixScaleY": r3(mix_scale_y.unwrap_or(*mix_scale) * on), "mixShearY": r3(mix_shear * on)
            })),
            ConstraintKind::Path { slot, position, spacing, spacing_mode, rotate_mode, off_rot, mix_rotate, mix_translate } => pc.push(json!({
                "name": c.name, "order": order, "bones": bones, "target": slot_name(*slot),
                "positionMode": "percent",
                "spacingMode": match spacing_mode { SpacingMode::Length => "length", SpacingMode::Fixed => "fixed", SpacingMode::Percent => "percent" },
                "rotateMode": match rotate_mode { RotateMode::Tangent => "tangent", RotateMode::Chain => "chain" },
                "rotation": r3(*off_rot), "position": r3(*position), "spacing": r3(*spacing),
                "mixRotate": r3(mix_rotate * on), "mixX": r3(mix_translate * on), "mixY": r3(mix_translate * on)
            })),
            ConstraintKind::Physics { inertia, strength, damping, mass, gravity, wind, mix } => {
                if let Some(b) = c.bones.first() {
                    ph.push(json!({
                        "name": c.name, "order": order, "bone": bone_name(*b), "rotate": 1,
                        "inertia": r3(*inertia), "strength": r3(*strength),
                        "damping": r3((1.0 - (-damping / 10.0).exp()).clamp(0.0, 1.0)),
                        "mass": r3(*mass), "gravity": r3(gravity * 10.0), "wind": r3(wind * 10.0), "mix": r3(mix * on)
                    }))
                }
            }
        }
    }

    // ---- eventos
    let mut events = Map::new();
    for e in &doc.events {
        let mut o = Map::new();
        if e.int != 0 {
            o.insert("int".into(), json!(e.int));
        }
        if e.float != 0.0 {
            o.insert("float".into(), r3(e.float));
        }
        if !e.string.is_empty() {
            o.insert("string".into(), json!(e.string));
        }
        if !e.audio.is_empty() {
            o.insert("audio".into(), json!(e.audio));
        }
        events.insert(e.name.clone(), Value::Object(o));
    }

    // ---- animaciones
    let mut anims = Map::new();
    for a in &doc.animations {
        let fps = a.fps.max(1.0);
        let t = |f: u32| f as f32 / fps;
        let mut bones_tl: Map<String, Value> = Map::new();
        for tr in &a.tracks {
            let Some(b) = doc.bone(tr.bone) else { continue };
            let sl = Doc::setup_local(b);
            let rel = |v: [f32; 2]| -> [f32; 2] {
                match tr.channel {
                    Channel::Rotate => [v[0] - sl.rot, 0.0],
                    Channel::Translate => [v[0] - sl.x, v[1] - sl.y],
                    Channel::Scale => [
                        if sl.sx.abs() > 1e-6 { v[0] / sl.sx } else { v[0] },
                        if sl.sy.abs() > 1e-6 { v[1] / sl.sy } else { v[1] },
                    ],
                    Channel::Shear => [v[0] - sl.shx, v[1] - sl.shy],
                }
            };
            let mut keys = Vec::new();
            for (i, k) in tr.keys.iter().enumerate() {
                let v = rel(k.v);
                let mut o = Map::new();
                if k.frame != 0 {
                    o.insert("time".into(), r3(t(k.frame)));
                }
                match tr.channel {
                    Channel::Rotate => {
                        o.insert("value".into(), r3(v[0]));
                    }
                    _ => {
                        o.insert("x".into(), r3(v[0]));
                        o.insert("y".into(), r3(v[1]));
                    }
                }
                if let Some(n) = tr.keys.get(i + 1) {
                    let nv = rel(n.v);
                    let c = if tr.channel == Channel::Rotate {
                        curve_json(k.curve, t(k.frame), t(n.frame), &[(v[0], nv[0])])
                    } else {
                        match (k.curve_y, k.curve) {
                            // curvas distintas por eje (el escalonado afecta a los dos en Spine)
                            (Some(cy), cx) if cy != cx && cx != Curve::Stepped && cy != Curve::Stepped => {
                                let mut a1 = curve_json(cx, t(k.frame), t(n.frame), &[(v[0], nv[0])]).and_then(|v| v.as_array().cloned()).unwrap_or_else(|| {
                                    curve_json(Curve::Bezier(Curve::Linear.as_bezier().unwrap()), t(k.frame), t(n.frame), &[(v[0], nv[0])]).unwrap().as_array().cloned().unwrap()
                                });
                                let a2 = curve_json(cy, t(k.frame), t(n.frame), &[(v[1], nv[1])]).and_then(|v| v.as_array().cloned()).unwrap_or_else(|| {
                                    curve_json(Curve::Bezier(Curve::Linear.as_bezier().unwrap()), t(k.frame), t(n.frame), &[(v[1], nv[1])]).unwrap().as_array().cloned().unwrap()
                                });
                                a1.extend(a2);
                                Some(Value::Array(a1))
                            }
                            _ => curve_json(k.curve, t(k.frame), t(n.frame), &[(v[0], nv[0]), (v[1], nv[1])]),
                        }
                    };
                    if let Some(c) = c {
                        o.insert("curve".into(), c);
                    }
                }
                keys.push(Value::Object(o));
            }
            let ch = match tr.channel {
                Channel::Rotate => "rotate",
                Channel::Translate => "translate",
                Channel::Scale => "scale",
                Channel::Shear => "shear",
            };
            bones_tl
                .entry(b.name.clone())
                .or_insert_with(|| Value::Object(Map::new()))
                .as_object_mut()
                .unwrap()
                .insert(ch.into(), Value::Array(keys));
        }

        let mut slots_tl: Map<String, Value> = Map::new();
        for s in &doc.slots {
            let mut o = Map::new();
            let ak: Vec<&AttKey> = a.att_keys.iter().filter(|k| k.slot == s.id).collect();
            if !ak.is_empty() {
                o.insert("attachment".into(), Value::Array(ak.iter().map(|k| json!({ "time": r3(t(k.frame)), "name": k.name })).collect()));
            }
            let ck: Vec<&ColorKey> = a.color_keys.iter().filter(|k| k.slot == s.id).collect();
            if !ck.is_empty() {
                let mut keys = Vec::new();
                for (i, k) in ck.iter().enumerate() {
                    let mut ko = Map::new();
                    ko.insert("time".into(), r3(t(k.frame)));
                    ko.insert("color".into(), json!(hex(k.color)));
                    if let Some(n) = ck.get(i + 1) {
                        let vals: Vec<(f32, f32)> = (0..4).map(|c| (k.color[c] as f32 / 255.0, n.color[c] as f32 / 255.0)).collect();
                        if let Some(c) = curve_json(k.curve, t(k.frame), t(n.frame), &vals) {
                            ko.insert("curve".into(), c);
                        }
                    }
                    keys.push(Value::Object(ko));
                }
                o.insert("rgba".into(), Value::Array(keys));
            }
            if !o.is_empty() {
                slots_tl.insert(s.name.clone(), Value::Object(o));
            }
        }

        // deformaciones: attachments -> skin -> slot -> adjunto -> deform
        let mut atts_tl: Map<String, Value> = Map::new();
        let mut groups: Vec<(u32, String)> = a.deform_keys.iter().map(|k| (k.slot, k.att.clone())).collect();
        groups.sort();
        groups.dedup();
        for (sid, an) in groups {
            let Some(s) = doc.slot(sid) else { continue };
            for sk in &doc.skins {
                let Some(e) = sk.entries.iter().find(|e| e.slot == sid && e.name == an) else { continue };
                let Some(m) = &e.att.mesh else { continue };
                let sbs = setup.get(&s.bone).copied().unwrap_or(Aff::IDENTITY);
                let keys: Vec<&DeformKey> = a.deform_keys.iter().filter(|k| k.slot == sid && k.att == an).collect();
                let mut out = Vec::new();
                for (i, k) in keys.iter().enumerate() {
                    let mut vals: Vec<Value> = Vec::new();
                    if m.weighted() {
                        for (vi, ws) in m.weights.iter().enumerate() {
                            let o = k.offsets.get(vi).copied().unwrap_or([0.0, 0.0]);
                            let ow = sbs.apply_vec(o[0], o[1]);
                            for w in ws {
                                let bs = setup.get(&w.bone).copied().unwrap_or(Aff::IDENTITY);
                                let ol = bs.inv_vec(ow[0], ow[1]);
                                vals.push(r3(ol[0]));
                                vals.push(r3(ol[1]));
                            }
                        }
                    } else {
                        for vi in 0..m.verts.len() {
                            let o = k.offsets.get(vi).copied().unwrap_or([0.0, 0.0]);
                            vals.push(r3(o[0]));
                            vals.push(r3(o[1]));
                        }
                    }
                    let mut ko = Map::new();
                    ko.insert("time".into(), r3(t(k.frame)));
                    ko.insert("vertices".into(), Value::Array(vals));
                    if let Some(n) = keys.get(i + 1) {
                        if let Some(c) = curve_json(k.curve, t(k.frame), t(n.frame), &[(0.0, 1.0)]) {
                            ko.insert("curve".into(), c);
                        }
                    }
                    out.push(Value::Object(ko));
                }
                let skin_o = atts_tl.entry(sk.name.clone()).or_insert_with(|| Value::Object(Map::new()));
                let slot_o = skin_o.as_object_mut().unwrap().entry(s.name.clone()).or_insert_with(|| Value::Object(Map::new()));
                slot_o.as_object_mut().unwrap().insert(an.clone(), json!({ "deform": out }));
            }
        }

        // restricciones animadas
        let (mut ik_tl, mut tc_tl, mut pc_tl, mut ph_tl) = (Map::new(), Map::new(), Map::new(), Map::new());
        for c in &doc.constraints {
            let keys: Vec<&ConKey> = a.con_keys.iter().filter(|k| k.con == c.name).collect();
            if keys.is_empty() {
                continue;
            }
            let curve_of = |i: usize, dims: &[usize]| -> Option<Value> {
                let (k, n) = (keys[i], keys.get(i + 1)?);
                let vals: Vec<(f32, f32)> = dims.iter().map(|d| (k.v.get(*d).copied().unwrap_or(0.0), n.v.get(*d).copied().unwrap_or(0.0))).collect();
                curve_json(k.curve, t(k.frame), t(n.frame), &vals)
            };
            let mut list = Vec::new();
            match &c.kind {
                ConstraintKind::Ik { .. } => {
                    for (i, k) in keys.iter().enumerate() {
                        let mut o = Map::new();
                        o.insert("time".into(), r3(t(k.frame)));
                        o.insert("mix".into(), r3(k.v[0]));
                        o.insert("softness".into(), r3(k.v.get(1).copied().unwrap_or(0.0)));
                        o.insert("bendPositive".into(), json!(k.bend));
                        o.insert("stretch".into(), json!(k.v.get(2).copied().unwrap_or(0.0) > 0.5));
                        o.insert("compress".into(), json!(k.v.get(3).copied().unwrap_or(0.0) > 0.5));
                        if let Some(cv) = curve_of(i, &[0, 1]) {
                            o.insert("curve".into(), cv);
                        }
                        list.push(Value::Object(o));
                    }
                    ik_tl.insert(c.name.clone(), Value::Array(list));
                }
                ConstraintKind::Transform { .. } => {
                    let names = ["mixRotate", "mixX", "mixY", "mixScaleX", "mixScaleY", "mixShearY"];
                    for (i, k) in keys.iter().enumerate() {
                        let mut o = Map::new();
                        o.insert("time".into(), r3(t(k.frame)));
                        for (j, n) in names.iter().enumerate() {
                            o.insert((*n).into(), r3(k.v.get(j).copied().unwrap_or(1.0)));
                        }
                        if let Some(cv) = curve_of(i, &[0, 1, 2, 3, 4, 5]) {
                            o.insert("curve".into(), cv);
                        }
                        list.push(Value::Object(o));
                    }
                    tc_tl.insert(c.name.clone(), Value::Array(list));
                }
                ConstraintKind::Path { .. } => {
                    let mut pos = Vec::new();
                    let mut spc = Vec::new();
                    let mut mix = Vec::new();
                    for (i, k) in keys.iter().enumerate() {
                        let tm = r3(t(k.frame));
                        let mut a1 = Map::new();
                        a1.insert("time".into(), tm.clone());
                        a1.insert("value".into(), r3(k.v[0]));
                        if let Some(cv) = curve_of(i, &[0]) {
                            a1.insert("curve".into(), cv);
                        }
                        pos.push(Value::Object(a1));
                        let mut a2 = Map::new();
                        a2.insert("time".into(), tm.clone());
                        a2.insert("value".into(), r3(k.v.get(1).copied().unwrap_or(0.0)));
                        if let Some(cv) = curve_of(i, &[1]) {
                            a2.insert("curve".into(), cv);
                        }
                        spc.push(Value::Object(a2));
                        let mut a3 = Map::new();
                        a3.insert("time".into(), tm);
                        a3.insert("mixRotate".into(), r3(k.v.get(2).copied().unwrap_or(1.0)));
                        a3.insert("mixX".into(), r3(k.v.get(3).copied().unwrap_or(1.0)));
                        a3.insert("mixY".into(), r3(k.v.get(3).copied().unwrap_or(1.0)));
                        if let Some(cv) = curve_of(i, &[2, 3, 3]) {
                            a3.insert("curve".into(), cv);
                        }
                        mix.push(Value::Object(a3));
                    }
                    pc_tl.insert(c.name.clone(), json!({ "position": pos, "spacing": spc, "mix": mix }));
                }
                ConstraintKind::Physics { .. } => {
                    for (i, k) in keys.iter().enumerate() {
                        let mut o = Map::new();
                        o.insert("time".into(), r3(t(k.frame)));
                        o.insert("value".into(), r3(k.v[0]));
                        if let Some(cv) = curve_of(i, &[0]) {
                            o.insert("curve".into(), cv);
                        }
                        list.push(Value::Object(o));
                    }
                    ph_tl.insert(c.name.clone(), json!({ "mix": list }));
                }
            }
        }

        // orden de dibujo animado: desplazamientos respecto al orden base
        let mut draw_tl = Vec::new();
        for k in &a.draw_keys {
            let mut offsets = Vec::new();
            for (orig, sid) in doc.draw_order.iter().enumerate() {
                if let Some(np) = k.order.iter().position(|s| s == sid) {
                    if np != orig {
                        offsets.push(json!({ "slot": slot_name(*sid), "offset": np as i64 - orig as i64 }));
                    }
                }
            }
            let mut o = Map::new();
            o.insert("time".into(), r3(t(k.frame)));
            if !offsets.is_empty() {
                o.insert("offsets".into(), Value::Array(offsets));
            }
            draw_tl.push(Value::Object(o));
        }

        let ev: Vec<Value> = a
            .events
            .iter()
            .map(|e| {
                let mut o = Map::new();
                o.insert("time".into(), r3(t(e.frame)));
                o.insert("name".into(), json!(e.name));
                if e.int != 0 {
                    o.insert("int".into(), json!(e.int));
                }
                if e.float != 0.0 {
                    o.insert("float".into(), r3(e.float));
                }
                if !e.string.is_empty() {
                    o.insert("string".into(), json!(e.string));
                }
                Value::Object(o)
            })
            .collect();

        let mut ao = Map::new();
        if !slots_tl.is_empty() {
            ao.insert("slots".into(), Value::Object(slots_tl));
        }
        if !bones_tl.is_empty() {
            ao.insert("bones".into(), Value::Object(bones_tl));
        }
        if !atts_tl.is_empty() {
            ao.insert("attachments".into(), Value::Object(atts_tl));
        }
        if !ev.is_empty() {
            ao.insert("events".into(), Value::Array(ev));
        }
        for (k, m) in [("ik", ik_tl), ("transform", tc_tl), ("path", pc_tl), ("physics", ph_tl)] {
            if !m.is_empty() {
                ao.insert(k.into(), Value::Object(m));
            }
        }
        if !draw_tl.is_empty() {
            ao.insert("drawOrder".into(), Value::Array(draw_tl));
        }
        anims.insert(a.name.clone(), Value::Object(ao));
    }

    // ---- límites aproximados
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for w in setup.values() {
        x0 = x0.min(w.tx);
        y0 = y0.min(w.ty);
        x1 = x1.max(w.tx);
        y1 = y1.max(w.ty);
    }
    if x0 > x1 {
        (x0, y0, x1, y1) = (0.0, 0.0, 0.0, 0.0);
    }

    let mut root = Map::new();
    root.insert(
        "skeleton".into(),
        json!({ "hash": "", "spine": "4.2.00", "x": r3(x0), "y": r3(y0), "width": r3(x1 - x0), "height": r3(y1 - y0), "images": "./images/", "audio": "./audio/" }),
    );
    root.insert("bones".into(), Value::Array(bones));
    if !slots.is_empty() {
        root.insert("slots".into(), Value::Array(slots));
    }
    if !ik.is_empty() {
        root.insert("ik".into(), Value::Array(ik));
    }
    if !tc.is_empty() {
        root.insert("transform".into(), Value::Array(tc));
    }
    if !pc.is_empty() {
        root.insert("path".into(), Value::Array(pc));
    }
    if !ph.is_empty() {
        root.insert("physics".into(), Value::Array(ph));
    }
    root.insert("skins".into(), Value::Array(skins));
    if !events.is_empty() {
        root.insert("events".into(), Value::Object(events));
    }
    root.insert("animations".into(), Value::Object(anims));

    let text = serde_json::to_string_pretty(&Value::Object(root)).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| e.to_string())?;

    // ---- atlas
    let n = regions.len();
    if n > 0 {
        let (packed, w, h) = pack(regions);
        let mut page = RgbaImage::new(w, h);
        let mut atlas = format!("{stem}.png\nsize:{w},{h}\nfilter:Linear,Linear\n");
        for p in &packed {
            image::imageops::overlay(&mut page, &p.img, p.x as i64, p.y as i64);
            atlas.push_str(&format!("{}\nbounds:{},{},{},{}\n", p.name, p.x, p.y, p.img.width(), p.img.height()));
        }
        page.save(dir.join(format!("{stem}.png"))).map_err(|e| e.to_string())?;
        std::fs::write(dir.join(format!("{stem}.atlas")), atlas).map_err(|e| e.to_string())?;
    }
    Ok(n)
}

// ---------------------------------------------------------------------------
// Importar
// ---------------------------------------------------------------------------

struct AtlasRegion {
    page: PathBuf,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    rotate: bool,
    /// Recorte de espacios vacíos: desplazamiento y tamaño original.
    offsets: Option<[u32; 4]>,
}

/// Lee un .atlas (formato antiguo y nuevo de libGDX/Spine).
fn read_atlas(path: &Path) -> HashMap<String, AtlasRegion> {
    let mut out = HashMap::new();
    let Ok(text) = std::fs::read_to_string(path) else { return out };
    let dir = path.parent().unwrap_or(Path::new("."));
    let mut page: Option<PathBuf> = None;
    let mut cur: Option<(String, AtlasRegion)> = None;
    let mut expect_page = true;
    for raw in text.lines() {
        let line = raw.trim_end();
        if line.trim().is_empty() {
            if let Some((n, r)) = cur.take() {
                out.insert(n, r);
            }
            expect_page = true;
            continue;
        }
        if let Some((k, v)) = line.trim().split_once(':') {
            let nums: Vec<u32> = v.split(',').filter_map(|s| s.trim().parse().ok()).collect();
            if let Some((_, r)) = cur.as_mut() {
                match k.trim() {
                    "bounds" if nums.len() == 4 => {
                        r.x = nums[0];
                        r.y = nums[1];
                        r.w = nums[2];
                        r.h = nums[3];
                    }
                    "xy" if nums.len() == 2 => {
                        r.x = nums[0];
                        r.y = nums[1];
                    }
                    "size" if nums.len() == 2 => {
                        r.w = nums[0];
                        r.h = nums[1];
                    }
                    "rotate" => r.rotate = v.trim() == "true" || v.trim() == "90",
                    "offsets" if nums.len() == 4 => r.offsets = Some([nums[0], nums[1], nums[2], nums[3]]),
                    "offset" if nums.len() == 2 => {
                        let o = r.offsets.get_or_insert([0, 0, 0, 0]);
                        o[0] = nums[0];
                        o[1] = nums[1];
                    }
                    "orig" if nums.len() == 2 => {
                        let o = r.offsets.get_or_insert([0, 0, 0, 0]);
                        o[2] = nums[0];
                        o[3] = nums[1];
                    }
                    _ => {}
                }
            }
            continue;
        }
        // línea sin ':' -> página o región
        let name = line.trim().to_string();
        let is_image = [".png", ".jpg", ".jpeg"].iter().any(|e| name.to_lowercase().ends_with(e));
        if expect_page && is_image {
            if let Some((n, r)) = cur.take() {
                out.insert(n, r);
            }
            page = Some(dir.join(&name));
            expect_page = false;
            continue;
        }
        expect_page = false;
        if let Some((n, r)) = cur.take() {
            out.insert(n, r);
        }
        if let Some(p) = &page {
            cur = Some((name, AtlasRegion { page: p.clone(), x: 0, y: 0, w: 0, h: 0, rotate: false, offsets: None }));
        }
    }
    if let Some((n, r)) = cur.take() {
        out.insert(n, r);
    }
    out
}

struct Images {
    dirs: Vec<PathBuf>,
    atlas: HashMap<String, AtlasRegion>,
    out_dir: PathBuf,
    pages: HashMap<PathBuf, Option<RgbaImage>>,
    cache: HashMap<String, Option<String>>,
}

impl Images {
    fn find(&mut self, region: &str) -> Option<String> {
        if let Some(c) = self.cache.get(region) {
            return c.clone();
        }
        let mut found = None;
        'dirs: for d in &self.dirs {
            for ext in ["png", "jpg", "jpeg"] {
                let p = d.join(format!("{region}.{ext}"));
                if p.is_file() {
                    found = Some(p.to_string_lossy().to_string());
                    break 'dirs;
                }
            }
        }
        if found.is_none() {
            if let Some(r) = self.atlas.get(region) {
                let page = self.pages.entry(r.page.clone()).or_insert_with(|| image::open(&r.page).ok().map(|i| i.to_rgba8()));
                if let Some(pg) = page {
                    let (w, h) = if r.rotate { (r.h, r.w) } else { (r.w, r.h) };
                    if w > 0 && h > 0 && r.x + w <= pg.width() && r.y + h <= pg.height() {
                        let mut sub = image::imageops::crop_imm(pg, r.x, r.y, w, h).to_image();
                        if r.rotate {
                            sub = image::imageops::rotate270(&sub);
                        }
                        if let Some([ox, oy, ow, oh]) = r.offsets.filter(|o| o[2] > 0 && o[3] > 0 && (o[2] != r.w || o[3] != r.h)) {
                            // devuelve el espacio vacío recortado al empaquetar (oy se mide desde abajo)
                            let mut full = RgbaImage::new(ow, oh);
                            let top = oh as i64 - oy as i64 - sub.height() as i64;
                            image::imageops::overlay(&mut full, &sub, ox as i64, top);
                            sub = full;
                        }
                        let _ = std::fs::create_dir_all(&self.out_dir);
                        let p = self.out_dir.join(format!("{}.png", safe_name(region)));
                        if sub.save(&p).is_ok() {
                            found = Some(p.to_string_lossy().to_string());
                        }
                    }
                }
            }
        }
        self.cache.insert(region.to_string(), found.clone());
        found
    }
}

fn f(v: &Value, k: &str, d: f32) -> f32 {
    v.get(k).and_then(|x| x.as_f64()).map(|x| x as f32).unwrap_or(d)
}
fn s<'a>(v: &'a Value, k: &str) -> Option<&'a str> {
    v.get(k).and_then(|x| x.as_str())
}
fn arr<'a>(v: &'a Value, k: &str) -> &'a [Value] {
    v.get(k).and_then(|x| x.as_array()).map(|a| a.as_slice()).unwrap_or(&[])
}
fn nums(v: &Value, k: &str) -> Vec<f32> {
    arr(v, k).iter().filter_map(|x| x.as_f64()).map(|x| x as f32).collect()
}

/// Convierte la curva de una clave de Spine a nuestra curva normalizada.
fn read_curve(k: &Value, ta: f32, tb: f32, va: f32, vb: f32) -> Curve {
    read_curve_dim(k, ta, tb, va, vb, 0)
}

/// Igual que `read_curve`, usando la curva del componente `dim` (0 = x, 1 = y...).
fn read_curve_dim(k: &Value, ta: f32, tb: f32, va: f32, vb: f32, dim: usize) -> Curve {
    match k.get("curve") {
        None => Curve::Linear,
        Some(Value::String(s)) if s == "stepped" => Curve::Stepped,
        Some(Value::Number(n)) => {
            // formato 3.8: curve = c1, y c2, c3, c4 aparte, normalizados
            let c1 = n.as_f64().unwrap_or(0.0) as f32;
            Curve::Bezier([c1, f(k, "c2", 0.0), f(k, "c3", 1.0), f(k, "c4", 1.0)])
        }
        Some(Value::Array(a)) => {
            let all: Vec<f32> = a.iter().filter_map(|x| x.as_f64()).map(|x| x as f32).collect();
            let c: Vec<f32> = all.get(dim * 4..dim * 4 + 4).map(|c| c.to_vec()).unwrap_or_else(|| all.clone());
            if c.len() < 4 {
                return Curve::Linear;
            }
            let dt = (tb - ta).abs().max(1e-6);
            let dv = vb - va;
            // sin recortar: Spine admite asas fuera del intervalo
            let nx = |x: f32| (x - ta) / dt;
            let x1 = nx(c[0]);
            let x2 = nx(c[2]);
            let flat = dv.abs() < 1e-4 * (1.0 + va.abs());
            if flat {
                if (c[1] - va).abs() < 1e-4 && (c[3] - va).abs() < 1e-4 {
                    Curve::Linear
                } else {
                    Curve::BezierAbs([x1, c[1] - va, x2, c[3] - va])
                }
            } else {
                Curve::Bezier([x1, (c[1] - va) / dv, x2, (c[3] - va) / dv])
            }
        }
        _ => Curve::Linear,
    }
}

/// Lee vértices de Spine (con o sin pesos). Devuelve posiciones en el espacio del hueso del slot y pesos.
fn read_verts(verts: &[f32], n: usize, sbone: u32, bone_ids: &[u32], setup: &HashMap<u32, Aff>) -> (Vec<[f32; 2]>, Vec<Vec<Weight>>) {
    if verts.len() == n * 2 {
        return (verts.chunks(2).map(|c| [c[0], c[1]]).collect(), Vec::new());
    }
    let sbs = setup.get(&sbone).copied().unwrap_or(Aff::IDENTITY);
    let mut pos = Vec::new();
    let mut weights = Vec::new();
    let mut i = 0;
    while i < verts.len() && weights.len() < n {
        let count = verts[i] as usize;
        i += 1;
        let mut ws = Vec::new();
        let mut wp = [0.0f32, 0.0];
        for _ in 0..count {
            if i + 4 > verts.len() {
                break;
            }
            let bid = bone_ids.get(verts[i] as usize).copied().unwrap_or(0);
            let (x, y, w) = (verts[i + 1], verts[i + 2], verts[i + 3]);
            i += 4;
            let q = setup.get(&bid).copied().unwrap_or(Aff::IDENTITY).apply(x, y);
            wp[0] += q[0] * w;
            wp[1] += q[1] * w;
            ws.push(Weight { bone: bid, x, y, w });
        }
        pos.push(sbs.inv_point(wp[0], wp[1]));
        weights.push(ws);
    }
    (pos, weights)
}

pub fn import(path: &Path) -> Result<Doc, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let root: Value = serde_json::from_str(&text).map_err(|e| format!("JSON no válido: {e}"))?;
    if root.get("bones").is_none() {
        return Err("no parece un esqueleto de Spine (falta «bones»)".into());
    }
    let dir = path.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("."));
    let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let fps = 30.0f32;

    let mut doc = Doc::empty();
    doc.bones.clear();
    doc.animations.clear();
    doc.skins.clear();
    doc.next_id = 0;

    // ---- huesos
    let mut bone_ids: Vec<u32> = Vec::new();
    let mut by_name: HashMap<String, u32> = HashMap::new();
    for b in arr(&root, "bones") {
        let name = s(b, "name").unwrap_or("hueso").to_string();
        let parent = s(b, "parent").and_then(|p| by_name.get(p).copied());
        let id = doc.add_bone(parent, name.clone(), f(b, "x", 0.0), f(b, "y", 0.0), f(b, "rotation", 0.0), f(b, "length", 0.0));
        let bb = doc.bone_mut(id).unwrap();
        bb.sx = f(b, "scaleX", 1.0);
        bb.sy = f(b, "scaleY", 1.0);
        bb.shx = f(b, "shearX", 0.0);
        bb.shy = f(b, "shearY", 0.0);
        bb.inherit = Inherit::from_spine(s(b, "inherit").or_else(|| s(b, "transform")).unwrap_or("normal"));
        bb.color = s(b, "color").map(parse_hex).unwrap_or([155, 155, 155, 255]);
        bone_ids.push(id);
        by_name.insert(name, id);
    }
    let setup = doc.world_fk(None, 0.0);

    // ---- slots
    let mut slot_by_name: HashMap<String, u32> = HashMap::new();
    for sl in arr(&root, "slots") {
        let name = s(sl, "name").unwrap_or("slot").to_string();
        let bone = s(sl, "bone").and_then(|b| by_name.get(b).copied()).unwrap_or(0);
        let id = doc.add_slot(bone, name.clone());
        let so = doc.slot_mut(id).unwrap();
        so.color = s(sl, "color").map(parse_hex).unwrap_or([255; 4]);
        so.attachment = s(sl, "attachment").map(|x| x.to_string());
        slot_by_name.insert(name, id);
    }

    // ---- imágenes
    let images_dir = root.get("skeleton").and_then(|sk| s(sk, "images")).map(|p| dir.join(p));
    let mut dirs = Vec::new();
    if let Some(d) = images_dir {
        dirs.push(d);
    }
    dirs.push(dir.join("images"));
    dirs.push(dir.clone());
    let atlas_path = dir.join(format!("{stem}.atlas"));
    let atlas = if atlas_path.is_file() {
        read_atlas(&atlas_path)
    } else {
        std::fs::read_dir(&dir)
            .ok()
            .and_then(|rd| rd.flatten().map(|e| e.path()).find(|p| p.extension().map(|e| e == "atlas").unwrap_or(false)))
            .map(|p| read_atlas(&p))
            .unwrap_or_default()
    };
    let mut imgs = Images { dirs, atlas, out_dir: dir.join(format!("{stem}_regiones")), pages: HashMap::new(), cache: HashMap::new() };

    // ---- skins (4.x: lista; 3.x: objeto)
    let mut skin_list: Vec<(String, Value)> = match root.get("skins") {
        Some(Value::Array(a)) => a
            .iter()
            .map(|sk| (s(sk, "name").unwrap_or("default").to_string(), sk.get("attachments").cloned().unwrap_or(Value::Null)))
            .collect(),
        Some(Value::Object(o)) => o.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        _ => Vec::new(),
    };
    if let Some(i) = skin_list.iter().position(|(n, _)| n == "default") {
        let d = skin_list.remove(i);
        skin_list.insert(0, d);
    } else {
        skin_list.insert(0, ("default".into(), Value::Null));
    }
    let mut linked: Vec<(usize, u32, String, String, String, String)> = Vec::new();
    for (si, (sname, atts)) in skin_list.iter().enumerate() {
        doc.skins.push(Skin { name: sname.clone(), entries: Vec::new() });
        let Some(slots) = atts.as_object() else { continue };
        for (slot_n, list) in slots {
            let Some(&sid) = slot_by_name.get(slot_n) else { continue };
            let sbone = doc.slot(sid).map(|s| s.bone).unwrap_or(0);
            let Some(list) = list.as_object() else { continue };
            for (att_n, a) in list {
                let kind = s(a, "type").unwrap_or("region");
                let region = s(a, "path").or_else(|| s(a, "name")).unwrap_or(att_n).to_string();
                let tint = s(a, "color").map(parse_hex).unwrap_or([255; 4]);
                let mut att = Attachment::rect(f(a, "width", 32.0), f(a, "height", 32.0), f(a, "x", 0.0), f(a, "y", 0.0), tint);
                att.rot = f(a, "rotation", 0.0);
                match kind {
                    "region" => {
                        if let Some(p) = imgs.find(&region) {
                            att.kind = AttachKind::Image;
                            att.image = p;
                        }
                    }
                    "mesh" => {
                        let uvs = nums(a, "uvs");
                        let verts = nums(a, "vertices");
                        let tris: Vec<u32> = arr(a, "triangles").iter().filter_map(|x| x.as_u64()).map(|x| x as u32).collect();
                        let nv = uvs.len() / 2;
                        let mut m = MeshData {
                            verts: Vec::new(),
                            uvs: uvs.chunks(2).filter(|c| c.len() == 2).map(|c| [c[0], c[1]]).collect(),
                            tris: tris.chunks(3).filter(|c| c.len() == 3).map(|c| [c[0], c[1], c[2]]).collect(),
                            hull: a.get("hull").and_then(|h| h.as_u64()).unwrap_or(nv as u64) as usize,
                            weights: Vec::new(),
                        };
                        let (pv, pw) = read_verts(&verts, nv, sbone, &bone_ids, &setup);
                        m.verts = pv;
                        m.weights = pw;
                        att.kind = AttachKind::Mesh;
                        if let Some(p) = imgs.find(&region) {
                            att.image = p;
                        }
                        att.mesh = Some(m);
                    }
                    "linkedmesh" => {
                        let parent = s(a, "parent").unwrap_or("").to_string();
                        let pskin = s(a, "skin").unwrap_or("default").to_string();
                        let img = imgs.find(&region).unwrap_or_default();
                        linked.push((si, sid, att_n.clone(), parent, pskin, img));
                        continue;
                    }
                    "path" => {
                        let v = nums(a, "vertices");
                        let n = f(a, "vertexCount", (v.len() / 2) as f32) as usize;
                        if v.len() != n * 2 {
                            continue; // trayectorias con pesos no se importan
                        }
                        let pts: Vec<PathPt> = v
                            .chunks(6)
                            .filter(|c| c.len() == 6)
                            .map(|c| PathPt { p: [c[2], c[3]], hin: [c[0] - c[2], c[1] - c[3]], hout: [c[4] - c[2], c[5] - c[3]] })
                            .collect();
                        att = Attachment::path_line(10.0);
                        att.path = Some(PathData { pts, closed: a.get("closed").and_then(|x| x.as_bool()).unwrap_or(false) });
                        att.tint = tint;
                    }
                    "boundingbox" | "clipping" => {
                        let v = nums(a, "vertices");
                        let n = f(a, "vertexCount", (v.len() / 2) as f32) as usize;
                        let (pv, pw) = read_verts(&v, n, sbone, &bone_ids, &setup);
                        let k = if kind == "clipping" { AttachKind::Clipping } else { AttachKind::BoundingBox };
                        let mut pa = Attachment::polygon(k, pv);
                        if let Some(m) = pa.mesh.as_mut() {
                            m.weights = pw;
                        }
                        if let Some(c) = s(a, "color") {
                            pa.tint = parse_hex(c);
                        }
                        pa.end_slot = s(a, "end").and_then(|e| slot_by_name.get(e).copied());
                        att = pa;
                    }
                    "point" => {
                        att = Attachment::point(f(a, "x", 0.0), f(a, "y", 0.0), f(a, "rotation", 0.0));
                        if let Some(c) = s(a, "color") {
                            att.tint = parse_hex(c);
                        }
                    }
                    _ => continue,
                }
                doc.skins[si].entries.push(SkinEntry { slot: sid, name: att_n.clone(), att });
            }
        }
    }
    // mallas enlazadas: copian la malla de su padre
    for (si, sid, name, parent, pskin, img) in linked {
        let psi = doc.skins.iter().position(|s| s.name == pskin).unwrap_or(0);
        let pm = doc.skins.get(psi).and_then(|s| s.entries.iter().find(|e| e.slot == sid && e.name == parent)).map(|e| e.att.clone());
        if let Some(mut a) = pm {
            if !img.is_empty() {
                a.image = img;
            }
            doc.skins[si].entries.push(SkinEntry { slot: sid, name, att: a });
        }
    }

    // ---- restricciones
    let mut cons: Vec<(i64, Constraint)> = Vec::new();
    let order_of = |c: &Value| c.get("order").and_then(|o| o.as_i64()).unwrap_or(0);
    let bones_of = |v: &Value| -> Vec<u32> { arr(v, "bones").iter().filter_map(|b| b.as_str()).filter_map(|b| by_name.get(b).copied()).collect() };
    let mixf = |c: &Value, new: &str, old: &str| c.get(new).or_else(|| c.get(old)).and_then(|x| x.as_f64()).unwrap_or(1.0) as f32;
    for c in arr(&root, "ik") {
        let Some(target) = s(c, "target").and_then(|t| by_name.get(t).copied()) else { continue };
        cons.push((
            order_of(c),
            Constraint {
                name: s(c, "name").unwrap_or("ik").into(),
                enabled: true,
                bones: bones_of(c),
                kind: ConstraintKind::Ik {
                    target,
                    mix: f(c, "mix", 1.0),
                    bend_positive: c.get("bendPositive").and_then(|b| b.as_bool()).unwrap_or(true),
                    softness: f(c, "softness", 0.0),
                    stretch: c.get("stretch").and_then(|b| b.as_bool()).unwrap_or(false),
                    compress: c.get("compress").and_then(|b| b.as_bool()).unwrap_or(false),
                    uniform: c.get("uniform").and_then(|b| b.as_bool()).unwrap_or(false),
                },
            },
        ));
    }
    for c in arr(&root, "transform") {
        let Some(target) = s(c, "target").and_then(|t| by_name.get(t).copied()) else { continue };
        cons.push((
            order_of(c),
            Constraint {
                name: s(c, "name").unwrap_or("transformación").into(),
                enabled: true,
                bones: bones_of(c),
                kind: ConstraintKind::Transform {
                    target,
                    mix_rotate: mixf(c, "mixRotate", "rotateMix"),
                    mix_translate: mixf(c, "mixX", "translateMix"),
                    mix_scale: mixf(c, "mixScaleX", "scaleMix"),
                    off_rot: f(c, "rotation", 0.0),
                    off_x: f(c, "x", 0.0),
                    off_y: f(c, "y", 0.0),
                    off_sx: f(c, "scaleX", 0.0),
                    off_sy: f(c, "scaleY", 0.0),
                    mix_y: c.get("mixY").and_then(|x| x.as_f64()).map(|x| x as f32),
                    mix_scale_y: c.get("mixScaleY").and_then(|x| x.as_f64()).map(|x| x as f32),
                    mix_shear: c.get("mixShearY").or_else(|| c.get("shearMix")).and_then(|x| x.as_f64()).unwrap_or(1.0) as f32,
                    off_shy: f(c, "shearY", 0.0),
                },
            },
        ));
    }
    for c in arr(&root, "path") {
        let Some(slot) = s(c, "target").and_then(|t| slot_by_name.get(t).copied()) else { continue };
        let spacing_mode = match s(c, "spacingMode").unwrap_or("length") {
            "fixed" => SpacingMode::Fixed,
            "percent" => SpacingMode::Percent,
            _ => SpacingMode::Length,
        };
        let mut position = f(c, "position", 0.0);
        if s(c, "positionMode") == Some("fixed") {
            // posición en píxeles: se convierte en proporción de la longitud de la trayectoria
            let len = doc.skins.iter().flat_map(|s| s.entries.iter()).find(|e| e.slot == slot && e.att.path.is_some()).and_then(|e| e.att.path.as_ref()).map(|p| path_lengths(p).last().copied().unwrap_or(1.0)).unwrap_or(1.0);
            position /= len.max(1.0);
        }
        cons.push((
            order_of(c),
            Constraint {
                name: s(c, "name").unwrap_or("trayectoria").into(),
                enabled: true,
                bones: bones_of(c),
                kind: ConstraintKind::Path {
                    slot,
                    position,
                    spacing: f(c, "spacing", 0.0),
                    spacing_mode,
                    rotate_mode: if s(c, "rotateMode") == Some("chain") { RotateMode::Chain } else { RotateMode::Tangent },
                    off_rot: f(c, "rotation", 0.0),
                    mix_rotate: mixf(c, "mixRotate", "rotateMix"),
                    mix_translate: mixf(c, "mixX", "translateMix"),
                },
            },
        ));
    }
    for c in arr(&root, "physics") {
        let Some(b) = s(c, "bone").and_then(|t| by_name.get(t).copied()) else { continue };
        let damp = f(c, "damping", 1.0).clamp(0.0, 0.999);
        cons.push((
            order_of(c),
            Constraint {
                name: s(c, "name").unwrap_or("física").into(),
                enabled: true,
                bones: vec![b],
                kind: ConstraintKind::Physics {
                    inertia: f(c, "inertia", 0.5),
                    strength: f(c, "strength", 100.0),
                    damping: -10.0 * (1.0 - damp).ln(),
                    mass: f(c, "mass", 1.0),
                    gravity: f(c, "gravity", 0.0) / 10.0,
                    wind: f(c, "wind", 0.0) / 10.0,
                    mix: f(c, "mix", 1.0),
                },
            },
        ));
    }
    cons.sort_by_key(|c| c.0);
    doc.constraints = cons.into_iter().map(|c| c.1).filter(|c| !c.bones.is_empty()).collect();

    // ---- eventos
    if let Some(ev) = root.get("events").and_then(|e| e.as_object()) {
        for (n, e) in ev {
            doc.events.push(EventDef {
                name: n.clone(),
                int: e.get("int").and_then(|x| x.as_i64()).unwrap_or(0) as i32,
                float: f(e, "float", 0.0),
                string: s(e, "string").unwrap_or("").into(),
                audio: s(e, "audio").unwrap_or("").into(),
            });
        }
    }

    // ---- animaciones
    let frame_of = |t: f32| (t * fps).round().max(0.0) as u32;
    if let Some(anims) = root.get("animations").and_then(|a| a.as_object()) {
        for (aname, av) in anims {
            let mut a = Animation::new(aname);
            a.fps = fps;
            a.duration = 1;
            if let Some(bones) = av.get("bones").and_then(|b| b.as_object()) {
                for (bn, tls) in bones {
                    let Some(&bid) = by_name.get(bn) else { continue };
                    let sl = Doc::setup_local(doc.bone(bid).unwrap());
                    for (tname, ch) in [("rotate", Channel::Rotate), ("translate", Channel::Translate), ("scale", Channel::Scale), ("shear", Channel::Shear)] {
                        let keys = arr(tls, tname);
                        if keys.is_empty() {
                            continue;
                        }
                        let raw: Vec<(f32, [f32; 2])> = keys
                            .iter()
                            .map(|k| {
                                let t = f(k, "time", 0.0);
                                let v = match ch {
                                    Channel::Rotate => [k.get("value").or_else(|| k.get("angle")).and_then(|x| x.as_f64()).unwrap_or(0.0) as f32, 0.0],
                                    Channel::Translate => [f(k, "x", 0.0), f(k, "y", 0.0)],
                                    Channel::Scale => [f(k, "x", 1.0), f(k, "y", 1.0)],
                                    Channel::Shear => [f(k, "x", 0.0), f(k, "y", 0.0)],
                                };
                                (t, v)
                            })
                            .collect();
                        let mut track = Track { bone: bid, channel: ch, keys: Vec::new() };
                        for (i, k) in keys.iter().enumerate() {
                            let (t, v) = raw[i];
                            let (tb, vb) = raw.get(i + 1).copied().unwrap_or((t, v));
                            let curve = read_curve_dim(k, t, tb, v[0], vb[0], 0);
                            let curve_y = if ch != Channel::Rotate && matches!(k.get("curve"), Some(Value::Array(c)) if c.len() >= 8) {
                                Some(read_curve_dim(k, t, tb, v[1], vb[1], 1))
                            } else {
                                None
                            };
                            let abs = match ch {
                                Channel::Rotate => [v[0] + sl.rot, 0.0],
                                Channel::Translate => [v[0] + sl.x, v[1] + sl.y],
                                Channel::Scale => [v[0] * sl.sx, v[1] * sl.sy],
                                Channel::Shear => [v[0] + sl.shx, v[1] + sl.shy],
                            };
                            let fr = frame_of(t);
                            if track.keys.iter().any(|k| k.frame == fr) {
                                continue;
                            }
                            track.keys.push(Key { frame: fr, v: abs, curve, curve_y });
                            a.grow(fr);
                        }
                        a.tracks.push(track);
                    }
                }
            }
            if let Some(slots) = av.get("slots").and_then(|b| b.as_object()) {
                for (sn, tls) in slots {
                    let Some(&sid) = slot_by_name.get(sn) else { continue };
                    for k in arr(tls, "attachment") {
                        let fr = frame_of(f(k, "time", 0.0));
                        a.set_att_key(sid, fr, s(k, "name").map(|x| x.to_string()));
                    }
                    let ck = if tls.get("rgba").is_some() { arr(tls, "rgba") } else { arr(tls, "color") };
                    let times: Vec<f32> = ck.iter().map(|k| f(k, "time", 0.0)).collect();
                    for (i, k) in ck.iter().enumerate() {
                        let fr = frame_of(times[i]);
                        let c = s(k, "color").map(parse_hex).unwrap_or([255; 4]);
                        let tb = times.get(i + 1).copied().unwrap_or(times[i]);
                        let nc = ck.get(i + 1).and_then(|n| s(n, "color")).map(parse_hex).unwrap_or(c);
                        a.set_color_key(sid, fr, c);
                        // una sola curva por clave: la del canal que más cambia
                        let ch = (0..4).max_by_key(|n| (nc[*n] as i32 - c[*n] as i32).abs()).unwrap_or(0);
                        let curve = read_curve_dim(k, times[i], tb, c[ch] as f32 / 255.0, nc[ch] as f32 / 255.0, ch);
                        if let Some(kk) = a.color_keys.iter_mut().find(|x| x.slot == sid && x.frame == fr) {
                            kk.curve = curve;
                        }
                    }
                }
            }
            // deformaciones (4.x en "attachments", 3.x en "deform")
            let mut deform_src: Vec<(String, String, String, Vec<Value>)> = Vec::new();
            if let Some(o) = av.get("attachments").and_then(|x| x.as_object()) {
                for (sk, slots) in o {
                    for (sn, atts) in slots.as_object().into_iter().flatten() {
                        for (an, tl) in atts.as_object().into_iter().flatten() {
                            deform_src.push((sk.clone(), sn.clone(), an.clone(), arr(tl, "deform").to_vec()));
                        }
                    }
                }
            }
            if let Some(o) = av.get("deform").and_then(|x| x.as_object()) {
                for (sk, slots) in o {
                    for (sn, atts) in slots.as_object().into_iter().flatten() {
                        for (an, tl) in atts.as_object().into_iter().flatten() {
                            deform_src.push((sk.clone(), sn.clone(), an.clone(), tl.as_array().cloned().unwrap_or_default()));
                        }
                    }
                }
            }
            for (sk, sn, an, keys) in deform_src {
                let Some(&sid) = slot_by_name.get(&sn) else { continue };
                let si = doc.skins.iter().position(|s| s.name == sk).unwrap_or(0);
                let Some(mesh) = doc.skins[si].entries.iter().find(|e| e.slot == sid && e.name == an).and_then(|e| e.att.mesh.clone()) else { continue };
                let sbone = doc.slot(sid).map(|s| s.bone).unwrap_or(0);
                let sbs = setup.get(&sbone).copied().unwrap_or(Aff::IDENTITY);
                let times: Vec<f32> = keys.iter().map(|k| f(k, "time", 0.0)).collect();
                for (i, k) in keys.iter().enumerate() {
                    let off = k.get("offset").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
                    let vals = nums(k, "vertices");
                    let ncomp = if mesh.weighted() { mesh.weights.iter().map(|w| w.len() * 2).sum() } else { mesh.verts.len() * 2 };
                    let mut flat = vec![0.0f32; ncomp];
                    for (j, v) in vals.iter().enumerate() {
                        if off + j < flat.len() {
                            flat[off + j] = *v;
                        }
                    }
                    let offsets: Vec<[f32; 2]> = if mesh.weighted() {
                        let mut out = Vec::new();
                        let mut c = 0;
                        for ws in &mesh.weights {
                            let mut acc = [0.0f32, 0.0];
                            for w in ws {
                                let bs = setup.get(&w.bone).copied().unwrap_or(Aff::IDENTITY);
                                let ow = bs.apply_vec(flat[c], flat[c + 1]);
                                acc[0] += ow[0] * w.w;
                                acc[1] += ow[1] * w.w;
                                c += 2;
                            }
                            out.push(sbs.inv_vec(acc[0], acc[1]));
                        }
                        out
                    } else {
                        flat.chunks(2).map(|c| [c[0], c[1]]).collect()
                    };
                    let fr = frame_of(times[i]);
                    let tb = times.get(i + 1).copied().unwrap_or(times[i]);
                    a.set_deform_key(sid, &an, fr, offsets);
                    let curve = read_curve(k, times[i], tb, 0.0, 1.0);
                    if let Some(kk) = a.deform_keys.iter_mut().find(|x| x.slot == sid && x.att == an && x.frame == fr) {
                        kk.curve = curve;
                    }
                }
            }
            // restricciones animadas
            let con_keys = |a: &mut Animation, name: &str, keys: Vec<(f32, Vec<f32>, bool, Curve)>| {
                for (t, v, bend, curve) in keys {
                    let fr = frame_of(t);
                    a.set_con_key(name, fr, v, bend);
                    if let Some(k) = a.con_keys.iter_mut().find(|k| k.con == name && k.frame == fr) {
                        k.curve = curve;
                    }
                }
            };
            for (name, tl) in av.get("ik").and_then(|x| x.as_object()).into_iter().flatten() {
                let keys = tl.as_array().cloned().unwrap_or_default();
                let flag = |k: &Value, n: &str| if k.get(n).and_then(|b| b.as_bool()).unwrap_or(false) { 1.0 } else { 0.0 };
                let vals: Vec<(f32, f32, bool, f32)> = keys
                    .iter()
                    .map(|k| (f(k, "time", 0.0), f(k, "mix", 1.0), k.get("bendPositive").and_then(|b| b.as_bool()).unwrap_or(true), f(k, "softness", 0.0)))
                    .collect();
                let out = keys
                    .iter()
                    .enumerate()
                    .map(|(i, k)| {
                        let (t, m, b, sft) = vals[i];
                        let (tb, mb, _, _) = vals.get(i + 1).copied().unwrap_or((t, m, b, sft));
                        (t, vec![m, sft, flag(k, "stretch"), flag(k, "compress")], b, read_curve(k, t, tb, m, mb))
                    })
                    .collect();
                con_keys(&mut a, name, out);
            }
            for (name, tl) in av.get("transform").and_then(|x| x.as_object()).into_iter().flatten() {
                let keys = tl.as_array().cloned().unwrap_or_default();
                let vals: Vec<(f32, Vec<f32>)> = keys
                    .iter()
                    .map(|k| {
                        let mr = f(k, "mixRotate", f(k, "rotateMix", 1.0));
                        let mx = f(k, "mixX", f(k, "translateMix", 1.0));
                        let my = f(k, "mixY", mx);
                        let msx = f(k, "mixScaleX", f(k, "scaleMix", 1.0));
                        let msy = f(k, "mixScaleY", msx);
                        let msh = f(k, "mixShearY", f(k, "shearMix", 1.0));
                        (f(k, "time", 0.0), vec![mr, mx, my, msx, msy, msh])
                    })
                    .collect();
                let out = keys
                    .iter()
                    .enumerate()
                    .map(|(i, k)| {
                        let (t, v) = vals[i].clone();
                        let (tb, vb) = vals.get(i + 1).cloned().unwrap_or((t, v.clone()));
                        let dim = (0..6).max_by(|x, y| (vb[*x] - v[*x]).abs().partial_cmp(&(vb[*y] - v[*y]).abs()).unwrap()).unwrap_or(0);
                        let c = read_curve_dim(k, t, tb, v[dim], vb[dim], dim);
                        (t, v, true, c)
                    })
                    .collect();
                con_keys(&mut a, name, out);
            }
            for (name, tl) in av.get("path").and_then(|x| x.as_object()).into_iter().flatten() {
                let Some(base) = doc.constraints.iter().find(|c| c.name == *name).map(|c| c.anim_values().0) else { continue };
                let sample = |list: &[Value], t: f32, key: &str, d: f32| -> f32 {
                    let pts: Vec<(f32, f32)> = list.iter().map(|k| (f(k, "time", 0.0), f(k, key, d))).collect();
                    if pts.is_empty() {
                        return d;
                    }
                    if t <= pts[0].0 {
                        return pts[0].1;
                    }
                    for w in pts.windows(2) {
                        if t >= w[0].0 && t <= w[1].0 {
                            let u = if w[1].0 > w[0].0 { (t - w[0].0) / (w[1].0 - w[0].0) } else { 0.0 };
                            return w[0].1 + (w[1].1 - w[0].1) * u;
                        }
                    }
                    pts.last().unwrap().1
                };
                let (pos, spc, mix) = (arr(tl, "position"), arr(tl, "spacing"), arr(tl, "mix"));
                let mut times: Vec<f32> = pos.iter().chain(spc).chain(mix).map(|k| f(k, "time", 0.0)).collect();
                times.sort_by(|a, b| a.partial_cmp(b).unwrap());
                times.dedup_by(|a, b| (*a - *b).abs() < 1e-4);
                let out = times
                    .iter()
                    .map(|t| {
                        let v = vec![
                            sample(pos, *t, "value", base[0]),
                            sample(spc, *t, "value", base[1]),
                            sample(mix, *t, "mixRotate", 1.0),
                            sample(mix, *t, "mixX", 1.0),
                        ];
                        (*t, v, true, Curve::Linear)
                    })
                    .collect();
                con_keys(&mut a, name, out);
            }
            for (name, tl) in av.get("physics").and_then(|x| x.as_object()).into_iter().flatten() {
                let keys = arr(tl, "mix");
                let vals: Vec<(f32, f32)> = keys.iter().map(|k| (f(k, "time", 0.0), f(k, "value", 1.0))).collect();
                let out = keys
                    .iter()
                    .enumerate()
                    .map(|(i, k)| {
                        let (t, m) = vals[i];
                        let (tb, mb) = vals.get(i + 1).copied().unwrap_or((t, m));
                        (t, vec![m], true, read_curve(k, t, tb, m, mb))
                    })
                    .collect();
                con_keys(&mut a, name, out);
            }
            // orden de dibujo animado
            let setup_order = doc.draw_order.clone();
            for k in arr(av, "drawOrder").iter().chain(arr(av, "draworder")) {
                let fr = frame_of(f(k, "time", 0.0));
                let n = setup_order.len();
                let offs = arr(k, "offsets");
                let order = if offs.is_empty() {
                    setup_order.clone()
                } else {
                    let mut draw: Vec<i64> = vec![-1; n];
                    let mut unchanged: Vec<usize> = Vec::new();
                    let mut orig = 0usize;
                    for o in offs {
                        let Some(si) = s(o, "slot").and_then(|nm| slot_by_name.get(nm)).and_then(|id| setup_order.iter().position(|x| x == id)) else { continue };
                        while orig != si && orig < n {
                            unchanged.push(orig);
                            orig += 1;
                        }
                        let off = o.get("offset").and_then(|x| x.as_i64()).unwrap_or(0);
                        let target = orig as i64 + off;
                        if target >= 0 && (target as usize) < n {
                            draw[target as usize] = orig as i64;
                        }
                        orig += 1;
                    }
                    while orig < n {
                        unchanged.push(orig);
                        orig += 1;
                    }
                    for i in (0..n).rev() {
                        if draw[i] == -1 {
                            draw[i] = unchanged.pop().map(|x| x as i64).unwrap_or(i as i64);
                        }
                    }
                    draw.iter().map(|i| setup_order[*i as usize]).collect()
                };
                a.set_draw_key(fr, order);
            }
            for e in arr(av, "events") {
                let fr = frame_of(f(e, "time", 0.0));
                let name = s(e, "name").unwrap_or("evento").to_string();
                let def = doc.events.iter().find(|d| d.name == name).cloned().unwrap_or_default();
                a.events.push(EventKey {
                    frame: fr,
                    name,
                    int: e.get("int").and_then(|x| x.as_i64()).map(|x| x as i32).unwrap_or(def.int),
                    float: e.get("float").and_then(|x| x.as_f64()).map(|x| x as f32).unwrap_or(def.float),
                    string: s(e, "string").map(|x| x.to_string()).unwrap_or(def.string),
                });
                a.grow(fr);
            }
            doc.animations.push(a);
        }
    }
    if doc.animations.is_empty() {
        doc.animations.push(Animation::new("animación"));
    }
    doc.version = 2;
    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exportar_e_importar_conserva_lo_esencial() {
        let d = crate::sample::sample_doc();
        let dir = std::env::temp_dir().join("huesos_spine_test");
        let _ = std::fs::create_dir_all(&dir);
        let p = dir.join("muneco.json");
        let n = export(&d, &p).expect("exportar");
        assert!(n > 0);
        assert!(dir.join("muneco.atlas").is_file());
        assert!(dir.join("muneco.png").is_file());
        let back = import(&p).expect("importar");
        assert_eq!(back.bones.len(), d.bones.len());
        assert_eq!(back.slots.len(), d.slots.len());
        assert_eq!(back.skins.len(), d.skins.len());
        assert_eq!(back.constraints.len(), d.constraints.len());
        assert_eq!(back.animations.len(), d.animations.len());
        assert_eq!(back.events.len(), d.events.len());
        // las imágenes de las formas se recuperan desde el atlas
        let img_atts = back.skins.iter().flat_map(|s| s.entries.iter()).filter(|e| e.att.kind == AttachKind::Image).count();
        assert!(img_atts > 0, "no se recortaron regiones del atlas");
        // la pose de cada animación debe coincidir
        for (ai, a) in d.animations.iter().enumerate() {
            let a2 = back.animations.iter().find(|x| x.name == a.name).unwrap();
            let _ = ai;
            for fr in [0.0, 5.0, 7.0, 13.0] {
                let w1 = d.world_fk(Some(a), fr);
                let w2 = back.world_fk(Some(a2), fr);
                for b in &d.bones {
                    let id2 = back.bones.iter().find(|x| x.name == b.name).unwrap().id;
                    let (p1, p2) = (w1[&b.id].origin(), w2[&id2].origin());
                    assert!((p1[0] - p2[0]).abs() < 0.1 && (p1[1] - p2[1]).abs() < 0.1, "{} f{fr} {}: {:?} vs {:?}", a.name, b.name, p1, p2);
                }
            }
        }
    }
}

#[cfg(test)]
mod spine_check {
    use super::*;

    /// Escribe el JSON exportado y la pose esperada para compararla con el runtime oficial.
    /// Solo se ejecuta si existe la variable SPINE_CHECK_DIR.
    #[test]
    fn volcar_pose_para_runtime() {
        let Ok(dir) = std::env::var("SPINE_CHECK_DIR") else { return };
        let dir = PathBuf::from(dir);
        for (name, d) in [("muneco", crate::sample::sample_doc()), ("demo", crate::sample::demo_doc())] {
            export(&d, &dir.join(format!("{name}.json"))).unwrap();
            let setup = d.world_fk(None, 0.0);
            let mut anims = Map::new();
            for a in &d.animations {
                let mut frames = Vec::new();
                for fr in [0.0f32, 7.0, 15.0, 20.0, 30.0, 41.0, 52.0] {
                    let pose = crate::pose::solve(&d, Some(a), fr, 0, None);
                    let mut bones = Map::new();
                    for b in &d.bones {
                        let w = pose.world[&b.id];
                        bones.insert(b.name.clone(), json!([w.tx, w.ty, w.angle_deg()]));
                    }
                    let mut slots = Map::new();
                    for s in &d.slots {
                        let sp = &pose.slots[&s.id];
                        let mut verts = Value::Null;
                        if let Some(n) = &sp.att {
                            if let Some((_, att)) = d.find_att(0, s.id, n) {
                                if let Some(m) = &att.mesh {
                                    let def = a.deform_at(s.id, n, fr);
                                    let v = crate::pose::mesh_world(&d, &pose.world, &setup, s, m, def.as_deref());
                                    verts = json!(v.iter().flat_map(|p| [p[0], p[1]]).collect::<Vec<f32>>());
                                }
                            }
                        }
                        slots.insert(s.name.clone(), json!({ "att": sp.att, "color": hex(sp.color), "verts": verts }));
                    }
                    frames.push(json!({ "time": fr / a.fps, "bones": bones, "slots": slots }));
                }
                anims.insert(a.name.clone(), Value::Array(frames));
            }
            std::fs::write(dir.join(format!("{name}.expected.json")), serde_json::to_string(&Value::Object(anims)).unwrap()).unwrap();
        }
    }

    /// Importa un JSON de Spine real y vuelca nuestra pose para compararla con el runtime oficial.
    #[test]
    fn volcar_importado() {
        let (Ok(src), Ok(out)) = (std::env::var("SPINE_IMPORT"), std::env::var("SPINE_IMPORT_OUT")) else { return };
        let d = import(Path::new(&src)).expect("importar");
        let setup = d.world_fk(None, 0.0);
        let mut anims = Map::new();
        for a in &d.animations {
            let mut frames = Vec::new();
            let mut fr = 0.0f32;
            while fr <= a.duration as f32 {
                let pose = crate::pose::solve(&d, Some(a), fr, 0, None);
                let mut bones = Map::new();
                for b in &d.bones {
                    let w = pose.world[&b.id];
                    bones.insert(b.name.clone(), json!([w.tx, w.ty, w.angle_deg()]));
                }
                let mut slots = Map::new();
                for s in &d.slots {
                    let sp = &pose.slots[&s.id];
                    let mut verts = Value::Null;
                    if let Some(n) = &sp.att {
                        if let Some((_, att)) = d.find_att(0, s.id, n) {
                            if let Some(m) = att.mesh.as_ref().filter(|_| att.kind == AttachKind::Mesh) {
                                let def = a.deform_at(s.id, n, fr);
                                let v = crate::pose::mesh_world(&d, &pose.world, &setup, s, m, def.as_deref());
                                verts = json!(v.iter().flat_map(|p| [p[0], p[1]]).collect::<Vec<f32>>());
                            }
                        }
                    }
                    slots.insert(s.name.clone(), json!({ "att": sp.att, "color": hex(sp.color), "verts": verts }));
                }
                let order: Vec<String> = pose.draw_order.iter().filter_map(|s| d.slot(*s).map(|x| x.name.clone())).collect();
                frames.push(json!({ "time": fr / a.fps, "bones": bones, "slots": slots, "order": order }));
                fr += 3.0;
            }
            anims.insert(a.name.clone(), Value::Array(frames));
        }
        std::fs::write(out, serde_json::to_string(&Value::Object(anims)).unwrap()).unwrap();
    }
}
