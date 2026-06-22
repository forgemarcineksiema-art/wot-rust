//! The loft operation: connect a sequence of convex cross-sections along an axis into a skinned
//! solid whose silhouette changes from station to station. See [`crate::LoftSpec`] for the shape
//! contract; this is the operator behind tapered hulls and swelling cast turrets.

use glam::Vec2;

use super::section::{assert_convex, centroid, oriented_quad, oriented_tri, section_to_world};
use super::{MeshBuilder, axis_vector};
use crate::LoftSpec;
use crate::polygon::signed_area;

impl MeshBuilder {
    /// Loft a skin through `spec.sections` along `spec.axis`. Each section is normalized to CCW and
    /// asserted convex; all must share a point count so consecutive rings connect 1:1.
    pub fn loft(mut self, origin: glam::Vec3, spec: LoftSpec) -> Self {
        assert!(spec.sections.len() >= 2, "loft needs at least two sections");
        let count = spec.sections[0].section.len();
        assert!(count >= 3, "loft sections need at least 3 points");

        // Normalize every ring to CCW, assert convex, and require a matching point count.
        let rings: Vec<(f32, Vec<Vec2>)> = spec
            .sections
            .iter()
            .map(|station| {
                assert_eq!(
                    station.section.len(),
                    count,
                    "loft sections must all share the same point count"
                );
                let mut points = station.section.clone();
                if signed_area(&points) < 0.0 {
                    points.reverse();
                }
                assert_convex(&points);
                (station.offset, points)
            })
            .collect();

        let axis = spec.axis;
        // Side walls: one quad per edge between each consecutive pair of rings.
        for pair in rings.windows(2) {
            let (d0, s0) = (pair[0].0, &pair[0].1);
            let (d1, s1) = (pair[1].0, &pair[1].1);
            let (c0, c1) = (centroid(s0), centroid(s1));
            for i in 0..count {
                let j = (i + 1) % count;
                let edge_dir = ((s0[i] + s0[j]) * 0.5 - c0) + ((s1[i] + s1[j]) * 0.5 - c1);
                let outward = section_to_world(axis, edge_dir, 0.0);
                let quad = oriented_quad(
                    [
                        origin + section_to_world(axis, s0[i], d0),
                        origin + section_to_world(axis, s0[j], d0),
                        origin + section_to_world(axis, s1[j], d1),
                        origin + section_to_world(axis, s1[i], d1),
                    ],
                    outward,
                );
                self.push_quad(quad, spec.material, spec.smoothing);
            }
        }

        if spec.cap_ends {
            let axis_dir = axis_vector(axis);
            let (d_first, s_first) = (rings[0].0, &rings[0].1);
            let last = rings.len() - 1;
            let (d_last, s_last) = (rings[last].0, &rings[last].1);
            self = self.cap_section(origin, d_first, s_first, -axis_dir, &spec);
            self = self.cap_section(origin, d_last, s_last, axis_dir, &spec);
        }
        self
    }

    /// Fan flat caps over one end ring, oriented along `outward`.
    fn cap_section(
        mut self,
        origin: glam::Vec3,
        depth: f32,
        section: &[Vec2],
        outward: glam::Vec3,
        spec: &LoftSpec,
    ) -> Self {
        let axis = spec.axis;
        let center = origin + section_to_world(axis, centroid(section), depth);
        for i in 0..section.len() {
            let j = (i + 1) % section.len();
            let tri = oriented_tri(
                [
                    center,
                    origin + section_to_world(axis, section[i], depth),
                    origin + section_to_world(axis, section[j], depth),
                ],
                outward,
            );
            self.push_tri(tri, spec.material, spec.smoothing);
        }
        self
    }
}
