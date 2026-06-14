//! The extrude/loft operation: sweep a convex 2D section along an axis into a capped prism.
//!
//! These methods extend [`MeshBuilder`] from a child module so the larger builder file stays
//! reviewable; they reach the parent's private `push_quad`/`push_tri` and `axis_vector` directly.

use glam::{Vec2, Vec3};

use super::section::{assert_convex, oriented_quad, oriented_tri, section_to_world, signed_area};
use super::{MeshBuilder, axis_vector};
use crate::ExtrudeSpec;

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
            let outward = section_to_world(axis, (a + b) * 0.5 - centroid, 0.0);
            let quad = oriented_quad(
                [
                    center + section_to_world(axis, a, near),
                    center + section_to_world(axis, b, near),
                    center + section_to_world(axis, b, far),
                    center + section_to_world(axis, a, far),
                ],
                outward,
            );
            self.push_quad(quad, spec.material, spec.smoothing);
        }

        // Caps: fan from the section centroid at each end.
        let axis_dir = axis_vector(axis);
        let near_center = center + section_to_world(axis, centroid, near);
        let far_center = center + section_to_world(axis, centroid, far);
        for i in 0..count {
            let j = (i + 1) % count;
            let near_tri = oriented_tri(
                [
                    near_center,
                    center + section_to_world(axis, section[i], near),
                    center + section_to_world(axis, section[j], near),
                ],
                -axis_dir,
            );
            self.push_tri(near_tri, spec.material, spec.smoothing);
            let far_tri = oriented_tri(
                [
                    far_center,
                    center + section_to_world(axis, section[i], far),
                    center + section_to_world(axis, section[j], far),
                ],
                axis_dir,
            );
            self.push_tri(far_tri, spec.material, spec.smoothing);
        }
        self
    }
}
