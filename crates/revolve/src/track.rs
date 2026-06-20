//! Track belt: a rounded loop wrapping the road wheels on one side, swept as a rectangular band.
//! This is the path-sweep generator — the last new shape kind a tank needs beyond plates, castings
//! and revolves. Link tread is left to the material/texture layer; the geometry is a smooth band.

use game_core::TrackBeltVisual;
use glam::{Vec2, Vec3};
use vehicle_geometry::{GeometryMesh, GeometryVertex, MaterialRole, SmoothingGroup};

use crate::merge;

/// One side's track belt at world `side_x`, wrapping the wheel run described by the blueprint belt.
pub fn track_belt(side_x: f32, belt: &TrackBeltVisual) -> GeometryMesh {
    sweep_band(&belt_loop(belt), side_x, belt.half_thickness, belt.half_width)
}

/// Both track belts (left and right), merged.
pub fn t54_tracks(belt: &TrackBeltVisual) -> GeometryMesh {
    merge(&[track_belt(belt.side_x, belt), track_belt(-belt.side_x, belt)])
}

/// Track link cues: a row of small raised blocks along the ground (bottom) run at world `side_x`, so
/// the belt reads as discrete tracked links. Fine tread is left to the material layer.
pub fn t54_track_links(side_x: f32, belt: &TrackBeltVisual) -> GeometryMesh {
    let n = belt.link_count.max(1);
    let span = belt.front_z - belt.rear_z;
    let pitch = span / n as f32;
    let y = belt.axle_y - belt.radius - belt.half_thickness;
    let half = Vec3::new(belt.half_width * 0.92, belt.half_thickness * 0.6, pitch * 0.36);
    let mut links = Vec::with_capacity(n);
    for i in 0..n {
        let z = belt.rear_z + (i as f32 + 0.5) * pitch;
        links.push(box_mesh(Vec3::new(side_x, y, z), half));
    }
    merge(&links)
}

/// Both sides' track link rows, merged.
pub fn t54_track_link_cues(belt: &TrackBeltVisual) -> GeometryMesh {
    merge(&[t54_track_links(belt.side_x, belt), t54_track_links(-belt.side_x, belt)])
}

/// A hard-edged axis-aligned box as a `GeometryMesh` (six flat faces, 12 triangles).
fn box_mesh(center: Vec3, half: Vec3) -> GeometryMesh {
    let mut vertices = Vec::with_capacity(24);
    let mut indices = Vec::with_capacity(36);
    for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
        for sign in [1.0_f32, -1.0] {
            let normal = axis * sign;
            let u = Vec3::new(axis.y, axis.z, axis.x); // a perpendicular in-plane axis
            let w = normal.cross(u);
            let base = vertices.len() as u32;
            for (su, sw) in [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
                let pos = center
                    + normal * half.dot(axis.abs())
                    + u * (su * half.dot(u.abs()))
                    + w * (sw * half.dot(w.abs()));
                vertices.push(GeometryVertex::new(
                    pos,
                    normal,
                    MaterialRole::TrackMetal,
                    SmoothingGroup::hard_edges(),
                ));
            }
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }
    GeometryMesh::new(vertices, indices)
}

/// The side-profile loop as `(z, y)` points: bottom run, rear wrap, top run, front wrap.
fn belt_loop(belt: &TrackBeltVisual) -> Vec<Vec2> {
    let (front, rear, axle, r) = (belt.front_z, belt.rear_z, belt.axle_y, belt.radius);
    let mut pts = Vec::new();
    let straight = belt.straight_segments;
    for i in 0..straight {
        let t = i as f32 / straight as f32;
        pts.push(Vec2::new(front + (rear - front) * t, axle - r));
    }
    push_arc(&mut pts, Vec2::new(rear, axle), r, -90.0, -270.0, belt.arc_segments);
    for i in 0..straight {
        let t = i as f32 / straight as f32;
        pts.push(Vec2::new(rear + (front - rear) * t, axle + r));
    }
    push_arc(&mut pts, Vec2::new(front, axle), r, 90.0, -90.0, belt.arc_segments);
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

    fn belt() -> TrackBeltVisual {
        game_core::VehicleBlueprint::for_vehicle(game_core::VehicleKind::T54_1951)
            .unwrap()
            .hybrid()
            .unwrap()
            .track_belt
    }

    #[test]
    fn the_belt_wraps_a_closed_band_around_the_wheels() {
        let belt_visual = belt();
        let band = track_belt(1.5, &belt_visual);
        assert!(band.triangle_count() > 0, "belt has geometry");
        let b = band.bounds().expect("non-empty");
        // The band spans the wheel run in z and, with the grounded layout, rests its bottom run on
        // the ground (≈0) and wraps up over the wheel tops.
        assert!(
            b.min.z < -1.5 && b.max.z > 1.5,
            "wraps front-to-rear: {:.2}..{:.2}",
            b.min.z,
            b.max.z
        );
        assert!(
            b.min.y < 0.05 && b.max.y > 1.0,
            "grounds and wraps over the wheels: {:.2}..{:.2}",
            b.min.y,
            b.max.y
        );
        let expected_x = 1.5 + belt_visual.half_width;
        assert!(
            (b.max.x - expected_x).abs() < 0.05,
            "sits at the right track lane: {:.2} vs {expected_x:.2}",
            b.max.x
        );
    }

    #[test]
    fn both_tracks_double_the_geometry() {
        let belt = belt();
        assert_eq!(
            t54_tracks(&belt).triangle_count(),
            2 * track_belt(belt.side_x, &belt).triangle_count()
        );
    }
}
