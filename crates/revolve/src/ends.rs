//! The track-end mechanisms: the front **idler** and the rear **drive sprocket**, sitting on the
//! belt's axle line at each end of the run. They are deliberately distinct — the idler is a smooth
//! road-wheel-like disc, the sprocket a smaller smooth core ringed by teeth so it reads as the
//! toothed drive wheel. Every dimension is derived from the blueprint's running gear and track belt.

use game_core::{RunningGearVisual, TrackBeltVisual};
use glam::Vec3;
use vehicle_geometry::{GeometryMesh, MaterialRole, SmoothingGroup};

use crate::parts::mirror_x;
use crate::track::oriented_box_mesh;
use crate::{merge, revolve, translate};

/// A wheel revolved about the axle (X) and centred at `center` (not lifted to the ground): the core
/// disc of an idler or sprocket.
fn end_wheel(
    center: Vec3,
    radius: f32,
    half_width: f32,
    segments: usize,
    smoothing: SmoothingGroup,
) -> GeometryMesh {
    let tyre = [(-half_width, 0.0), (-half_width, radius), (half_width, radius), (half_width, 0.0)];
    translate(&revolve(Vec3::X, &tyre, segments, MaterialRole::TrackMetal, smoothing), center)
}

/// A raised end-wheel hub placed on the exterior face of either track run.
fn end_hub(
    center: Vec3,
    side_x: f32,
    half_width: f32,
    radius: f32,
    overhang: f32,
    segments: usize,
) -> GeometryMesh {
    let local = revolve(
        Vec3::X,
        &[
            (half_width - 0.02, 0.0),
            (half_width - 0.02, radius),
            (half_width + overhang, radius),
            (half_width + overhang, 0.0),
        ],
        segments,
        MaterialRole::TrackMetal,
        SmoothingGroup::hard_edges(),
    );
    let local = if side_x.is_sign_negative() { mirror_x(&local) } else { local };
    translate(&local, center)
}

/// A ring of `count` radial teeth around the sprocket rim, between `inner_r` and `outer_r`, in the
/// wheel's Y-Z plane — the drive sprocket's engagement teeth.
fn sprocket_teeth(center: Vec3, half_width: f32, outer_r: f32, count: usize) -> GeometryMesh {
    let inner_r = outer_r * 0.74;
    let mid = 0.5 * (outer_r + inner_r);
    let radial_half = 0.5 * (outer_r - inner_r);
    let mut teeth = Vec::with_capacity(count);
    for tooth in 0..count {
        let angle = std::f32::consts::TAU * tooth as f32 / count as f32;
        let radial = Vec3::new(0.0, angle.cos(), angle.sin());
        let tangent = Vec3::new(0.0, -angle.sin(), angle.cos());
        let half = Vec3::new(half_width * 0.85, radial_half, 0.05);
        teeth.push(oriented_box_mesh(center + radial * mid, half, tangent, radial));
    }
    merge(&teeth)
}

/// The track-end wheels, both sides: a smooth front idler and a toothed rear drive sprocket, sitting
/// in the belt's end wraps. The idler reads as a clean disc; the sprocket as a toothed drive wheel.
pub fn t54_track_ends(gear: &RunningGearVisual, belt: &TrackBeltVisual) -> GeometryMesh {
    let r = belt.radius - belt.half_thickness;
    let mut parts = Vec::new();
    for side in [gear.side_x, -gear.side_x] {
        // Front idler: a smooth full-radius disc with a small hub.
        let idler = Vec3::new(side, belt.axle_y, belt.front_z);
        parts.push(end_wheel(
            idler,
            r,
            gear.wheel_half_width,
            gear.wheel_segments,
            SmoothingGroup(5),
        ));
        parts.push(end_hub(idler, side, gear.wheel_half_width, r * 0.26, 0.06, gear.hub_segments));

        // Rear drive sprocket: a smaller smooth core ringed by teeth, plus a hub.
        let sprocket = Vec3::new(side, belt.axle_y, belt.rear_z);
        parts.push(end_wheel(
            sprocket,
            r * 0.74,
            gear.wheel_half_width,
            gear.hub_segments,
            SmoothingGroup::hard_edges(),
        ));
        parts.push(sprocket_teeth(sprocket, gear.wheel_half_width, r, 10));
        parts.push(end_hub(
            sprocket,
            side,
            gear.wheel_half_width,
            r * 0.30,
            0.07,
            gear.hub_segments,
        ));
    }
    merge(&parts)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn visual() -> game_core::HybridVisual {
        *game_core::VehicleBlueprint::for_vehicle(game_core::VehicleKind::T54_1951)
            .expect("T-54 blueprint")
            .hybrid()
            .expect("hybrid visual")
    }

    #[test]
    fn track_end_mechanisms_expose_outer_hubs_on_both_sides() {
        let v = visual();
        let mesh = t54_track_ends(&v.running_gear, &v.track_belt);
        let max_x = mesh
            .vertices()
            .iter()
            .map(|vertex| vertex.position.x)
            .fold(f32::NEG_INFINITY, f32::max);
        let min_x =
            mesh.vertices().iter().map(|vertex| vertex.position.x).fold(f32::INFINITY, f32::min);
        let outer_face = v.running_gear.side_x + v.running_gear.wheel_half_width;
        assert!(max_x > outer_face + 0.03, "right outer hub is visible");
        assert!(min_x < -outer_face - 0.03, "left outer hub is visible");
    }

    #[test]
    fn the_drive_sprocket_carries_teeth_out_to_the_rim() {
        let v = visual();
        let mesh = t54_track_ends(&v.running_gear, &v.track_belt);
        let outer_r = v.track_belt.radius - v.track_belt.half_thickness;
        let (axle_y, rear_z) = (v.track_belt.axle_y, v.track_belt.rear_z);
        // Around the rear sprocket axle, metal reaching near the band-inner radius can only be teeth
        // (the smooth core stops at 0.74 of it). Count the distinct angular sectors lit up: a ring of
        // teeth fills many sectors, a bare disc would fill none.
        let mut sectors = std::collections::HashSet::new();
        for vertex in mesh.vertices() {
            let (y, z) = (vertex.position.y - axle_y, vertex.position.z - rear_z);
            let radius = (y * y + z * z).sqrt();
            if radius > 0.88 * outer_r && radius < outer_r + 0.06 {
                sectors.insert((y.atan2(z) / (std::f32::consts::TAU / 12.0)).round() as i32);
            }
        }
        assert!(
            sectors.len() >= 6,
            "drive sprocket teeth must ring the rim: {} sectors",
            sectors.len()
        );
    }
}
