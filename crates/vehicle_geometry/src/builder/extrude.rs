//! The extrude/loft operation: sweep a convex 2D section along an axis into a capped prism.
//!
//! These methods extend [`MeshBuilder`] from a child module so the larger builder file stays
//! reviewable; they reach the parent's private `push_quad`/`push_tri` and `axis_vector` directly.

use glam::{Vec2, Vec3};

use super::{MeshBuilder, axis_vector};
use crate::{Axis, ExtrudeSpec, MaterialRole, SmoothingGroup};

impl MeshBuilder {
    /// Sweep a convex cross-section along an axis. See [`ExtrudeSpec`] for the section mapping.
    pub fn extrude(self, center: Vec3, spec: ExtrudeSpec) -> Self {
        self.append_extrude(center, spec)
    }

    fn append_extrude(mut self, center: Vec3, spec: ExtrudeSpec) -> Self {
        assert!(spec.section.len() >= 3, "extrude section needs at least 3 points");
        // Normalize winding to CCW so the geometry is deterministic regardless of how the recipe
        // listed its points; faces are then reoriented outward individually below.
        let mut section = spec.section.clone();
        if signed_area(&section) < 0.0 {
            section.reverse();
        }
        assert_convex(&section);
        let count = section.len();
        let centroid = section.iter().fold(Vec2::ZERO, |sum, p| sum + *p) / count as f32;
        let near = -spec.half_depth;
        let far = spec.half_depth;
        let axis = spec.axis;

        // Side walls: one quad per section edge, spanning the near and far caps.
        for i in 0..count {
            let j = (i + 1) % count;
            let (a, b) = (section[i], section[j]);
            let outward = section_to_world_dir(axis, (a + b) * 0.5 - centroid);
            self.push_quad_outward(
                [
                    center + section_to_world(axis, a, near),
                    center + section_to_world(axis, b, near),
                    center + section_to_world(axis, b, far),
                    center + section_to_world(axis, a, far),
                ],
                outward,
                spec.material,
                spec.smoothing,
            );
        }

        // Caps: fan from the section centroid at each end.
        let axis_dir = axis_vector(axis);
        let near_center = center + section_to_world(axis, centroid, near);
        let far_center = center + section_to_world(axis, centroid, far);
        for i in 0..count {
            let j = (i + 1) % count;
            self.push_tri_outward(
                [
                    near_center,
                    center + section_to_world(axis, section[i], near),
                    center + section_to_world(axis, section[j], near),
                ],
                -axis_dir,
                spec.material,
                spec.smoothing,
            );
            self.push_tri_outward(
                [
                    far_center,
                    center + section_to_world(axis, section[i], far),
                    center + section_to_world(axis, section[j], far),
                ],
                axis_dir,
                spec.material,
                spec.smoothing,
            );
        }
        self
    }

    /// Push a quad, flipping its winding if needed so the geometric normal points along `outward`.
    fn push_quad_outward(
        &mut self,
        mut points: [Vec3; 4],
        outward: Vec3,
        material: MaterialRole,
        smoothing: SmoothingGroup,
    ) {
        let normal = (points[1] - points[0]).cross(points[2] - points[0]);
        if normal.dot(outward) < 0.0 {
            points.reverse();
        }
        self.push_quad(points, material, smoothing);
    }

    /// Push a triangle, flipping its winding if needed so the geometric normal points along `outward`.
    fn push_tri_outward(
        &mut self,
        mut points: [Vec3; 3],
        outward: Vec3,
        material: MaterialRole,
        smoothing: SmoothingGroup,
    ) {
        let normal = (points[1] - points[0]).cross(points[2] - points[0]);
        if normal.dot(outward) < 0.0 {
            points.swap(1, 2);
        }
        self.push_tri(points, material, smoothing);
    }
}

/// Map a section point `(u, v)` at sweep depth `d` into local space for the given axis.
/// See [`ExtrudeSpec`] for the per-axis `(u, v)` meaning.
fn section_to_world(axis: Axis, uv: Vec2, depth: f32) -> Vec3 {
    match axis {
        Axis::X => Vec3::new(depth, uv.y, uv.x),
        Axis::Y => Vec3::new(uv.x, depth, uv.y),
        Axis::Z => Vec3::new(uv.x, uv.y, depth),
    }
}

/// Map an in-section direction into local space (zero component along the sweep axis).
fn section_to_world_dir(axis: Axis, uv: Vec2) -> Vec3 {
    section_to_world(axis, uv, 0.0)
}

/// Caps are fanned from the section centroid and side quads assume one outward-facing wall per
/// edge, so the section must be convex — a reflex corner would silently produce self-overlapping
/// caps and inward walls. Catch that at bake time. Expects CCW winding (the caller normalizes),
/// so every turn must be a left turn; collinear runs are allowed.
fn assert_convex(section: &[Vec2]) {
    let count = section.len();
    for index in 0..count {
        let prev = section[(index + count - 1) % count];
        let here = section[index];
        let next = section[(index + 1) % count];
        let turn = (here - prev).perp_dot(next - here);
        assert!(
            turn >= -1.0e-4,
            "extrude section is concave at point {index} ({here:?}); the kernel only sweeps convex sections"
        );
    }
}

/// Twice the signed area of a polygon (shoelace); positive when wound counter-clockwise.
fn signed_area(section: &[Vec2]) -> f32 {
    let mut area = 0.0;
    for i in 0..section.len() {
        let a = section[i];
        let b = section[(i + 1) % section.len()];
        area += a.x * b.y - b.x * a.y;
    }
    area
}
