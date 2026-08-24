//! Concave polygon extrusion backed by `earcut`.

use glam::{Vec2, Vec3};

use super::section::{oriented_quad, oriented_tri, section_to_world};
use super::{MeshBuilder, axis_vector};
use crate::ExtrudePolygonSpec;

impl MeshBuilder {
    pub fn extrude_polygon(self, center: Vec3, spec: ExtrudePolygonSpec) -> Self {
        self.append_extrude_polygon(center, spec)
    }

    fn append_extrude_polygon(mut self, center: Vec3, spec: ExtrudePolygonSpec) -> Self {
        assert!(spec.points.len() >= 3, "polygon extrusion needs at least 3 points");
        assert!(
            spec.hole_indices.iter().all(|&index| index > 2 && index < spec.points.len()),
            "hole starts must point inside the polygon point list"
        );
        assert!(
            spec.hole_indices.windows(2).all(|pair| pair[0] < pair[1]),
            "hole starts must be strictly increasing"
        );

        let mut triangulator = earcut::Earcut::<f32>::new();
        let points = spec.points.iter().map(|point| [point.x, point.y]);
        let mut triangles: Vec<usize> = Vec::new();
        triangulator.earcut(points, &spec.hole_indices, &mut triangles);
        assert!(!triangles.is_empty(), "polygon extrusion produced no cap triangles");

        let near = -spec.half_depth;
        let far = spec.half_depth;
        let axis_dir = axis_vector(spec.axis);

        for triangle in triangles.chunks_exact(3) {
            let [a, b, c] = [triangle[0], triangle[1], triangle[2]];
            let near_tri = oriented_tri(
                [
                    center + section_to_world(spec.axis, spec.points[a], near),
                    center + section_to_world(spec.axis, spec.points[b], near),
                    center + section_to_world(spec.axis, spec.points[c], near),
                ],
                -axis_dir,
            );
            self.push_tri(near_tri, spec.material, spec.smoothing);
            let far_tri = oriented_tri(
                [
                    center + section_to_world(spec.axis, spec.points[a], far),
                    center + section_to_world(spec.axis, spec.points[b], far),
                    center + section_to_world(spec.axis, spec.points[c], far),
                ],
                axis_dir,
            );
            self.push_tri(far_tri, spec.material, spec.smoothing);
        }

        for (ring_index, ring) in polygon_rings(&spec.points, &spec.hole_indices).iter().enumerate()
        {
            let ring_points = &spec.points[ring.clone()];
            // A wall faces its edge's true outward normal, chosen by the ring's winding — NOT
            // `edge_mid - centroid`. That heuristic only holds for a ring star-shaped about the mean
            // of its points; on any edge of a reflex (concave) notch — which is exactly what this
            // earcut-backed operator exists to build — the edge midpoint sits centroid-side of the
            // real normal, so the hint is perpendicular-or-worse and the wall is left facing inward.
            // The outer ring faces away from its own interior (the solid); a hole faces into its
            // interior (the void). Keyed off the signed area, so either input orientation walls out.
            let ccw = crate::polygon::signed_area(ring_points) >= 0.0;
            for local_index in ring.clone() {
                let next_index =
                    if local_index + 1 == ring.end { ring.start } else { local_index + 1 };
                let a = spec.points[local_index];
                let b = spec.points[next_index];
                let edge = b - a;
                // Right-hand edge normal points to a CCW ring's exterior; flip for a CW ring.
                let exterior =
                    if ccw { Vec2::new(edge.y, -edge.x) } else { Vec2::new(-edge.y, edge.x) };
                let outward_2d = if ring_index == 0 { exterior } else { -exterior };
                let outward = section_to_world(spec.axis, outward_2d, 0.0);
                let quad = oriented_quad(
                    [
                        center + section_to_world(spec.axis, a, near),
                        center + section_to_world(spec.axis, b, near),
                        center + section_to_world(spec.axis, b, far),
                        center + section_to_world(spec.axis, a, far),
                    ],
                    outward,
                );
                self.push_quad(quad, spec.material, spec.smoothing);
            }
        }

        self
    }
}

fn polygon_rings(points: &[Vec2], hole_indices: &[usize]) -> Vec<std::ops::Range<usize>> {
    let mut starts = Vec::with_capacity(hole_indices.len() + 2);
    starts.push(0);
    starts.extend_from_slice(hole_indices);
    starts.push(points.len());
    starts.windows(2).map(|pair| pair[0]..pair[1]).collect()
}
