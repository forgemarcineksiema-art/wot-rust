//! Track belt: a rounded loop wrapping the road wheels on one side, swept as a rectangular band.
//! This is the path-sweep generator — the last new shape kind a tank needs beyond plates, castings
//! and revolves. Link tread is left to the material/texture layer; the geometry is a smooth band.

use glam::{Vec2, Vec3};
use vehicle_geometry::{GeometryMesh, GeometryVertex, MaterialRole, SmoothingGroup};

use crate::merge;

/// One side's track belt at world `side_x`, wrapping wheels centred on `z = +/-1.2`, axle `y = 0.42`.
pub fn track_belt(side_x: f32) -> GeometryMesh {
    sweep_band(&belt_loop(), side_x, 0.06, 0.22)
}

/// Both track belts (left and right), merged.
pub fn t54_tracks() -> GeometryMesh {
    merge(&[track_belt(1.5), track_belt(-1.5)])
}

/// The side-profile loop as `(z, y)` points: bottom run, rear wrap, top run, front wrap.
fn belt_loop() -> Vec<Vec2> {
    let (front, rear, axle, r) = (2.3_f32, -2.3_f32, 0.42_f32, 0.47_f32);
    let mut pts = Vec::new();
    let straight = 6;
    for i in 0..straight {
        let t = i as f32 / straight as f32;
        pts.push(Vec2::new(front + (rear - front) * t, axle - r));
    }
    push_arc(&mut pts, Vec2::new(rear, axle), r, -90.0, -270.0, 8);
    for i in 0..straight {
        let t = i as f32 / straight as f32;
        pts.push(Vec2::new(rear + (front - rear) * t, axle + r));
    }
    push_arc(&mut pts, Vec2::new(front, axle), r, 90.0, -90.0, 8);
    pts
}

fn push_arc(pts: &mut Vec<Vec2>, center: Vec2, r: f32, a0_deg: f32, a1_deg: f32, n: usize) {
    for i in 0..n {
        let a = (a0_deg + (a1_deg - a0_deg) * i as f32 / n as f32).to_radians();
        pts.push(center + Vec2::new(a.cos(), a.sin()) * r);
    }
}

/// Sweep a `(2*half_w) x (2*half_t)` rectangular cross-section along the closed loop. Each loop
/// point gets four corners (width in X, thickness along the outward loop normal); consecutive
/// cross-sections stitch into a closed tube.
fn sweep_band(loop_pts: &[Vec2], side_x: f32, half_t: f32, half_w: f32) -> GeometryMesh {
    let n = loop_pts.len();
    let centroid = loop_pts.iter().fold(Vec2::ZERO, |a, &p| a + p) / n as f32;
    let mut vertices = Vec::with_capacity(n * 4);
    for (i, &p) in loop_pts.iter().enumerate() {
        let tangent = (loop_pts[(i + 1) % n] - loop_pts[(i + n - 1) % n]).normalize_or_zero();
        let mut normal = Vec2::new(-tangent.y, tangent.x);
        if normal.dot(p - centroid) < 0.0 {
            normal = -normal;
        }
        let outer = p + normal * half_t;
        let inner = p - normal * half_t;
        for &(x, zy) in &[
            (side_x - half_w, outer),
            (side_x + half_w, outer),
            (side_x + half_w, inner),
            (side_x - half_w, inner),
        ] {
            let pos = Vec3::new(x, zy.y, zy.x);
            vertices.push(GeometryVertex::new(
                pos,
                Vec3::ZERO,
                MaterialRole::TrackMetal,
                SmoothingGroup::hard_edges(),
            ));
        }
    }
    let mut indices = Vec::new();
    for i in 0..n {
        let (a, b) = ((i * 4) as u32, (((i + 1) % n) * 4) as u32);
        for k in 0..4u32 {
            let k1 = (k + 1) % 4;
            indices.extend_from_slice(&[a + k, a + k1, b + k1, a + k, b + k1, b + k]);
        }
    }
    GeometryMesh::new(vertices, indices).weld_and_smooth()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_belt_wraps_a_closed_band_around_the_wheels() {
        let belt = track_belt(1.5);
        assert!(belt.triangle_count() > 0, "belt has geometry");
        let b = belt.bounds().expect("non-empty");
        // The band spans the wheel run in z and rises from the ground to above the axle in y.
        assert!(
            b.min.z < -1.5 && b.max.z > 1.5,
            "wraps front-to-rear: {:.2}..{:.2}",
            b.min.z,
            b.max.z
        );
        assert!(
            b.min.y < 0.0 && b.max.y > 0.8,
            "rises ground to top: {:.2}..{:.2}",
            b.min.y,
            b.max.y
        );
        assert!((b.max.x - 1.72).abs() < 0.05, "sits at the right track lane: {:.2}", b.max.x);
    }

    #[test]
    fn both_tracks_double_the_geometry() {
        assert_eq!(t54_tracks().triangle_count(), 2 * track_belt(1.5).triangle_count());
    }
}
