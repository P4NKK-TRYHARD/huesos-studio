//! Datos del proyecto: huesos, slots, skins, adjuntos, restricciones, eventos y animaciones.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Adjuntos
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum AttachKind {
    Rect,
    Ellipse,
    Image,
    Mesh,
    Path,
    /// Polígono de colisión (no se dibuja en el juego).
    BoundingBox,
    /// Polígono que recorta los slots siguientes hasta `end_slot`.
    Clipping,
    /// Punto con rotación (para lanzar balas, partículas...).
    Point,
}

impl AttachKind {
    pub fn label(self) -> &'static str {
        match self {
            AttachKind::Rect => "Rectángulo",
            AttachKind::Ellipse => "Elipse",
            AttachKind::Image => "Imagen",
            AttachKind::Mesh => "Malla",
            AttachKind::Path => "Trayectoria",
            AttachKind::BoundingBox => "Caja de colisión",
            AttachKind::Clipping => "Recorte",
            AttachKind::Point => "Punto",
        }
    }
}

/// Qué transformaciones hereda un hueso de su padre (como en Spine).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Inherit {
    #[default]
    Normal,
    OnlyTranslation,
    NoRotationOrReflection,
    NoScale,
    NoScaleOrReflection,
}

impl Inherit {
    pub const ALL: [Inherit; 5] = [Inherit::Normal, Inherit::OnlyTranslation, Inherit::NoRotationOrReflection, Inherit::NoScale, Inherit::NoScaleOrReflection];
    pub fn label(self) -> &'static str {
        match self {
            Inherit::Normal => "Todo",
            Inherit::OnlyTranslation => "Solo posición",
            Inherit::NoRotationOrReflection => "Sin rotación",
            Inherit::NoScale => "Sin escala",
            Inherit::NoScaleOrReflection => "Sin escala ni reflejo",
        }
    }
    pub fn spine(self) -> &'static str {
        match self {
            Inherit::Normal => "normal",
            Inherit::OnlyTranslation => "onlyTranslation",
            Inherit::NoRotationOrReflection => "noRotationOrReflection",
            Inherit::NoScale => "noScale",
            Inherit::NoScaleOrReflection => "noScaleOrReflection",
        }
    }
    pub fn from_spine(s: &str) -> Inherit {
        Inherit::ALL.into_iter().find(|i| i.spine() == s).unwrap_or_default()
    }
}

/// Influencia de un hueso sobre un vértice: posición del vértice en el espacio
/// local de ese hueso (en la pose base) y su peso.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Weight {
    pub bone: u32,
    pub x: f32,
    pub y: f32,
    pub w: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct MeshData {
    /// Vértices en el espacio local del hueso del slot (pose base).
    pub verts: Vec<[f32; 2]>,
    pub uvs: Vec<[f32; 2]>,
    pub tris: Vec<[u32; 3]>,
    /// Los primeros `hull` vértices forman el contorno, en orden.
    pub hull: usize,
    /// Vacío = sin pesos (el vértice sigue solo al hueso del slot).
    #[serde(default)]
    pub weights: Vec<Vec<Weight>>,
}

impl MeshData {
    pub fn weighted(&self) -> bool {
        !self.weights.is_empty() && self.weights.len() == self.verts.len()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PathPt {
    pub p: [f32; 2],
    /// Asa de entrada, relativa a `p`.
    pub hin: [f32; 2],
    /// Asa de salida, relativa a `p`.
    pub hout: [f32; 2],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct PathData {
    /// Puntos en el espacio local del hueso del slot.
    pub pts: Vec<PathPt>,
    pub closed: bool,
}

impl PathData {
    /// Segmentos Bézier cúbicos en el espacio que devuelve `map`.
    pub fn segments(&self, map: impl Fn([f32; 2]) -> [f32; 2]) -> Vec<[[f32; 2]; 4]> {
        let n = self.pts.len();
        let mut out = Vec::new();
        if n < 2 {
            return out;
        }
        let count = if self.closed { n } else { n - 1 };
        for i in 0..count {
            let a = &self.pts[i];
            let b = &self.pts[(i + 1) % n];
            out.push([
                map(a.p),
                map([a.p[0] + a.hout[0], a.p[1] + a.hout[1]]),
                map([b.p[0] + b.hin[0], b.p[1] + b.hin[1]]),
                map(b.p),
            ]);
        }
        out
    }
}

pub fn bezier_pt(s: &[[f32; 2]; 4], t: f32) -> [f32; 2] {
    let u = 1.0 - t;
    let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
    [
        a * s[0][0] + b * s[1][0] + c * s[2][0] + d * s[3][0],
        a * s[0][1] + b * s[1][1] + c * s[2][1] + d * s[3][1],
    ]
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Attachment {
    pub kind: AttachKind,
    pub image: String,
    pub w: f32,
    pub h: f32,
    pub ox: f32,
    pub oy: f32,
    pub rot: f32,
    pub tint: [u8; 4],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mesh: Option<MeshData>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<PathData>,
    /// Recorte: último slot afectado.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_slot: Option<u32>,
}

impl Attachment {
    pub fn rect(w: f32, h: f32, ox: f32, oy: f32, tint: [u8; 4]) -> Self {
        Self { kind: AttachKind::Rect, image: String::new(), w, h, ox, oy, rot: 0.0, tint, mesh: None, path: None, end_slot: None }
    }
    pub fn ellipse(w: f32, h: f32, ox: f32, oy: f32, tint: [u8; 4]) -> Self {
        Self { kind: AttachKind::Ellipse, ..Self::rect(w, h, ox, oy, tint) }
    }
    pub fn path_line(len: f32) -> Self {
        let pts = vec![
            PathPt { p: [0.0, 0.0], hin: [-len / 6.0, 0.0], hout: [len / 3.0, len / 3.0] },
            PathPt { p: [len, 0.0], hin: [-len / 3.0, -len / 3.0], hout: [len / 6.0, 0.0] },
        ];
        Self {
            kind: AttachKind::Path,
            image: String::new(),
            w: len,
            h: 0.0,
            ox: 0.0,
            oy: 0.0,
            rot: 0.0,
            tint: [255, 140, 60, 255],
            mesh: None,
            path: Some(PathData { pts, closed: false }),
            end_slot: None,
        }
    }
    /// Polígono (caja de colisión o recorte) con vértices en el espacio del hueso.
    pub fn polygon(kind: AttachKind, verts: Vec<[f32; 2]>) -> Self {
        let n = verts.len();
        let mesh = MeshData { uvs: vec![[0.0, 0.0]; n], verts, tris: Vec::new(), hull: n, weights: Vec::new() };
        let tint = if kind == AttachKind::Clipping { [206, 58, 58, 255] } else { [96, 240, 96, 255] };
        Self { kind, image: String::new(), w: 0.0, h: 0.0, ox: 0.0, oy: 0.0, rot: 0.0, tint, mesh: Some(mesh), path: None, end_slot: None }
    }
    pub fn point(x: f32, y: f32, rot: f32) -> Self {
        Self { kind: AttachKind::Point, ox: x, oy: y, rot, tint: [240, 240, 60, 255], ..Self::rect(0.0, 0.0, 0.0, 0.0, [255; 4]) }
    }
    /// Transformación del rectángulo del adjunto dentro del hueso.
    pub fn local_aff(&self) -> Aff {
        Aff::from_local(&Local { x: self.ox, y: self.oy, rot: self.rot, sx: 1.0, sy: 1.0, shx: 0.0, shy: 0.0 })
    }
}

// ---------------------------------------------------------------------------
// Huesos, slots y skins
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bone {
    pub id: u32,
    pub name: String,
    pub parent: Option<u32>,
    pub x: f32,
    pub y: f32,
    pub rotation: f32,
    pub length: f32,
    pub sx: f32,
    pub sy: f32,
    pub color: [u8; 4],
    pub visible: bool,
    #[serde(default)]
    pub shx: f32,
    #[serde(default)]
    pub shy: f32,
    #[serde(default)]
    pub inherit: Inherit,
    /// Solo para abrir proyectos de la versión anterior.
    #[serde(default, skip_serializing)]
    pub attachment: Option<Attachment>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Slot {
    pub id: u32,
    pub name: String,
    pub bone: u32,
    pub color: [u8; 4],
    /// Nombre del adjunto visible en la pose base.
    pub attachment: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkinEntry {
    pub slot: u32,
    pub name: String,
    pub att: Attachment,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Skin {
    pub name: String,
    pub entries: Vec<SkinEntry>,
}

// ---------------------------------------------------------------------------
// Restricciones
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum SpacingMode {
    Length,
    Fixed,
    Percent,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum RotateMode {
    Tangent,
    Chain,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ConstraintKind {
    Ik {
        target: u32,
        mix: f32,
        bend_positive: bool,
        /// Distancia (px) en la que la cadena se va estirando suavemente.
        #[serde(default)]
        softness: f32,
        /// Estirar los huesos si el objetivo queda lejos.
        #[serde(default)]
        stretch: bool,
        /// Encoger el hueso (IK de 1 hueso) si el objetivo queda cerca.
        #[serde(default)]
        compress: bool,
        /// Escalar también en Y al estirar.
        #[serde(default)]
        uniform: bool,
    },
    Transform {
        target: u32,
        mix_rotate: f32,
        mix_translate: f32,
        mix_scale: f32,
        off_rot: f32,
        off_x: f32,
        off_y: f32,
        off_sx: f32,
        off_sy: f32,
        /// Mezclas independientes en Y (si faltan, se usa la de X).
        #[serde(default)]
        mix_y: Option<f32>,
        #[serde(default)]
        mix_scale_y: Option<f32>,
        #[serde(default)]
        mix_shear: f32,
        #[serde(default)]
        off_shy: f32,
    },
    Path {
        slot: u32,
        position: f32,
        spacing: f32,
        spacing_mode: SpacingMode,
        rotate_mode: RotateMode,
        off_rot: f32,
        mix_rotate: f32,
        mix_translate: f32,
    },
    Physics {
        inertia: f32,
        strength: f32,
        damping: f32,
        mass: f32,
        gravity: f32,
        wind: f32,
        mix: f32,
    },
}

impl ConstraintKind {
    pub fn label(&self) -> &'static str {
        match self {
            ConstraintKind::Ik { .. } => "IK",
            ConstraintKind::Transform { .. } => "Transformación",
            ConstraintKind::Path { .. } => "Trayectoria",
            ConstraintKind::Physics { .. } => "Física",
        }
    }
    pub fn icon(&self) -> &'static str {
        match self {
            ConstraintKind::Ik { .. } => "🎯",
            ConstraintKind::Transform { .. } => "🔁",
            ConstraintKind::Path { .. } => "〰",
            ConstraintKind::Physics { .. } => "🌀",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Constraint {
    pub name: String,
    pub enabled: bool,
    /// Huesos afectados (IK: 1 o 2, padre y luego hijo; Física: 1).
    pub bones: Vec<u32>,
    pub kind: ConstraintKind,
}

impl Constraint {
    /// Nombres de los valores animables, en el orden de `anim_values`.
    pub fn value_labels(&self) -> &'static [&'static str] {
        match self.kind {
            ConstraintKind::Ik { .. } => &["Mezcla", "Suavidad", "Estirar", "Comprimir"],
            ConstraintKind::Transform { .. } => &["Mezcla rotación", "Mezcla X", "Mezcla Y", "Mezcla escala X", "Mezcla escala Y", "Mezcla cizalla"],
            ConstraintKind::Path { .. } => &["Posición", "Espaciado", "Mezcla rotación", "Mezcla posición"],
            ConstraintKind::Physics { .. } => &["Mezcla"],
        }
    }
    /// Valores animables actuales y doblez (IK).
    pub fn anim_values(&self) -> (Vec<f32>, bool) {
        match &self.kind {
            ConstraintKind::Ik { mix, bend_positive, softness, stretch, compress, .. } => {
                (vec![*mix, *softness, if *stretch { 1.0 } else { 0.0 }, if *compress { 1.0 } else { 0.0 }], *bend_positive)
            }
            ConstraintKind::Transform { mix_rotate, mix_translate, mix_scale, mix_y, mix_scale_y, mix_shear, .. } => (
                vec![*mix_rotate, *mix_translate, mix_y.unwrap_or(*mix_translate), *mix_scale, mix_scale_y.unwrap_or(*mix_scale), *mix_shear],
                true,
            ),
            ConstraintKind::Path { position, spacing, mix_rotate, mix_translate, .. } => (vec![*position, *spacing, *mix_rotate, *mix_translate], true),
            ConstraintKind::Physics { mix, .. } => (vec![*mix], true),
        }
    }
    /// Aplica valores animados.
    pub fn set_anim_values(&mut self, v: &[f32], bend: bool) {
        let g = |i: usize, d: f32| v.get(i).copied().unwrap_or(d);
        match &mut self.kind {
            ConstraintKind::Ik { mix, bend_positive, softness, stretch, compress, .. } => {
                *mix = g(0, *mix);
                *softness = g(1, *softness);
                *stretch = g(2, if *stretch { 1.0 } else { 0.0 }) > 0.5;
                *compress = g(3, if *compress { 1.0 } else { 0.0 }) > 0.5;
                *bend_positive = bend;
            }
            ConstraintKind::Transform { mix_rotate, mix_translate, mix_scale, mix_y, mix_scale_y, mix_shear, .. } => {
                *mix_rotate = g(0, *mix_rotate);
                *mix_translate = g(1, *mix_translate);
                *mix_y = Some(g(2, *mix_translate));
                *mix_scale = g(3, *mix_scale);
                *mix_scale_y = Some(g(4, *mix_scale));
                *mix_shear = g(5, *mix_shear);
            }
            ConstraintKind::Path { position, spacing, mix_rotate, mix_translate, .. } => {
                *position = g(0, *position);
                *spacing = g(1, *spacing);
                *mix_rotate = g(2, *mix_rotate);
                *mix_translate = g(3, *mix_translate);
            }
            ConstraintKind::Physics { mix, .. } => *mix = g(0, *mix),
        }
    }
}

// ---------------------------------------------------------------------------
// Eventos
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct EventDef {
    pub name: String,
    pub int: i32,
    pub float: f32,
    pub string: String,
    pub audio: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EventKey {
    pub frame: u32,
    pub name: String,
    pub int: i32,
    pub float: f32,
    pub string: String,
}

// ---------------------------------------------------------------------------
// Curvas y líneas de tiempo
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Channel {
    Rotate,
    Translate,
    Scale,
    Shear,
}

impl Channel {
    pub const ALL: [Channel; 4] = [Channel::Rotate, Channel::Translate, Channel::Scale, Channel::Shear];
    pub fn label(self) -> &'static str {
        match self {
            Channel::Rotate => "Rotación",
            Channel::Translate => "Posición",
            Channel::Scale => "Escala",
            Channel::Shear => "Cizalla",
        }
    }
    pub fn get(self, l: &Local) -> [f32; 2] {
        match self {
            Channel::Rotate => [l.rot, 0.0],
            Channel::Translate => [l.x, l.y],
            Channel::Scale => [l.sx, l.sy],
            Channel::Shear => [l.shx, l.shy],
        }
    }
    pub fn set(self, l: &mut Local, v: [f32; 2]) {
        match self {
            Channel::Rotate => l.rot = v[0],
            Channel::Translate => {
                l.x = v[0];
                l.y = v[1];
            }
            Channel::Scale => {
                l.sx = v[0];
                l.sy = v[1];
            }
            Channel::Shear => {
                l.shx = v[0];
                l.shy = v[1];
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Curve {
    Linear,
    Stepped,
    Smooth,
    /// Puntos de control normalizados (x1, y1, x2, y2); x en 0..1.
    Bezier([f32; 4]),
    /// Curva importada de Spine con y en unidades del valor, relativas al inicio
    /// (permite curvas con el mismo valor al principio y al final).
    BezierAbs([f32; 4]),
}

impl Curve {
    pub fn label(self) -> &'static str {
        match self {
            Curve::Linear => "Lineal",
            Curve::Stepped => "Escalonada",
            Curve::Smooth => "Suave",
            Curve::Bezier(_) => "Bézier",
            Curve::BezierAbs(_) => "Bézier (Spine)",
        }
    }
    /// Puntos de control equivalentes, si la curva es expresable como Bézier.
    pub fn as_bezier(self) -> Option<[f32; 4]> {
        match self {
            Curve::Linear => Some([1.0 / 3.0, 1.0 / 3.0, 2.0 / 3.0, 2.0 / 3.0]),
            Curve::Smooth => Some([1.0 / 3.0, 0.0, 2.0 / 3.0, 1.0]),
            Curve::Bezier(c) => Some(c),
            Curve::BezierAbs(c) => Some([c[0], 0.0, c[2], 1.0]),
            Curve::Stepped => None,
        }
    }
    /// Progreso interpolado para un avance `t` (0..1).
    pub fn ease(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Curve::Linear => t,
            Curve::Stepped => 0.0,
            Curve::Smooth => bezier_ease([1.0 / 3.0, 0.0, 2.0 / 3.0, 1.0], t),
            Curve::Bezier(c) => bezier_ease(c, t),
            Curve::BezierAbs(c) => bezier_ease([c[0], 0.0, c[2], 1.0], t),
        }
    }
    /// Valor entre `va` y `vb` para un avance lineal `t` (0..1).
    pub fn value(self, t: f32, va: f32, vb: f32) -> f32 {
        match self {
            Curve::BezierAbs(c) => va + bezier_pts(c[0], c[1], c[2], c[3], 1.0, vb - va, t.clamp(0.0, 1.0)),
            Curve::Stepped => {
                if t >= 1.0 {
                    vb
                } else {
                    va
                }
            }
            c => va + (vb - va) * c.ease(t),
        }
    }
}

/// Curva Bézier (0,0)-(x1,y1)-(x2,y2)-(1,ye) evaluada como Spine (10 tramos rectos).
pub fn bezier_pts(x1: f32, y1: f32, x2: f32, y2: f32, xe: f32, ye: f32, x: f32) -> f32 {
    let pt = |t: f32| {
        let u = 1.0 - t;
        (3.0 * u * u * t * x1 + 3.0 * u * t * t * x2 + t * t * t * xe, 3.0 * u * u * t * y1 + 3.0 * u * t * t * y2 + t * t * t * ye)
    };
    let mut prev = (0.0f32, 0.0f32);
    for i in 1..=10 {
        let p = if i == 10 { (xe, ye) } else { pt(i as f32 / 10.0) };
        if p.0 >= x {
            let span = p.0 - prev.0;
            let f = if span.abs() < 1e-9 { 1.0 } else { (x - prev.0) / span };
            return prev.1 + (p.1 - prev.1) * f;
        }
        prev = p;
    }
    ye
}

/// Evalúa una curva Bézier (0,0)-(x1,y1)-(x2,y2)-(1,1) en la abscisa `x`.
/// Igual que los runtimes de Spine: la curva se aproxima con 10 tramos rectos,
/// así el editor muestra exactamente lo que se verá en el juego.
pub fn bezier_ease(c: [f32; 4], x: f32) -> f32 {
    bezier_pts(c[0], c[1], c[2], c[3], 1.0, 1.0, x)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Key {
    pub frame: u32,
    pub v: [f32; 2],
    pub curve: Curve,
    /// Curva propia para el segundo valor (Y), si es distinta.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub curve_y: Option<Curve>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub bone: u32,
    pub channel: Channel,
    pub keys: Vec<Key>,
}

/// Busca entre qué claves cae `frame` y devuelve (índice a, índice b, avance con curva).
pub fn segment<K>(keys: &[K], frame: f32, f: impl Fn(&K) -> u32, curve: impl Fn(&K) -> Curve) -> Option<(usize, usize, f32)> {
    let first = keys.first()?;
    if frame < f(first) as f32 - 1e-4 {
        return None; // antes de la primera clave: pose base (como Spine)
    }
    if frame <= f(first) as f32 {
        return Some((0, 0, 0.0));
    }
    for i in 0..keys.len().saturating_sub(1) {
        let (a, b) = (&keys[i], &keys[i + 1]);
        let (fa, fb) = (f(a) as f32, f(b) as f32);
        if frame >= fa && frame < fb {
            let span = (fb - fa).max(1.0);
            let t = curve(a).ease((frame - fa) / span);
            return Some((i, i + 1, t));
        }
    }
    let l = keys.len() - 1;
    Some((l, l, 0.0))
}

impl Track {
    pub fn sort(&mut self) {
        self.keys.sort_by_key(|k| k.frame);
    }
    pub fn sample(&self, frame: f32) -> Option<[f32; 2]> {
        let (i, j, _) = segment(&self.keys, frame, |k| k.frame, |_| Curve::Linear)?;
        let (a, b) = (&self.keys[i], &self.keys[j]);
        if i == j {
            return Some(a.v);
        }
        let span = (b.frame as f32 - a.frame as f32).max(1.0);
        let t = (frame - a.frame as f32) / span;
        let cy = a.curve_y.unwrap_or(a.curve);
        Some([a.curve.value(t, a.v[0], b.v[0]), cy.value(t, a.v[1], b.v[1])])
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttKey {
    pub slot: u32,
    pub frame: u32,
    pub name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ColorKey {
    pub slot: u32,
    pub frame: u32,
    pub color: [u8; 4],
    pub curve: Curve,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DeformKey {
    pub slot: u32,
    pub att: String,
    pub frame: u32,
    /// Desplazamiento de cada vértice en el espacio de la malla.
    pub offsets: Vec<[f32; 2]>,
    pub curve: Curve,
}

/// Clave de los parámetros animables de una restricción (mezclas, posición...).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConKey {
    pub con: String,
    pub frame: u32,
    pub v: Vec<f32>,
    #[serde(default = "yes")]
    pub bend: bool,
    pub curve: Curve,
}

fn yes() -> bool {
    true
}

/// Orden de dibujo completo (ids de slots) a partir de un fotograma.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DrawKey {
    pub frame: u32,
    pub order: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Animation {
    pub name: String,
    pub duration: u32,
    pub fps: f32,
    pub tracks: Vec<Track>,
    #[serde(default)]
    pub att_keys: Vec<AttKey>,
    #[serde(default)]
    pub color_keys: Vec<ColorKey>,
    #[serde(default)]
    pub deform_keys: Vec<DeformKey>,
    #[serde(default)]
    pub events: Vec<EventKey>,
    #[serde(default)]
    pub con_keys: Vec<ConKey>,
    #[serde(default)]
    pub draw_keys: Vec<DrawKey>,
}

impl Animation {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            duration: 30,
            fps: 30.0,
            tracks: Vec::new(),
            att_keys: Vec::new(),
            color_keys: Vec::new(),
            deform_keys: Vec::new(),
            events: Vec::new(),
            con_keys: Vec::new(),
            draw_keys: Vec::new(),
        }
    }

    pub fn set_con_key(&mut self, con: &str, frame: u32, v: Vec<f32>, bend: bool) {
        if let Some(k) = self.con_keys.iter_mut().find(|k| k.con == con && k.frame == frame) {
            k.v = v;
            k.bend = bend;
        } else {
            self.con_keys.push(ConKey { con: con.to_string(), frame, v, bend, curve: Curve::Linear });
            self.con_keys.sort_by(|a, b| (&a.con, a.frame).cmp(&(&b.con, b.frame)));
        }
        self.grow(frame);
    }

    /// Valores interpolados de una restricción (y su doblez), si tiene claves.
    pub fn con_at(&self, con: &str, frame: f32) -> Option<(Vec<f32>, bool)> {
        let keys: Vec<&ConKey> = self.con_keys.iter().filter(|k| k.con == con).collect();
        let (i, j, t) = segment(&keys, frame, |k| k.frame, |k| k.curve)?;
        let (a, b) = (&keys[i].v, &keys[j].v);
        let mut v: Vec<f32> = (0..a.len()).map(|n| a[n] + (b.get(n).copied().unwrap_or(a[n]) - a[n]) * t).collect();
        // los interruptores (estirar/comprimir del IK) no se interpolan
        for n in 2..v.len().min(4) {
            if self.con_keys.iter().any(|k| k.con == con && k.v.len() > 2) {
                v[n] = a[n];
            }
        }
        Some((v, keys[i].bend))
    }

    pub fn set_draw_key(&mut self, frame: u32, order: Vec<u32>) {
        if let Some(k) = self.draw_keys.iter_mut().find(|k| k.frame == frame) {
            k.order = order;
        } else {
            self.draw_keys.push(DrawKey { frame, order });
            self.draw_keys.sort_by_key(|k| k.frame);
        }
        self.grow(frame);
    }

    pub fn draw_at(&self, frame: f32) -> Option<&Vec<u32>> {
        self.draw_keys.iter().filter(|k| k.frame as f32 <= frame + 1e-4).last().map(|k| &k.order)
    }

    pub fn track(&self, bone: u32, ch: Channel) -> Option<&Track> {
        self.tracks.iter().find(|t| t.bone == bone && t.channel == ch)
    }

    pub fn set_key(&mut self, bone: u32, ch: Channel, frame: u32, v: [f32; 2]) {
        let idx = match self.tracks.iter().position(|t| t.bone == bone && t.channel == ch) {
            Some(i) => i,
            None => {
                self.tracks.push(Track { bone, channel: ch, keys: Vec::new() });
                self.tracks.len() - 1
            }
        };
        let tr = &mut self.tracks[idx];
        if let Some(k) = tr.keys.iter_mut().find(|k| k.frame == frame) {
            k.v = v;
        } else {
            tr.keys.push(Key { frame, v, curve: Curve::Linear, curve_y: None });
            tr.sort();
        }
        self.grow(frame);
    }

    pub fn remove_key(&mut self, bone: u32, ch: Channel, frame: u32) {
        if let Some(tr) = self.tracks.iter_mut().find(|t| t.bone == bone && t.channel == ch) {
            tr.keys.retain(|k| k.frame != frame);
        }
        self.tracks.retain(|t| !t.keys.is_empty());
    }

    pub fn has_key(&self, bone: u32, ch: Channel, frame: u32) -> bool {
        self.track(bone, ch).map(|t| t.keys.iter().any(|k| k.frame == frame)).unwrap_or(false)
    }

    pub fn grow(&mut self, frame: u32) {
        if frame > self.duration {
            self.duration = frame;
        }
    }

    pub fn set_att_key(&mut self, slot: u32, frame: u32, name: Option<String>) {
        if let Some(k) = self.att_keys.iter_mut().find(|k| k.slot == slot && k.frame == frame) {
            k.name = name;
        } else {
            self.att_keys.push(AttKey { slot, frame, name });
            self.att_keys.sort_by_key(|k| (k.slot, k.frame));
        }
        self.grow(frame);
    }

    pub fn set_color_key(&mut self, slot: u32, frame: u32, color: [u8; 4]) {
        if let Some(k) = self.color_keys.iter_mut().find(|k| k.slot == slot && k.frame == frame) {
            k.color = color;
        } else {
            self.color_keys.push(ColorKey { slot, frame, color, curve: Curve::Linear });
            self.color_keys.sort_by_key(|k| (k.slot, k.frame));
        }
        self.grow(frame);
    }

    pub fn set_deform_key(&mut self, slot: u32, att: &str, frame: u32, offsets: Vec<[f32; 2]>) {
        if let Some(k) = self.deform_keys.iter_mut().find(|k| k.slot == slot && k.att == att && k.frame == frame) {
            k.offsets = offsets;
        } else {
            self.deform_keys.push(DeformKey { slot, att: att.to_string(), frame, offsets, curve: Curve::Linear });
            self.deform_keys.sort_by(|a, b| (a.slot, &a.att, a.frame).cmp(&(b.slot, &b.att, b.frame)));
        }
        self.grow(frame);
    }

    pub fn att_at(&self, slot: u32, frame: f32) -> Option<Option<String>> {
        let mut out = None;
        for k in self.att_keys.iter().filter(|k| k.slot == slot) {
            if k.frame as f32 <= frame + 1e-4 {
                out = Some(k.name.clone());
            }
        }
        out
    }

    pub fn color_at(&self, slot: u32, frame: f32) -> Option<[u8; 4]> {
        let keys: Vec<&ColorKey> = self.color_keys.iter().filter(|k| k.slot == slot).collect();
        let (i, j, t) = segment(&keys, frame, |k| k.frame, |k| k.curve)?;
        let (a, b) = (keys[i].color, keys[j].color);
        let mut c = [0u8; 4];
        for n in 0..4 {
            c[n] = (a[n] as f32 + (b[n] as f32 - a[n] as f32) * t).round().clamp(0.0, 255.0) as u8;
        }
        Some(c)
    }

    pub fn deform_at(&self, slot: u32, att: &str, frame: f32) -> Option<Vec<[f32; 2]>> {
        let keys: Vec<&DeformKey> = self.deform_keys.iter().filter(|k| k.slot == slot && k.att == att).collect();
        let (i, j, t) = segment(&keys, frame, |k| k.frame, |k| k.curve)?;
        let (a, b) = (&keys[i].offsets, &keys[j].offsets);
        Some(
            (0..a.len().max(b.len()))
                .map(|n| {
                    let pa = a.get(n).copied().unwrap_or([0.0, 0.0]);
                    let pb = b.get(n).copied().unwrap_or([0.0, 0.0]);
                    [pa[0] + (pb[0] - pa[0]) * t, pa[1] + (pb[1] - pa[1]) * t]
                })
                .collect(),
        )
    }

    /// Todos los fotogramas con alguna clave (para saltar entre claves).
    pub fn all_frames(&self) -> Vec<u32> {
        let mut v: Vec<u32> = self.tracks.iter().flat_map(|t| t.keys.iter().map(|k| k.frame)).collect();
        v.extend(self.att_keys.iter().map(|k| k.frame));
        v.extend(self.color_keys.iter().map(|k| k.frame));
        v.extend(self.deform_keys.iter().map(|k| k.frame));
        v.extend(self.events.iter().map(|k| k.frame));
        v.extend(self.con_keys.iter().map(|k| k.frame));
        v.extend(self.draw_keys.iter().map(|k| k.frame));
        v.sort();
        v.dedup();
        v
    }
}

// ---------------------------------------------------------------------------
// Documento
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Doc {
    pub bones: Vec<Bone>,
    #[serde(default)]
    pub slots: Vec<Slot>,
    #[serde(default)]
    pub skins: Vec<Skin>,
    /// Ids de slots, de atrás hacia delante.
    pub draw_order: Vec<u32>,
    #[serde(default)]
    pub constraints: Vec<Constraint>,
    #[serde(default)]
    pub events: Vec<EventDef>,
    pub animations: Vec<Animation>,
    pub next_id: u32,
    /// 2 = formato con slots y skins.
    #[serde(default)]
    pub version: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Local {
    pub x: f32,
    pub y: f32,
    pub rot: f32,
    pub sx: f32,
    pub sy: f32,
    pub shx: f32,
    pub shy: f32,
}

/// Transformación afín 2D: (x, y) -> (a x + b y + tx, c x + d y + ty)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aff {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub tx: f32,
    pub ty: f32,
}

#[allow(dead_code)]
impl Aff {
    pub const IDENTITY: Aff = Aff { a: 1.0, b: 0.0, c: 0.0, d: 1.0, tx: 0.0, ty: 0.0 };

    pub fn from_local(l: &Local) -> Aff {
        let rx = (l.rot + l.shx).to_radians();
        let ry = (l.rot + 90.0 + l.shy).to_radians();
        Aff { a: rx.cos() * l.sx, b: ry.cos() * l.sy, c: rx.sin() * l.sx, d: ry.sin() * l.sy, tx: l.x, ty: l.y }
    }

    /// Transformación de mundo de un hueso hijo según su modo de herencia (igual que Spine).
    pub fn child(parent: &Aff, l: &Local, inherit: Inherit) -> Aff {
        let (pa, pb, pc, pd) = (parent.a, parent.b, parent.c, parent.d);
        let tx = pa * l.x + pb * l.y + parent.tx;
        let ty = pc * l.x + pd * l.y + parent.ty;
        let (a, b, c, d) = match inherit {
            Inherit::Normal => {
                let la = Aff::from_local(l);
                (pa * la.a + pb * la.c, pa * la.b + pb * la.d, pc * la.a + pd * la.c, pc * la.b + pd * la.d)
            }
            Inherit::OnlyTranslation => {
                let la = Aff::from_local(l);
                (la.a, la.b, la.c, la.d)
            }
            Inherit::NoRotationOrReflection => {
                let (mut pa, mut pb, mut pc, mut pd) = (pa, pb, pc, pd);
                let mut s = pa * pa + pc * pc;
                let prx;
                if s > 0.0001 {
                    s = (pa * pd - pb * pc).abs() / s;
                    pb = pc * s;
                    pd = pa * s;
                    prx = pc.atan2(pa).to_degrees();
                } else {
                    pa = 0.0;
                    pc = 0.0;
                    prx = 90.0 - pd.atan2(pb).to_degrees();
                }
                let rx = (l.rot + l.shx - prx).to_radians();
                let ry = (l.rot + l.shy - prx + 90.0).to_radians();
                let (la, lb, lc, ld) = (rx.cos() * l.sx, ry.cos() * l.sy, rx.sin() * l.sx, ry.sin() * l.sy);
                (pa * la - pb * lc, pa * lb - pb * ld, pc * la + pd * lc, pc * lb + pd * ld)
            }
            Inherit::NoScale | Inherit::NoScaleOrReflection => {
                let r = l.rot.to_radians();
                let (sn, cs) = r.sin_cos();
                let mut za = pa * cs + pb * sn;
                let mut zc = pc * cs + pd * sn;
                let mut s = (za * za + zc * zc).sqrt();
                if s > 0.00001 {
                    s = 1.0 / s;
                }
                za *= s;
                zc *= s;
                let mut s = (za * za + zc * zc).sqrt();
                if inherit == Inherit::NoScale && (pa * pd - pb * pc) < 0.0 {
                    s = -s;
                }
                let rr = std::f32::consts::FRAC_PI_2 + zc.atan2(za);
                let zb = rr.cos() * s;
                let zd = rr.sin() * s;
                let shx = l.shx.to_radians();
                let shy = (90.0 + l.shy).to_radians();
                let (la, lb, lc, ld) = (shx.cos() * l.sx, shy.cos() * l.sy, shx.sin() * l.sx, shy.sin() * l.sy);
                (za * la + zb * lc, za * lb + zb * ld, zc * la + zd * lc, zc * lb + zd * ld)
            }
        };
        Aff { a, b, c, d, tx, ty }
    }
    pub fn mul(&self, o: &Aff) -> Aff {
        Aff {
            a: self.a * o.a + self.b * o.c,
            b: self.a * o.b + self.b * o.d,
            c: self.c * o.a + self.d * o.c,
            d: self.c * o.b + self.d * o.d,
            tx: self.a * o.tx + self.b * o.ty + self.tx,
            ty: self.c * o.tx + self.d * o.ty + self.ty,
        }
    }
    pub fn apply(&self, x: f32, y: f32) -> [f32; 2] {
        [self.a * x + self.b * y + self.tx, self.c * x + self.d * y + self.ty]
    }
    pub fn apply_vec(&self, x: f32, y: f32) -> [f32; 2] {
        [self.a * x + self.b * y, self.c * x + self.d * y]
    }
    pub fn det(&self) -> f32 {
        self.a * self.d - self.b * self.c
    }
    pub fn inv_vec(&self, x: f32, y: f32) -> [f32; 2] {
        let det = self.det();
        if det.abs() < 1e-6 {
            return [x, y];
        }
        [(self.d * x - self.b * y) / det, (-self.c * x + self.a * y) / det]
    }
    pub fn inv_point(&self, x: f32, y: f32) -> [f32; 2] {
        self.inv_vec(x - self.tx, y - self.ty)
    }
    pub fn angle_deg(&self) -> f32 {
        self.c.atan2(self.a).to_degrees()
    }
    pub fn scale_x(&self) -> f32 {
        (self.a * self.a + self.c * self.c).sqrt()
    }
    pub fn scale_y(&self) -> f32 {
        (self.b * self.b + self.d * self.d).sqrt()
    }
    pub fn origin(&self) -> [f32; 2] {
        [self.tx, self.ty]
    }
    pub fn inverse(&self) -> Aff {
        let det = self.det();
        if det.abs() < 1e-9 {
            return Aff::IDENTITY;
        }
        let (a, b, c, d) = (self.d / det, -self.b / det, -self.c / det, self.a / det);
        Aff { a, b, c, d, tx: -(a * self.tx + b * self.ty), ty: -(c * self.tx + d * self.ty) }
    }
    pub fn linear(&self) -> Aff {
        Aff { tx: 0.0, ty: 0.0, ..*self }
    }
    /// Gira la parte lineal `deg` grados alrededor del origen del hueso.
    pub fn rotated(&self, deg: f32) -> Aff {
        let (s, c) = deg.to_radians().sin_cos();
        Aff {
            a: c * self.a - s * self.c,
            b: c * self.b - s * self.d,
            c: s * self.a + c * self.c,
            d: s * self.b + c * self.d,
            tx: self.tx,
            ty: self.ty,
        }
    }
}

impl Doc {
    pub fn empty() -> Self {
        let root = Bone {
            id: 0,
            name: "raíz".into(),
            parent: None,
            x: 0.0,
            y: 0.0,
            rotation: 0.0,
            length: 0.0,
            sx: 1.0,
            sy: 1.0,
            color: [200, 200, 200, 255],
            visible: true,
            shx: 0.0,
            shy: 0.0,
            inherit: Inherit::Normal,
            attachment: None,
        };
        Doc {
            bones: vec![root],
            slots: Vec::new(),
            skins: vec![Skin { name: "default".into(), entries: Vec::new() }],
            draw_order: Vec::new(),
            constraints: Vec::new(),
            events: Vec::new(),
            animations: vec![Animation::new("animación")],
            next_id: 1,
            version: 2,
        }
    }

    /// Convierte proyectos de la versión anterior (un adjunto por hueso).
    pub fn migrate(&mut self) {
        if self.skins.is_empty() {
            self.skins.push(Skin { name: "default".into(), entries: Vec::new() });
        }
        if self.version >= 2 {
            return;
        }
        let old_order = std::mem::take(&mut self.draw_order);
        let mut map: HashMap<u32, u32> = HashMap::new();
        let bones: Vec<(u32, String, Attachment)> =
            self.bones.iter_mut().filter_map(|b| b.attachment.take().map(|a| (b.id, b.name.clone(), a))).collect();
        for (bid, name, att) in bones {
            let sid = self.new_id();
            self.slots.push(Slot { id: sid, name: name.clone(), bone: bid, color: [255; 4], attachment: Some(name.clone()) });
            self.skins[0].entries.push(SkinEntry { slot: sid, name, att });
            map.insert(bid, sid);
        }
        for b in old_order {
            if let Some(s) = map.get(&b) {
                self.draw_order.push(*s);
            }
        }
        for s in &self.slots {
            if !self.draw_order.contains(&s.id) {
                self.draw_order.push(s.id);
            }
        }
        self.version = 2;
    }

    pub fn new_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn bone(&self, id: u32) -> Option<&Bone> {
        self.bones.iter().find(|b| b.id == id)
    }
    pub fn bone_mut(&mut self, id: u32) -> Option<&mut Bone> {
        self.bones.iter_mut().find(|b| b.id == id)
    }
    pub fn slot(&self, id: u32) -> Option<&Slot> {
        self.slots.iter().find(|s| s.id == id)
    }
    pub fn slot_mut(&mut self, id: u32) -> Option<&mut Slot> {
        self.slots.iter_mut().find(|s| s.id == id)
    }
    pub fn slots_of(&self, bone: u32) -> Vec<u32> {
        self.slots.iter().filter(|s| s.bone == bone).map(|s| s.id).collect()
    }
    pub fn children(&self, id: Option<u32>) -> Vec<u32> {
        self.bones.iter().filter(|b| b.parent == id).map(|b| b.id).collect()
    }
    pub fn is_descendant(&self, id: u32, ancestor: u32) -> bool {
        let mut cur = self.bone(id).and_then(|b| b.parent);
        let mut guard = 0;
        while let Some(p) = cur {
            if p == ancestor {
                return true;
            }
            guard += 1;
            if guard > 1024 {
                break;
            }
            cur = self.bone(p).and_then(|b| b.parent);
        }
        false
    }

    /// Huesos en orden de árbol (padres antes que hijos).
    pub fn bone_order(&self) -> Vec<u32> {
        let mut out = Vec::with_capacity(self.bones.len());
        let mut stack: Vec<u32> = self.children(None).into_iter().rev().collect();
        while let Some(id) = stack.pop() {
            if out.contains(&id) {
                continue;
            }
            out.push(id);
            for c in self.children(Some(id)).into_iter().rev() {
                stack.push(c);
            }
        }
        out
    }

    pub fn add_bone(&mut self, parent: Option<u32>, name: String, x: f32, y: f32, rot: f32, length: f32) -> u32 {
        let id = self.new_id();
        let color = self.bone(parent.unwrap_or(u32::MAX)).map(|b| b.color).unwrap_or([200, 200, 200, 255]);
        self.bones.push(Bone {
            id,
            name,
            parent,
            x,
            y,
            rotation: rot,
            length,
            sx: 1.0,
            sy: 1.0,
            color,
            visible: true,
            shx: 0.0,
            shy: 0.0,
            inherit: Inherit::Normal,
            attachment: None,
        });
        id
    }

    pub fn add_slot(&mut self, bone: u32, name: String) -> u32 {
        let id = self.new_id();
        self.slots.push(Slot { id, name, bone, color: [255; 4], attachment: None });
        self.draw_order.push(id);
        id
    }

    /// Añade (o reemplaza) un adjunto en una skin.
    pub fn put_attachment(&mut self, skin: usize, slot: u32, name: &str, att: Attachment) {
        let skin = skin.min(self.skins.len().saturating_sub(1));
        let Some(sk) = self.skins.get_mut(skin) else { return };
        if let Some(e) = sk.entries.iter_mut().find(|e| e.slot == slot && e.name == name) {
            e.att = att;
        } else {
            sk.entries.push(SkinEntry { slot, name: name.to_string(), att });
        }
    }

    /// Adjunto por nombre: primero en la skin activa, luego en la predeterminada.
    pub fn find_att(&self, skin: usize, slot: u32, name: &str) -> Option<(usize, &Attachment)> {
        for si in [skin, 0] {
            if let Some(e) = self.skins.get(si).and_then(|s| s.entries.iter().find(|e| e.slot == slot && e.name == name)) {
                return Some((si, &e.att));
            }
        }
        None
    }

    pub fn find_att_mut(&mut self, skin: usize, slot: u32, name: &str) -> Option<&mut Attachment> {
        let si = self.find_att(skin, slot, name)?.0;
        self.skins[si].entries.iter_mut().find(|e| e.slot == slot && e.name == name).map(|e| &mut e.att)
    }

    /// Nombres de adjuntos disponibles para un slot (todas las skins, sin repetir).
    pub fn att_names(&self, slot: u32) -> Vec<String> {
        let mut v: Vec<String> = Vec::new();
        for s in &self.skins {
            for e in s.entries.iter().filter(|e| e.slot == slot) {
                if !v.contains(&e.name) {
                    v.push(e.name.clone());
                }
            }
        }
        v
    }

    /// Borra un hueso y todos sus descendientes (con sus slots).
    pub fn delete_bone(&mut self, id: u32) {
        let mut doomed = vec![id];
        let mut i = 0;
        while i < doomed.len() {
            let cur = doomed[i];
            doomed.extend(self.children(Some(cur)));
            i += 1;
        }
        let slots: Vec<u32> = self.slots.iter().filter(|s| doomed.contains(&s.bone)).map(|s| s.id).collect();
        for s in slots {
            self.delete_slot(s);
        }
        self.bones.retain(|b| !doomed.contains(&b.id));
        for a in &mut self.animations {
            a.tracks.retain(|t| !doomed.contains(&t.bone));
        }
        for c in &mut self.constraints {
            c.bones.retain(|b| !doomed.contains(b));
        }
        let bones: Vec<u32> = self.bones.iter().map(|b| b.id).collect();
        let slots: Vec<u32> = self.slots.iter().map(|s| s.id).collect();
        self.constraints.retain(|c| {
            !c.bones.is_empty()
                && match &c.kind {
                    ConstraintKind::Ik { target, .. } | ConstraintKind::Transform { target, .. } => bones.contains(target),
                    ConstraintKind::Path { slot, .. } => slots.contains(slot),
                    ConstraintKind::Physics { .. } => true,
                }
        });
        // pesos de mallas que usaban esos huesos
        for sk in &mut self.skins {
            for e in &mut sk.entries {
                if let Some(m) = &mut e.att.mesh {
                    if m.weights.iter().any(|ws| ws.iter().any(|w| doomed.contains(&w.bone))) {
                        m.weights.clear();
                    }
                }
            }
        }
    }

    pub fn delete_slot(&mut self, id: u32) {
        self.slots.retain(|s| s.id != id);
        self.draw_order.retain(|d| *d != id);
        for sk in &mut self.skins {
            sk.entries.retain(|e| e.slot != id);
        }
        for a in &mut self.animations {
            a.att_keys.retain(|k| k.slot != id);
            a.color_keys.retain(|k| k.slot != id);
            a.deform_keys.retain(|k| k.slot != id);
            for k in &mut a.draw_keys {
                k.order.retain(|s| *s != id);
            }
        }
        for sk in &mut self.skins {
            for e in &mut sk.entries {
                if e.att.end_slot == Some(id) {
                    e.att.end_slot = None;
                }
            }
        }
        self.constraints.retain(|c| !matches!(c.kind, ConstraintKind::Path { slot, .. } if slot == id));
    }

    pub fn unique_name(&self, base: &str) -> String {
        unique(base, |s| self.bones.iter().any(|b| b.name == s))
    }
    pub fn unique_slot_name(&self, base: &str) -> String {
        unique(base, |s| self.slots.iter().any(|b| b.name == s))
    }
    pub fn unique_constraint_name(&self, base: &str) -> String {
        unique(base, |s| self.constraints.iter().any(|b| b.name == s))
    }

    pub fn setup_local(b: &Bone) -> Local {
        Local { x: b.x, y: b.y, rot: b.rotation, sx: b.sx, sy: b.sy, shx: b.shx, shy: b.shy }
    }

    pub fn local_pose(&self, b: &Bone, anim: Option<&Animation>, frame: f32) -> Local {
        let mut l = Self::setup_local(b);
        if let Some(a) = anim {
            for ch in Channel::ALL {
                if let Some(v) = a.track(b.id, ch).and_then(|t| t.sample(frame)) {
                    ch.set(&mut l, v);
                }
            }
        }
        l
    }

    /// Transformación de mundo de cada hueso (con restricciones, sin física).
    pub fn world(&self, anim: Option<&Animation>, frame: f32) -> HashMap<u32, Aff> {
        crate::pose::solve(self, anim, frame, 0, None).world
    }

    /// Transformación de mundo sin restricciones (para crear pesos y deshacer el padre).
    pub fn world_fk(&self, anim: Option<&Animation>, frame: f32) -> HashMap<u32, Aff> {
        let mut out = HashMap::new();
        for id in self.bone_order() {
            let Some(b) = self.bone(id) else { continue };
            let parent = b.parent.and_then(|p| out.get(&p).copied()).unwrap_or(Aff::IDENTITY);
            out.insert(id, Aff::child(&parent, &self.local_pose(b, anim, frame), b.inherit));
        }
        out
    }
}

pub fn unique(base: &str, taken: impl Fn(&str) -> bool) -> String {
    if !taken(base) {
        return base.to_string();
    }
    let mut n = 2;
    loop {
        let s = format!("{base}{n}");
        if !taken(&s) {
            return s;
        }
        n += 1;
    }
}
