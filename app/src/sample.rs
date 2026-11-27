//! Proyecto de ejemplo: un muñeco sencillo con dos animaciones.

use crate::model::*;

pub fn sample_doc() -> Doc {
    let mut d = Doc::empty();
    let root = 0;
    let skin = [240, 200, 160, 255];
    let shirt = [70, 140, 220, 255];
    let shirt_back = [50, 105, 170, 255];
    let pants = [60, 60, 80, 255];
    let pants_back = [45, 45, 60, 255];

    let hip = d.add_bone(Some(root), "cadera".into(), 0.0, 110.0, 90.0, 18.0);
    let torso = d.add_bone(Some(hip), "torso".into(), 14.0, 0.0, 0.0, 80.0);
    let head = d.add_bone(Some(torso), "cabeza".into(), 82.0, 0.0, 0.0, 44.0);
    let arm_b = d.add_bone(Some(torso), "brazo_atrás".into(), 72.0, 0.0, 170.0, 44.0);
    let fore_b = d.add_bone(Some(arm_b), "antebrazo_atrás".into(), 44.0, 0.0, -10.0, 40.0);
    let arm_f = d.add_bone(Some(torso), "brazo_frente".into(), 72.0, 0.0, -170.0, 44.0);
    let fore_f = d.add_bone(Some(arm_f), "antebrazo_frente".into(), 44.0, 0.0, 10.0, 40.0);
    let thigh_b = d.add_bone(Some(hip), "muslo_atrás".into(), 0.0, 0.0, 175.0, 55.0);
    let shin_b = d.add_bone(Some(thigh_b), "pierna_atrás".into(), 55.0, 0.0, 0.0, 55.0);
    let thigh_f = d.add_bone(Some(hip), "muslo_frente".into(), 0.0, 0.0, -175.0, 55.0);
    let shin_f = d.add_bone(Some(thigh_f), "pierna_frente".into(), 55.0, 0.0, 0.0, 55.0);

    let colors: [(u32, [u8; 4]); 11] = [
        (hip, [230, 170, 60, 255]),
        (torso, [230, 170, 60, 255]),
        (head, [240, 120, 120, 255]),
        (arm_b, [120, 200, 120, 255]),
        (fore_b, [120, 200, 120, 255]),
        (arm_f, [120, 200, 120, 255]),
        (fore_f, [120, 200, 120, 255]),
        (thigh_b, [140, 160, 240, 255]),
        (shin_b, [140, 160, 240, 255]),
        (thigh_f, [140, 160, 240, 255]),
        (shin_f, [140, 160, 240, 255]),
    ];
    for (id, c) in colors {
        d.bone_mut(id).unwrap().color = c;
    }

    let set = |d: &mut Doc, id: u32, a: Attachment| d.bone_mut(id).unwrap().attachment = Some(a);
    set(&mut d, hip, Attachment::rect(30.0, 46.0, 6.0, 0.0, pants));
    set(&mut d, torso, Attachment::rect(92.0, 50.0, 42.0, 0.0, shirt));
    set(&mut d, head, Attachment::ellipse(54.0, 52.0, 24.0, 0.0, skin));
    set(&mut d, arm_b, Attachment::rect(54.0, 18.0, 22.0, 0.0, shirt_back));
    set(&mut d, fore_b, Attachment::rect(50.0, 15.0, 22.0, 0.0, [215, 175, 140, 255]));
    set(&mut d, arm_f, Attachment::rect(54.0, 18.0, 22.0, 0.0, shirt));
    set(&mut d, fore_f, Attachment::rect(50.0, 15.0, 22.0, 0.0, skin));
    set(&mut d, thigh_b, Attachment::rect(64.0, 22.0, 27.0, 0.0, pants_back));
    set(&mut d, shin_b, Attachment::rect(64.0, 19.0, 28.0, 0.0, pants_back));
    set(&mut d, thigh_f, Attachment::rect(64.0, 22.0, 27.0, 0.0, pants));
    set(&mut d, shin_f, Attachment::rect(64.0, 19.0, 28.0, 0.0, pants));

    d.draw_order = vec![root, arm_b, fore_b, thigh_b, shin_b, hip, torso, head, thigh_f, shin_f, arm_f, fore_f];

    // Animación de caminar
    let mut walk = Animation::new("caminar");
    walk.duration = 32;
    let k = |a: &mut Animation, id: u32, ch: Channel, frames: &[(u32, f32, f32)]| {
        for &(f, x, y) in frames {
            a.set_key(id, ch, f, [x, y]);
        }
        if let Some(t) = a.tracks.iter_mut().find(|t| t.bone == id && t.channel == ch) {
            for key in &mut t.keys {
                key.curve = Curve::Smooth;
            }
        }
    };
    k(&mut walk, thigh_f, Channel::Rotate, &[(0, -150.0, 0.0), (16, -205.0, 0.0), (32, -150.0, 0.0)]);
    k(&mut walk, thigh_b, Channel::Rotate, &[(0, 155.0, 0.0), (16, 210.0, 0.0), (32, 155.0, 0.0)]);
    k(&mut walk, shin_f, Channel::Rotate, &[(0, 0.0, 0.0), (8, -10.0, 0.0), (16, -35.0, 0.0), (24, -5.0, 0.0), (32, 0.0, 0.0)]);
    k(&mut walk, shin_b, Channel::Rotate, &[(0, -35.0, 0.0), (8, -5.0, 0.0), (16, 0.0, 0.0), (24, -10.0, 0.0), (32, -35.0, 0.0)]);
    k(&mut walk, arm_f, Channel::Rotate, &[(0, 155.0, 0.0), (16, 205.0, 0.0), (32, 155.0, 0.0)]);
    k(&mut walk, arm_b, Channel::Rotate, &[(0, 205.0, 0.0), (16, 155.0, 0.0), (32, 205.0, 0.0)]);
    k(&mut walk, fore_f, Channel::Rotate, &[(0, 25.0, 0.0), (16, 5.0, 0.0), (32, 25.0, 0.0)]);
    k(&mut walk, fore_b, Channel::Rotate, &[(0, 5.0, 0.0), (16, 25.0, 0.0), (32, 5.0, 0.0)]);
    k(&mut walk, hip, Channel::Translate, &[(0, 0.0, 104.0), (8, 0.0, 112.0), (16, 0.0, 104.0), (24, 0.0, 112.0), (32, 0.0, 104.0)]);
    k(&mut walk, torso, Channel::Rotate, &[(0, -4.0, 0.0), (16, -4.0, 0.0), (32, -4.0, 0.0)]);
    k(&mut walk, head, Channel::Rotate, &[(0, 2.0, 0.0), (8, -2.0, 0.0), (16, 2.0, 0.0), (24, -2.0, 0.0), (32, 2.0, 0.0)]);

    // Animación de saludar
    let mut wave = Animation::new("saludar");
    wave.duration = 40;
    k(&mut wave, arm_f, Channel::Rotate, &[(0, -170.0, 0.0), (8, -40.0, 0.0), (32, -40.0, 0.0), (40, -170.0, 0.0)]);
    k(&mut wave, fore_f, Channel::Rotate, &[(0, 10.0, 0.0), (8, 60.0, 0.0), (14, 20.0, 0.0), (20, 60.0, 0.0), (26, 20.0, 0.0), (32, 60.0, 0.0), (40, 10.0, 0.0)]);
    k(&mut wave, head, Channel::Rotate, &[(0, 0.0, 0.0), (10, 8.0, 0.0), (30, 8.0, 0.0), (40, 0.0, 0.0)]);
    k(&mut wave, torso, Channel::Scale, &[(0, 1.0, 1.0), (20, 1.03, 1.0), (40, 1.0, 1.0)]);

    // Pose inactiva
    let mut idle = Animation::new("respirar");
    idle.duration = 60;
    k(&mut idle, torso, Channel::Scale, &[(0, 1.0, 1.0), (30, 1.04, 1.03), (60, 1.0, 1.0)]);
    k(&mut idle, head, Channel::Rotate, &[(0, 0.0, 0.0), (30, -4.0, 0.0), (60, 0.0, 0.0)]);
    k(&mut idle, arm_f, Channel::Rotate, &[(0, -170.0, 0.0), (30, -165.0, 0.0), (60, -170.0, 0.0)]);
    k(&mut idle, arm_b, Channel::Rotate, &[(0, 170.0, 0.0), (30, 165.0, 0.0), (60, 170.0, 0.0)]);

    d.animations = vec![walk, wave, idle];
    d.version = 0;
    d.migrate();
    let slot_of = |d: &Doc, name: &str| d.slots.iter().find(|s| s.name == name).map(|s| s.id).unwrap();

    // coleta con física
    let hair = [110, 70, 40, 255];
    let c1 = d.add_bone(Some(head), "coleta1".into(), 34.0, -24.0, 215.0, 24.0);
    let c2 = d.add_bone(Some(c1), "coleta2".into(), 24.0, 0.0, 15.0, 22.0);
    let c3 = d.add_bone(Some(c2), "coleta3".into(), 22.0, 0.0, 15.0, 20.0);
    let head_slot = slot_of(&d, "cabeza");
    let mut hair_slots = Vec::new();
    for (b, w) in [(c1, 32.0), (c2, 30.0), (c3, 27.0)] {
        d.bone_mut(b).unwrap().color = [200, 140, 90, 255];
        let name = d.bones.iter().find(|x| x.id == b).unwrap().name.clone();
        let s = d.add_slot(b, name.clone());
        d.put_attachment(0, s, &name, Attachment::ellipse(w, w * 0.6, w * 0.45, 0.0, hair));
        d.slot_mut(s).unwrap().attachment = Some(name.clone());
        hair_slots.push(s);
        d.constraints.push(Constraint {
            name: format!("física_{name}"),
            enabled: true,
            bones: vec![b],
            kind: ConstraintKind::Physics { inertia: 0.7, strength: 70.0, damping: 5.0, mass: 1.0, gravity: 8.0, wind: 0.0, mix: 1.0 },
        });
    }
    // la coleta va detrás de la cabeza
    d.draw_order.retain(|s| !hair_slots.contains(s));
    let hp = d.draw_order.iter().position(|s| *s == head_slot).unwrap_or(0);
    for (i, s) in hair_slots.iter().enumerate() {
        d.draw_order.insert(hp + i, *s);
    }

    // boca con dos adjuntos
    let mouth = d.add_slot(head, "boca".into());
    d.put_attachment(0, mouth, "boca_cerrada", Attachment::rect(4.0, 12.0, 14.0, 12.0, [120, 50, 50, 255]));
    d.put_attachment(0, mouth, "boca_abierta", Attachment::ellipse(9.0, 12.0, 14.0, 12.0, [90, 30, 30, 255]));
    d.slot_mut(mouth).unwrap().attachment = Some("boca_cerrada".into());

    // skin alternativa: camiseta roja
    let red = [210, 70, 60, 255];
    let red_back = [160, 50, 45, 255];
    let mut skin = Skin { name: "camiseta_roja".into(), entries: Vec::new() };
    for (slot, col) in [("torso", red), ("brazo_frente", red), ("brazo_atrás", red_back)] {
        let sid = slot_of(&d, slot);
        if let Some((_, a)) = d.find_att(0, sid, slot) {
            let mut a = a.clone();
            a.tint = col;
            skin.entries.push(SkinEntry { slot: sid, name: slot.into(), att: a });
        }
    }
    d.skins.push(skin);

    // eventos
    d.events.push(EventDef { name: "paso".into(), int: 0, float: 1.0, string: String::new(), audio: "paso.ogg".into() });
    d.animations[0].events = vec![
        EventKey { frame: 0, name: "paso".into(), int: 1, float: 1.0, string: "izquierdo".into() },
        EventKey { frame: 16, name: "paso".into(), int: 2, float: 1.0, string: "derecho".into() },
    ];
    // saludar: abre la boca
    d.animations[1].set_att_key(mouth, 0, Some("boca_cerrada".into()));
    d.animations[1].set_att_key(mouth, 8, Some("boca_abierta".into()));
    d.animations[1].set_att_key(mouth, 32, Some("boca_cerrada".into()));
    d
}

/// Segundo ejemplo: IK, transformación, trayectoria, física, malla con pesos y deformación.
pub fn demo_doc() -> Doc {
    let mut d = Doc::empty();
    let root = 0;
    let shape = |d: &mut Doc, bone: u32, name: &str, a: Attachment| -> u32 {
        let s = d.add_slot(bone, name.to_string());
        d.put_attachment(0, s, name, a);
        d.slot_mut(s).unwrap().attachment = Some(name.to_string());
        s
    };

    // --- brazo con IK de dos huesos
    let hombro = d.add_bone(Some(root), "hombro".into(), -220.0, 60.0, 0.0, 0.0);
    let sup = d.add_bone(Some(hombro), "brazo_sup".into(), 0.0, 0.0, -30.0, 90.0);
    let inf = d.add_bone(Some(sup), "brazo_inf".into(), 90.0, 0.0, 40.0, 80.0);
    let objetivo = d.add_bone(Some(root), "objetivo".into(), -80.0, 20.0, 0.0, 0.0);
    for (b, c) in [(sup, [120, 200, 120, 255]), (inf, [120, 200, 120, 255]), (objetivo, [250, 90, 160, 255])] {
        d.bone_mut(b).unwrap().color = c;
    }
    shape(&mut d, sup, "brazo_sup", Attachment::rect(96.0, 22.0, 45.0, 0.0, [90, 170, 110, 255]));
    shape(&mut d, inf, "brazo_inf", Attachment::rect(86.0, 18.0, 40.0, 0.0, [110, 190, 130, 255]));
    shape(&mut d, inf, "mano", Attachment::ellipse(26.0, 26.0, 82.0, 0.0, [240, 200, 160, 255]));
    d.constraints.push(Constraint { name: "ik_brazo".into(), enabled: true, bones: vec![sup, inf], kind: ConstraintKind::Ik { target: objetivo, mix: 1.0, bend_positive: true, softness: 0.0, stretch: false, compress: false, uniform: false } });

    // --- ruedas: la pequeña copia la rotación de la grande
    let rueda = d.add_bone(Some(root), "rueda".into(), 0.0, -120.0, 0.0, 50.0);
    let rueda2 = d.add_bone(Some(root), "rueda_copia".into(), 110.0, -120.0, 0.0, 30.0);
    shape(&mut d, rueda, "rueda", Attachment::rect(100.0, 100.0, 0.0, 0.0, [230, 170, 60, 255]));
    shape(&mut d, rueda2, "rueda_copia", Attachment::rect(60.0, 60.0, 0.0, 0.0, [200, 130, 50, 255]));
    d.constraints.push(Constraint {
        name: "copiar_giro".into(),
        enabled: true,
        bones: vec![rueda2],
        kind: ConstraintKind::Transform { target: rueda, mix_rotate: 1.0, mix_translate: 0.0, mix_scale: 0.0, off_rot: 45.0, off_x: 0.0, off_y: 0.0, off_sx: 0.0, off_sy: 0.0, mix_y: None, mix_scale_y: None, mix_shear: 0.0, off_shy: 0.0 },
    });

    // --- serpiente que sigue una trayectoria
    let camino = d.add_bone(Some(root), "camino".into(), 60.0, 80.0, 0.0, 0.0);
    let ps = d.add_slot(camino, "camino".into());
    let mut path = Attachment::path_line(10.0);
    path.path = Some(PathData {
        pts: vec![
            PathPt { p: [0.0, 0.0], hin: [-30.0, 0.0], hout: [60.0, 90.0] },
            PathPt { p: [160.0, 40.0], hin: [-50.0, 40.0], hout: [50.0, -40.0] },
            PathPt { p: [300.0, -20.0], hin: [-60.0, -40.0], hout: [30.0, 0.0] },
        ],
        closed: false,
    });
    d.put_attachment(0, ps, "camino", path);
    d.slot_mut(ps).unwrap().attachment = Some("camino".into());
    let mut chain = Vec::new();
    let mut parent = root;
    for i in 0..6 {
        let b = d.add_bone(Some(parent), format!("serpiente{}", i + 1), if i == 0 { 60.0 } else { 34.0 }, if i == 0 { 80.0 } else { 0.0 }, 0.0, 34.0);
        d.bone_mut(b).unwrap().color = [140, 200, 90, 255];
        let w = 30.0 - i as f32 * 3.0;
        shape(&mut d, b, &format!("serpiente{}", i + 1), Attachment::ellipse(40.0, w, 17.0, 0.0, [120 + i * 15, 190, 80, 255]));
        chain.push(b);
        parent = b;
    }
    d.constraints.push(Constraint {
        name: "seguir_camino".into(),
        enabled: true,
        bones: chain.clone(),
        kind: ConstraintKind::Path { slot: ps, position: 0.1, spacing: 0.0, spacing_mode: SpacingMode::Length, rotate_mode: RotateMode::Tangent, off_rot: 0.0, mix_rotate: 1.0, mix_translate: 1.0 },
    });

    // --- bandera: malla con pesos sobre tres huesos
    let mastil = d.add_bone(Some(root), "mástil".into(), -40.0, 120.0, 0.0, 0.0);
    let t1 = d.add_bone(Some(mastil), "tela1".into(), 0.0, 0.0, 0.0, 50.0);
    let t2 = d.add_bone(Some(t1), "tela2".into(), 50.0, 0.0, 0.0, 50.0);
    let t3 = d.add_bone(Some(t2), "tela3".into(), 50.0, 0.0, 0.0, 50.0);
    for b in [t1, t2, t3] {
        d.bone_mut(b).unwrap().color = [100, 160, 240, 255];
    }
    let mut flag = Attachment::rect(150.0, 70.0, 75.0, 0.0, [80, 140, 230, 255]);
    let mesh = crate::mesh::grid(&flag, 6, 3);
    flag.kind = AttachKind::Mesh;
    flag.mesh = Some(mesh);
    let fs = shape(&mut d, t1, "bandera", flag);
    {
        let mut m = d.find_att(0, fs, "bandera").unwrap().1.mesh.clone().unwrap();
        crate::mesh::auto_weights(&d, t1, &mut m, &[t1, t2, t3]);
        d.find_att_mut(0, fs, "bandera").unwrap().mesh = Some(m);
    }

    // --- antena con física
    let antena = d.add_bone(Some(rueda), "antena".into(), 0.0, 50.0, 90.0, 60.0);
    d.bone_mut(antena).unwrap().color = [180, 140, 255, 255];
    shape(&mut d, antena, "antena", Attachment::rect(60.0, 6.0, 30.0, 0.0, [180, 140, 255, 255]));
    d.constraints.push(Constraint {
        name: "física_antena".into(),
        enabled: true,
        bones: vec![antena],
        kind: ConstraintKind::Physics { inertia: 0.8, strength: 60.0, damping: 4.0, mass: 1.0, gravity: 0.0, wind: 0.0, mix: 1.0 },
    });

    // --- animación
    let mut a = Animation::new("demostración");
    a.duration = 60;
    let bz = Curve::Bezier([0.42, 0.0, 0.58, 1.0]);
    for (f, x, y) in [(0, -80.0, 20.0), (20, -60.0, -70.0), (40, -150.0, -40.0), (60, -80.0, 20.0)] {
        a.set_key(objetivo, Channel::Translate, f, [x, y]);
    }
    for k in &mut a.tracks.last_mut().unwrap().keys {
        k.curve = bz;
    }
    a.set_key(rueda, Channel::Rotate, 0, [0.0, 0.0]);
    a.set_key(rueda, Channel::Rotate, 60, [-360.0, 0.0]);
    a.set_key(rueda, Channel::Translate, 0, [0.0, -120.0]);
    a.set_key(rueda, Channel::Translate, 30, [60.0, -120.0]);
    a.set_key(rueda, Channel::Translate, 60, [0.0, -120.0]);
    a.set_key(camino, Channel::Rotate, 0, [0.0, 0.0]);
    a.set_key(camino, Channel::Rotate, 30, [-12.0, 0.0]);
    a.set_key(camino, Channel::Rotate, 60, [0.0, 0.0]);
    for (b, ph) in [(t1, 0.0f32), (t2, 1.0), (t3, 2.0)] {
        for f in [0u32, 15, 30, 45, 60] {
            let v = ((f as f32 / 60.0) * std::f32::consts::TAU * 2.0 + ph).sin() * 14.0;
            a.set_key(b, Channel::Rotate, f, [v, 0.0]);
        }
    }
    for t in &mut a.tracks {
        if t.bone == t1 || t.bone == t2 || t.bone == t3 {
            for k in &mut t.keys {
                k.curve = Curve::Smooth;
            }
        }
    }
    // deformación: la punta de la bandera se ondula
    let nv = d.find_att(0, fs, "bandera").unwrap().1.mesh.as_ref().unwrap().verts.len();
    let mut off = vec![[0.0f32, 0.0]; nv];
    for (i, o) in off.iter_mut().enumerate().take(10) {
        o[1] = if i % 2 == 0 { 10.0 } else { -6.0 };
    }
    a.set_deform_key(fs, "bandera", 0, vec![[0.0, 0.0]; nv]);
    a.set_deform_key(fs, "bandera", 30, off);
    a.set_deform_key(fs, "bandera", 60, vec![[0.0, 0.0]; nv]);
    // color de la mano
    let mano = d.slots.iter().find(|s| s.name == "mano").unwrap().id;
    a.set_color_key(mano, 0, [255, 255, 255, 255]);
    a.set_color_key(mano, 30, [255, 120, 120, 255]);
    a.set_color_key(mano, 60, [255, 255, 255, 255]);
    d.animations = vec![a];
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guardar_y_abrir_conserva_el_proyecto() {
        let d = sample_doc();
        let s = serde_json::to_string(&d).unwrap();
        let back: Doc = serde_json::from_str(&s).unwrap();
        assert_eq!(d, back);
    }

    #[test]
    fn la_interpolacion_llega_a_las_claves() {
        let d = sample_doc();
        let a = &d.animations[0];
        let t = &a.tracks[0];
        assert_eq!(t.sample(t.keys[0].frame as f32), Some(t.keys[0].v));
        assert_eq!(t.sample(10_000.0), Some(t.keys.last().unwrap().v));
        let w = d.world(Some(a), 5.0);
        assert_eq!(w.len(), d.bones.len());
    }
}

#[cfg(test)]
mod tests_nuevas {
    use super::*;

    #[test]
    fn proyecto_de_la_version_anterior_se_convierte() {
        let old = r#"{"bones":[
            {"id":0,"name":"raíz","parent":null,"x":0,"y":0,"rotation":0,"length":0,"sx":1,"sy":1,"color":[200,200,200,255],"visible":true,"attachment":null},
            {"id":1,"name":"brazo","parent":0,"x":0,"y":10,"rotation":90,"length":40,"sx":1,"sy":1,"color":[1,2,3,255],"visible":true,
             "attachment":{"kind":"Rect","image":"","w":40,"h":10,"ox":20,"oy":0,"rot":0,"tint":[255,0,0,255]}}],
            "draw_order":[0,1],
            "animations":[{"name":"a","duration":10,"fps":30,"tracks":[{"bone":1,"channel":"Rotate","keys":[{"frame":0,"v":[90,0],"curve":"Smooth"},{"frame":10,"v":[0,0],"curve":"Linear"}]}]}],
            "next_id":2}"#;
        let mut d: Doc = serde_json::from_str(old).unwrap();
        d.migrate();
        assert_eq!(d.slots.len(), 1);
        assert_eq!(d.slots[0].bone, 1);
        assert_eq!(d.draw_order, vec![d.slots[0].id]);
        let (_, att) = d.find_att(0, d.slots[0].id, "brazo").unwrap();
        assert_eq!(att.tint, [255, 0, 0, 255]);
        let w = d.world(Some(&d.animations[0]), 10.0);
        assert!((w[&1].angle_deg()).abs() < 1e-3);
    }

    #[test]
    fn la_fisica_se_retrasa_al_mover_el_padre() {
        let mut d = Doc::empty();
        let base = d.add_bone(Some(0), "base".into(), 0.0, 0.0, 0.0, 0.0);
        let cola = d.add_bone(Some(base), "cola".into(), 0.0, 0.0, 0.0, 50.0);
        d.constraints.push(Constraint {
            name: "f".into(),
            enabled: true,
            bones: vec![cola],
            kind: ConstraintKind::Physics { inertia: 1.0, strength: 50.0, damping: 2.0, mass: 1.0, gravity: 0.0, wind: 0.0, mix: 1.0 },
        });
        let mut ph = crate::pose::Physics::default();
        ph.dt = 1.0 / 60.0;
        let mut a = Animation::new("mover");
        a.set_key(base, Channel::Translate, 0, [0.0, 0.0]);
        a.set_key(base, Channel::Translate, 10, [0.0, 100.0]);
        let mut max_dev = 0.0f32;
        for i in 0..=10 {
            let p = crate::pose::solve(&d, Some(&a), i as f32, 0, Some(&mut ph));
            max_dev = max_dev.max(p.world[&cola].angle_deg().abs());
        }
        assert!(max_dev > 5.0, "la cola debería quedarse atrás al subir la base ({max_dev}°)");
        // sin movimiento acaba volviendo a su sitio
        for _ in 0..600 {
            crate::pose::solve(&d, Some(&a), 10.0, 0, Some(&mut ph));
        }
        let p = crate::pose::solve(&d, Some(&a), 10.0, 0, Some(&mut ph));
        assert!(p.world[&cola].angle_deg().abs() < 1.0);
    }
}
