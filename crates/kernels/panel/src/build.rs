//! Geometry construction for thin fabricated panels: convex-outline insetting and the top/chamfer/
//! wall/bottom rings that make a closed plate with an optional edge treatment.

use glam::{Vec2, Vec3};
use vehicle_geometry::{GeometryMesh, GeometryVertex, MaterialRole, SmoothingGroup, signed_area};

/// A panel's local frame: `origin` with in-plane axes `u`, `v` and outward `normal`.
pub(crate) struct Frame {
    pub origin: Vec3,
    pub u: Vec3,
    pub v: Vec3,
    pub normal: Vec3,
}

impl Frame {
    pub(crate) fn new(origin: Vec3, normal: Vec3) -> Self {
        let normal = normal.normalize();
        let seed = if normal.x.abs() < 0.9 { Vec3::X } else { Vec3::Y };
        let u = (seed - normal * seed.dot(normal)).normalize();
        let v = normal.cross(u);
        Self { origin, u, v, normal }
    }

    fn world(&self, s: Vec2, depth: f32) -> Vec3 {
        self.origin + self.u * s.x + self.v * s.y - self.normal * depth
    }
}

/// The four stacked rings that define a panel: the visible top face, an optional chamfer down to the
/// outer rim, the vertical wall, and the bottom face.
pub(crate) struct PanelRings {
    pub top: Vec<Vec2>,
    pub chamfer_outer: Option<(Vec<Vec2>, f32)>,
    pub wall_top_depth: f32,
    pub bottom: Vec<Vec2>,
    pub bottom_depth: f32,
}

/// Build the closed panel mesh from its rings within `frame`. Every triangle carries its own
/// geometric normal, so hard-edge welding keeps the faces crisp while a smooth group fuses the
/// coincident rims into a watertight manifold.
pub(crate) fn build_panel(
    frame: &Frame,
    rings: &PanelRings,
    material: MaterialRole,
    smoothing: SmoothingGroup,
) -> GeometryMesh {
    let mut v: Vec<GeometryVertex> = Vec::new();
    let mut idx: Vec<u32> = Vec::new();
    let mut tri = |a: Vec3, b: Vec3, c: Vec3| {
        let n = (b - a).cross(c - a).normalize_or_zero();
        let base = v.len() as u32;
        for p in [a, b, c] {
            v.push(GeometryVertex::new(p, n, material, smoothing));
        }
        idx.extend_from_slice(&[base, base + 1, base + 2]);
    };

    let world = |s: Vec2, depth: f32| frame.world(s, depth);

    // Top face — a fan over the (CCW) top ring, winding toward +normal.
    let top: Vec<Vec3> = rings.top.iter().map(|&s| world(s, 0.0)).collect();
    for k in 1..top.len() - 1 {
        tri(top[0], top[k], top[k + 1]);
    }

    // Optional chamfer ring from the inset top edge down to the outer rim.
    let wall_outline: &[Vec2] = if let Some((outer, drop)) = &rings.chamfer_outer {
        let a: Vec<Vec3> = rings.top.iter().map(|&s| world(s, 0.0)).collect();
        let b: Vec<Vec3> = outer.iter().map(|&s| world(s, *drop)).collect();
        quad_ring(&mut tri, &a, &b);
        outer
    } else {
        &rings.top
    };

    // Vertical wall down to the bottom.
    let wt: Vec<Vec3> = wall_outline.iter().map(|&s| world(s, rings.wall_top_depth)).collect();
    let wb: Vec<Vec3> = rings.bottom.iter().map(|&s| world(s, rings.bottom_depth)).collect();
    quad_ring(&mut tri, &wt, &wb);

    // Bottom face — fan with reversed winding, toward -normal.
    let bottom: Vec<Vec3> = rings.bottom.iter().map(|&s| world(s, rings.bottom_depth)).collect();
    for k in 1..bottom.len() - 1 {
        tri(bottom[0], bottom[k + 1], bottom[k]);
    }

    GeometryMesh::new(v, idx).weld_and_smooth()
}

/// Stitch two equal-length rings (upper `a`, lower `b`) into a band of outward quads.
fn quad_ring(tri: &mut impl FnMut(Vec3, Vec3, Vec3), a: &[Vec3], b: &[Vec3]) {
    let n = a.len();
    for k in 0..n {
        let k1 = (k + 1) % n;
        tri(a[k], b[k], b[k1]);
        tri(a[k], b[k1], a[k1]);
    }
}

/// Inset a convex CCW polygon inward by `w` via per-edge offset and intersection. Returns `None` if
/// the inset collapses (the width exceeds what the outline can give back).
pub(crate) fn inset_convex(poly: &[Vec2], w: f32) -> Option<Vec<Vec2>> {
    let n = poly.len();
    let centroid = poly.iter().fold(Vec2::ZERO, |a, &p| a + p) / n as f32;
    let mut lines: Vec<(Vec2, Vec2)> = Vec::with_capacity(n);
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        let dir = (b - a).normalize_or_zero();
        let mut nrm = Vec2::new(-dir.y, dir.x);
        if nrm.dot(centroid - a) < 0.0 {
            nrm = -nrm;
        }
        lines.push((a + nrm * w, dir));
    }
    let mut out = Vec::with_capacity(n);
    for j in 0..n {
        let (p0, d0) = lines[(j + n - 1) % n];
        let (p1, d1) = lines[j];
        out.push(line_intersect(p0, d0, p1, d1)?);
    }
    if signed_area(&out) <= 1.0e-6 {
        return None;
    }
    Some(out)
}

fn line_intersect(p0: Vec2, d0: Vec2, p1: Vec2, d1: Vec2) -> Option<Vec2> {
    let denom = d0.perp_dot(d1);
    if denom.abs() < 1.0e-9 {
        return None;
    }
    let t = (p1 - p0).perp_dot(d1) / denom;
    Some(p0 + d0 * t)
}
