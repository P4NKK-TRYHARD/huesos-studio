//! Recorte de triángulos contra un polígono (cóncavo o convexo), como el clipping de Spine.

fn cross(o: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
}

fn area(p: &[[f32; 2]]) -> f32 {
    let mut s = 0.0;
    for i in 0..p.len() {
        let (a, b) = (p[i], p[(i + 1) % p.len()]);
        s += a[0] * b[1] - b[0] * a[1];
    }
    s * 0.5
}

fn in_tri(p: [f32; 2], a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> bool {
    cross(a, b, p) >= 0.0 && cross(b, c, p) >= 0.0 && cross(c, a, p) >= 0.0
}

/// Triangula un polígono simple por "orejas". Devuelve triángulos en sentido antihorario.
pub fn triangulate(poly: &[[f32; 2]]) -> Vec<[[f32; 2]; 3]> {
    let mut pts: Vec<[f32; 2]> = poly.to_vec();
    if pts.len() < 3 {
        return Vec::new();
    }
    if area(&pts) < 0.0 {
        pts.reverse();
    }
    let mut idx: Vec<usize> = (0..pts.len()).collect();
    let mut out = Vec::new();
    let mut guard = 0;
    while idx.len() > 3 && guard < 10_000 {
        guard += 1;
        let n = idx.len();
        let mut cut = false;
        for i in 0..n {
            let (ia, ib, ic) = (idx[(i + n - 1) % n], idx[i], idx[(i + 1) % n]);
            let (a, b, c) = (pts[ia], pts[ib], pts[ic]);
            if cross(a, b, c) <= 1e-7 {
                continue; // vértice reflejo
            }
            let ear = idx.iter().all(|&j| j == ia || j == ib || j == ic || !in_tri(pts[j], a, b, c));
            if ear {
                out.push([a, b, c]);
                idx.remove(i);
                cut = true;
                break;
            }
        }
        if !cut {
            // polígono degenerado: abanico de lo que queda
            for i in 1..idx.len() - 1 {
                out.push([pts[idx[0]], pts[idx[i]], pts[idx[i + 1]]]);
            }
            return out;
        }
    }
    if idx.len() == 3 {
        out.push([pts[idx[0]], pts[idx[1]], pts[idx[2]]]);
    }
    out
}

/// Vértice con coordenadas de textura.
pub type V = ([f32; 2], [f32; 2]);

/// Recorta un polígono convexo (Sutherland–Hodgman) contra un triángulo antihorario.
fn clip_convex(subject: Vec<V>, tri: &[[f32; 2]; 3]) -> Vec<V> {
    let mut out = subject;
    for e in 0..3 {
        if out.is_empty() {
            break;
        }
        let (a, b) = (tri[e], tri[(e + 1) % 3]);
        let inside = |p: [f32; 2]| cross(a, b, p) >= 0.0;
        let input = std::mem::take(&mut out);
        for i in 0..input.len() {
            let cur = input[i];
            let prev = input[(i + input.len() - 1) % input.len()];
            let (ci, pi) = (inside(cur.0), inside(prev.0));
            if ci != pi {
                let d1 = cross(a, b, prev.0);
                let d2 = cross(a, b, cur.0);
                let t = d1 / (d1 - d2);
                out.push((
                    [prev.0[0] + (cur.0[0] - prev.0[0]) * t, prev.0[1] + (cur.0[1] - prev.0[1]) * t],
                    [prev.1[0] + (cur.1[0] - prev.1[0]) * t, prev.1[1] + (cur.1[1] - prev.1[1]) * t],
                ));
            }
            if ci {
                out.push(cur);
            }
        }
    }
    out
}

/// Recorta una lista de triángulos (posición + uv) contra los triángulos del recorte.
pub fn clip_triangles(tris: &[[V; 3]], clip: &[[[f32; 2]; 3]]) -> Vec<[V; 3]> {
    let mut out = Vec::new();
    for t in tris {
        for c in clip {
            let poly = clip_convex(t.to_vec(), c);
            for i in 1..poly.len().saturating_sub(1) {
                out.push([poly[0], poly[i], poly[i + 1]]);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recorte_conserva_el_area_interior() {
        // cuadrado 0..10 recortado por una "L" (cóncava) de área 75
        let l = [[0.0, 0.0], [10.0, 0.0], [10.0, 5.0], [5.0, 5.0], [5.0, 10.0], [0.0, 10.0]];
        let clip = triangulate(&l);
        let tri_area: f32 = clip.iter().map(|t| area(t)).sum();
        assert!((tri_area - 75.0).abs() < 1e-3);
        let sq: Vec<[V; 3]> = vec![
            [([0.0, 0.0], [0.0, 0.0]), ([10.0, 0.0], [1.0, 0.0]), ([10.0, 10.0], [1.0, 1.0])],
            [([0.0, 0.0], [0.0, 0.0]), ([10.0, 10.0], [1.0, 1.0]), ([0.0, 10.0], [0.0, 1.0])],
        ];
        let res = clip_triangles(&sq, &clip);
        let a: f32 = res.iter().map(|t| area(&[t[0].0, t[1].0, t[2].0]).abs()).sum();
        assert!((a - 75.0).abs() < 1e-2, "{a}");
    }
}
