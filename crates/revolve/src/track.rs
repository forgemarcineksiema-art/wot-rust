//! Track belt: a rounded loop wrapping the road wheels on one side, swept as a rectangular band.
//! This is the path-sweep generator — the last new shape kind a tank needs beyond plates, castings
//! and revolves. The loop arcs wrap tightly around the idler/sprocket ends; the discrete link cues
//! ([`t54_track_link_cues`]) ride the whole loop but are carried as `PartLod::Detail`, so the
//! readable links appear only at the close-up LOD0 while lower LODs keep the smooth band. Clean
//! factory build: a crisp new track run — fine tread and wear stay in the material/texture layer.

use game_core::TrackBeltVisual;
use glam::{Vec2, Vec3};
use sweep::{SweepCaps, SweepFrameMode, SweepPath, SweepSection, SweepSpec, try_sweep};
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
    let loop_pts = belt_loop(belt);
    let centroid =
        loop_pts.iter().fold(Vec2::ZERO, |sum, &point| sum + point) / loop_pts.len() as f32;
    // Each pad is a low, wide track shoe seated on the band's outer face and spaced with a thin gap,
    // so the run reads as a continuous segmented track rather than a sparse comb of proud teeth. A
    // low profile and dense spacing also keep the pads hugging the tight end arcs without splaying.
    let pad_rise = belt.half_thickness * 0.40;
    let mut links = Vec::with_capacity(n);
    for link in 0..n {
        let (midpoint, tangent_2d, link_pitch) =
            loop_sample(&loop_pts, (link as f32 + 0.5) / n as f32);
        let mut normal_2d = Vec2::new(-tangent_2d.y, tangent_2d.x);
        if normal_2d.dot(midpoint - centroid) < 0.0 {
            normal_2d = -normal_2d;
        }
        let tangent = Vec3::new(0.0, tangent_2d.y, tangent_2d.x);
        let normal = Vec3::new(0.0, normal_2d.y, normal_2d.x);
        // Seat the pad on the band's outer face: half of it beds into the band skin, half stands
        // proud as a shallow shoe — fused to the band, not floating above it.
        let center_2d = midpoint + normal_2d * belt.half_thickness;
        let center = Vec3::new(side_x, center_2d.y, center_2d.x);
        let half = Vec3::new(belt.half_width * 0.94, pad_rise, (link_pitch / n as f32) * 0.46);
        links.push(oriented_box_mesh(center, half, tangent, normal));

        // The guide runs on the wheel-facing surface of every shoe. It is deliberately a separate
        // volume from the tread pad: the close-up model must explain how the track stays centred
        // on its running gear instead of reading as a smooth rubber belt.
        let guide_center_2d = midpoint - normal_2d * belt.half_thickness * 0.55;
        let guide_center = Vec3::new(side_x, guide_center_2d.y, guide_center_2d.x);
        let guide_half = Vec3::new(
            belt.half_width * 0.20,
            belt.half_thickness * 0.72,
            (link_pitch / n as f32) * 0.28,
        );
        links.push(oriented_box_mesh(guide_center, guide_half, tangent, normal));
    }
    merge(&links)
}

fn loop_sample(loop_pts: &[Vec2], fraction: f32) -> (Vec2, Vec2, f32) {
    let lengths: Vec<f32> = (0..loop_pts.len())
        .map(|index| (loop_pts[(index + 1) % loop_pts.len()] - loop_pts[index]).length())
        .collect();
    let perimeter: f32 = lengths.iter().sum();
    let mut remaining = fraction.clamp(0.0, 1.0) * perimeter;
    for (index, length) in lengths.iter().copied().enumerate() {
        if remaining <= length {
            let start = loop_pts[index];
            let end = loop_pts[(index + 1) % loop_pts.len()];
            return (
                start.lerp(end, remaining / length.max(f32::EPSILON)),
                (end - start).normalize_or_zero(),
                perimeter,
            );
        }
        remaining -= length;
    }
    let start = loop_pts[0];
    let end = loop_pts[1];
    (start, (end - start).normalize_or_zero(), perimeter)
}

/// Both sides' track link rows, merged.
pub fn t54_track_link_cues(belt: &TrackBeltVisual) -> GeometryMesh {
    merge(&[t54_track_links(belt.side_x, belt), t54_track_links(-belt.side_x, belt)])
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

/// Sweep a `(2*half_w) x (2*half_t)` rectangular cross-section along the closed loop using the
/// general path-sweep kernel. The loop lies in the world Y-Z plane at `side_x`; with a fixed up of
/// world X, the section's `u` axis is the band width (X) and `v` is the in-plane band thickness, so
/// the result is the same rectangular band the bespoke sweep produced — now one reusable primitive.
fn sweep_band(loop_pts: &[Vec2], side_x: f32, half_t: f32, half_w: f32) -> GeometryMesh {
    // Lift the (z, y) side-profile loop to world space at this track lane.
    let points: Vec<Vec3> = loop_pts.iter().map(|p| Vec3::new(side_x, p.y, p.x)).collect();
    let path = SweepPath { points, closed: true };
    let section = SweepSection {
        points: vec![
            Vec2::new(-half_w, -half_t),
            Vec2::new(half_w, -half_t),
            Vec2::new(half_w, half_t),
            Vec2::new(-half_w, half_t),
        ],
        closed: true,
    };
    try_sweep(&SweepSpec {
        path: &path,
        section: &section,
        frame_mode: SweepFrameMode::FixedUp(Vec3::X),
        caps: SweepCaps::Open,
        material: MaterialRole::TrackMetal,
        smoothing: SmoothingGroup::hard_edges(),
    })
    .expect("the T-54 track loop is a valid closed sweep path")
}

pub(crate) fn oriented_box_mesh(
    center: Vec3,
    half: Vec3,
    tangent: Vec3,
    normal: Vec3,
) -> GeometryMesh {
    let side = Vec3::X;
    let axes = [side, tangent, normal];
    let half_values = [half.x, half.z, half.y];
    let mut vertices = Vec::with_capacity(24);
    let mut indices = Vec::with_capacity(36);
    for (axis_index, axis) in axes.iter().enumerate() {
        for sign in [1.0_f32, -1.0] {
            let face_normal = *axis * sign;
            let first = axes[(axis_index + 1) % 3];
            let second = axes[(axis_index + 2) % 3];
            let first_half = half_values[(axis_index + 1) % 3];
            let second_half = half_values[(axis_index + 2) % 3];
            let axis_half = half_values[axis_index];
            let base = vertices.len() as u32;
            for (first_sign, second_sign) in [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
                vertices.push(GeometryVertex::new(
                    center
                        + face_normal * axis_half
                        + first * (first_sign * first_half)
                        + second * (second_sign * second_half),
                    face_normal,
                    MaterialRole::TrackMetal,
                    SmoothingGroup::hard_edges(),
                ));
            }
            if first.cross(second).dot(face_normal) > 0.0 {
                indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
            } else {
                indices.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
            }
        }
    }
    GeometryMesh::new(vertices, indices)
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
        band.validate_quality(vehicle_geometry::OPEN_OR_CLOSED_MESH)
            .expect("the swept track belt stays finite and non-self-degenerate");
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

    #[test]
    fn link_cues_follow_the_full_track_loop_instead_of_only_the_ground_run() {
        let belt_visual = belt();
        let links = t54_track_link_cues(&belt_visual);
        let bounds = links.bounds().expect("link cues");

        assert!(bounds.min.y < belt_visual.axle_y - belt_visual.radius);
        assert!(bounds.max.y > belt_visual.axle_y + belt_visual.radius);
    }

    #[test]
    fn closeup_track_uses_dense_links_instead_of_a_sparse_comb() {
        assert!(belt().link_count >= 40);
    }

    #[test]
    fn closeup_track_has_independent_inner_guide_geometry() {
        let links = t54_track_link_cues(&belt());
        assert!(
            links.triangle_count() > 2_500,
            "track shoes need separate inner guides, got {} triangles",
            links.triangle_count()
        );
    }

    #[test]
    fn every_oriented_link_face_winds_toward_its_authored_normal() {
        let mesh = t54_track_links(belt().side_x, &belt());
        let vertices = mesh.vertices();
        for triangle in mesh.indices().chunks_exact(3) {
            let (a, b, c) = (
                vertices[triangle[0] as usize],
                vertices[triangle[1] as usize],
                vertices[triangle[2] as usize],
            );
            let face = (b.position - a.position).cross(c.position - a.position);
            assert!(
                face.dot(a.normal + b.normal + c.normal) > 0.0,
                "oriented link face must wind outward"
            );
        }
    }
}
