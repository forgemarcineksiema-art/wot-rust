//! Shared section helpers for the section-sweeping operators ([`super::MeshBuilder::extrude`] and
//! [`super::MeshBuilder::loft`]): the per-axis `(u, v)` → local mapping, convexity/winding checks,
//! and outward face orientation. Kept in one place so the two operators cannot drift apart.

use glam::{Vec2, Vec3};

use crate::Axis;

/// Map a section point `(u, v)` at sweep depth `d` into local space for the given axis.
/// See [`crate::ExtrudeSpec`] for the per-axis `(u, v)` meaning.
pub(super) fn section_to_world(axis: Axis, uv: Vec2, depth: f32) -> Vec3 {
    match axis {
        Axis::X => Vec3::new(depth, uv.y, uv.x),
        Axis::Y => Vec3::new(uv.x, depth, uv.y),
        Axis::Z => Vec3::new(uv.x, uv.y, depth),
    }
}

/// Caps are fanned from the section centroid and side quads assume one outward-facing wall per
/// edge, so the section must be convex — a reflex corner would silently produce self-overlapping
/// caps and inward walls. Catch that at bake time. Expects CCW winding (callers normalize), so
/// every turn must be a left turn; collinear runs are allowed.
pub(super) fn assert_convex(section: &[Vec2]) {
    let count = section.len();
    for index in 0..count {
        let prev = section[(index + count - 1) % count];
        let here = section[index];
        let next = section[(index + 1) % count];
        let turn = (here - prev).perp_dot(next - here);
        assert!(
            turn >= -1.0e-4,
            "swept section is concave at point {index} ({here:?}); the kernel only sweeps convex sections"
        );
    }
}

/// Centroid of a section polygon's points.
pub(super) fn centroid(section: &[Vec2]) -> Vec2 {
    section.iter().fold(Vec2::ZERO, |sum, p| sum + *p) / section.len() as f32
}

/// Reorder a quad's corners so its geometric normal points along `outward`.
pub(super) fn oriented_quad(mut points: [Vec3; 4], outward: Vec3) -> [Vec3; 4] {
    let normal = (points[1] - points[0]).cross(points[2] - points[0]);
    if normal.dot(outward) < 0.0 {
        points.reverse();
    }
    points
}

/// Reorder a triangle's corners so its geometric normal points along `outward`.
pub(super) fn oriented_tri(mut points: [Vec3; 3], outward: Vec3) -> [Vec3; 3] {
    let normal = (points[1] - points[0]).cross(points[2] - points[0]);
    if normal.dot(outward) < 0.0 {
        points.swap(1, 2);
    }
    points
}
