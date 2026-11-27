//! Mallas: creación en rejilla, pesos automáticos y edición de vértices.

use crate::model::*;
use std::collections::HashMap;

/// Crea una malla en rejilla que cubre el rectángulo del adjunto.
pub fn grid(att: &Attachment, cols: usize, rows: usize) -> MeshData {
    let cols = cols.clamp(1, 40);
    let rows = rows.clamp(1, 40);
    let (hw, hh) = (att.w / 2.0, att.h / 2.0);
    let m = att.local_aff();
    let mut order: Vec<(usize, usize)> = Vec::new();
    for i in 0..cols {
        order.push((i, 0));
    }
    for j in 0..rows {
        order.push((cols, j));
    }
    for i in (1..=cols).rev() {
        order.push((i, rows));
    }
    for j in (1..=rows).rev() {
        order.push((0, j));
    }
    let hull = order.len();
    for j in 1..rows {
        for i in 1..cols {
            order.push((i, j));
        }
    }
    let mut index: HashMap<(usize, usize), u32> = HashMap::new();
    let mut verts = Vec::new();
    let mut uvs = Vec::new();
    for (n, &(i, j)) in order.iter().enumerate() {
        index.insert((i, j), n as u32);
        let u = i as f32 / cols as f32;
        let v = j as f32 / rows as f32; // 0 abajo
        let x = -hw + u * att.w;
        let y = -hh + v * att.h;
        verts.push(m.apply(x, y));
        uvs.push([u, 1.0 - v]);
    }
    let mut tris = Vec::new();
    for j in 0..rows {
        for i in 0..cols {
            let a = index[&(i, j)];
            let b = index[&(i + 1, j)];
            let c = index[&(i + 1, j + 1)];
            let d = index[&(i, j + 1)];
            tris.push([a, b, c]);
            tris.push([a, c, d]);
        }
    }
    MeshData { verts, uvs, tris, hull, weights: Vec::new() }
}

fn dist_to_bone(p: [f32; 2], w: &Aff, len: f32) -> f32 {
    let a = w.origin();
    let b = w.apply(len, 0.0);
    let ab = [b[0] - a[0], b[1] - a[1]];
    let l2 = ab[0] * ab[0] + ab[1] * ab[1];
    let t = if l2 < 1e-6 { 0.0 } else { (((p[0] - a[0]) * ab[0] + (p[1] - a[1]) * ab[1]) / l2).clamp(0.0, 1.0) };
    let q = [a[0] + ab[0] * t, a[1] + ab[1] * t];
    ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2)).sqrt()
}

/// Asigna pesos según la distancia de cada vértice a los huesos elegidos.
pub fn auto_weights(doc: &Doc, slot_bone: u32, mesh: &mut MeshData, bones: &[u32]) {
    if bones.is_empty() {
        mesh.weights.clear();
        return;
    }
    let setup = doc.world_fk(None, 0.0);
    let sb = setup.get(&slot_bone).copied().unwrap_or(Aff::IDENTITY);
    mesh.weights = mesh
        .verts
        .iter()
        .map(|v| {
            let p = sb.apply(v[0], v[1]);
            let mut cand: Vec<(u32, f32)> = bones
                .iter()
                .filter_map(|b| {
                    let w = setup.get(b)?;
                    let len = doc.bone(*b).map(|bb| bb.length).unwrap_or(0.0);
                    let d = dist_to_bone(p, w, len);
                    Some((*b, 1.0 / (d * d + 25.0)))
                })
                .collect();
            cand.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            cand.truncate(3);
            let sum: f32 = cand.iter().map(|c| c.1).sum::<f32>().max(1e-9);
            cand.iter()
                .map(|(b, w)| {
                    let bs = setup[b];
                    let l = bs.inv_point(p[0], p[1]);
                    Weight { bone: *b, x: l[0], y: l[1], w: w / sum }
                })
                .collect()
        })
        .collect();
}

/// Mueve un vértice de la pose base a la posición de mundo `p`.
pub fn set_setup_vertex(doc: &Doc, slot_bone: u32, mesh: &mut MeshData, i: usize, p: [f32; 2]) {
    let setup = doc.world_fk(None, 0.0);
    let sb = setup.get(&slot_bone).copied().unwrap_or(Aff::IDENTITY);
    if let Some(v) = mesh.verts.get_mut(i) {
        *v = sb.inv_point(p[0], p[1]);
    }
    if mesh.weighted() {
        for w in &mut mesh.weights[i] {
            if let Some(bs) = setup.get(&w.bone) {
                let l = bs.inv_point(p[0], p[1]);
                w.x = l[0];
                w.y = l[1];
            }
        }
    }
}

/// Vértices de la malla en la pose base (espacio de mundo).
#[allow(dead_code)]
pub fn setup_verts(doc: &Doc, slot_bone: u32, mesh: &MeshData) -> Vec<[f32; 2]> {
    let setup = doc.world_fk(None, 0.0);
    let sb = setup.get(&slot_bone).copied().unwrap_or(Aff::IDENTITY);
    mesh.verts.iter().map(|v| sb.apply(v[0], v[1])).collect()
}
