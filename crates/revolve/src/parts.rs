//! T-54 round parts built from the revolve generator: the road-wheel train, the track-end idler and
//! sprocket, and round fittings. Every dimension is read from the vehicle blueprint's
//! [`RunningGearVisual`] / [`TrackBeltVisual`] — the single source — rather than held here, so the
//! geometry cannot drift from the blueprint.
//!
//! Each road wheel is a dished rubber tyre carrying a proud steel hub; the belt ends read as a
//! *distinct* smooth front idler and a faceted (lower-segment) rear drive sprocket. Clean factory
//! build: crisp new running gear — no caked mud, rust or thrown-track damage (that finish belongs to
//! the material layer).

use game_core::RunningGearVisual;
use glam::Vec3;
use vehicle_geometry::{GeometryMesh, GeometryVertex, MaterialRole, SmoothingGroup};

use crate::{merge, revolve, translate};

/// One road wheel — a *dished* rubber tyre (full-radius tread, a stepped rim lip, then the outer
/// face recessed inboard to the hub seat) carrying a steel hub boss proud of the rim — revolved
/// about the axle (X) and placed at `(side_x, axle_y, center_z)`. The dish gives the visible disc
/// depth so it reads as a wheel, not a flat plate; `axle_y` is the shared belt/wheel axle height so
/// the wheel nests concentrically inside the track belt.
pub fn road_wheel(
    center_z: f32,
    side_x: f32,
    axle_y: f32,
    gear: &RunningGearVisual,
) -> GeometryMesh {
    let hw = gear.wheel_half_width;
    let r = gear.wheel_radius;
    let hub_r = gear.hub_radius;
    // Recess depth (in X) and the rim lip step (in radius); fractions of the wheel so the profile
    // never crosses itself for any reasonable wheel. The dish is shallow — a stamped-disc face, not
    // a deep cup, so the wheel reads as solid rather than a ring.
    let dish = hw * 0.35;
    let rim_lip = (r * 0.12).min(hw);
    let tyre = [
        (-hw, 0.0),               // inner face centre (cap)
        (-hw, r),                 // inner rim, full radius
        (hw, r),                  // outer rim edge, full radius
        (hw, r - rim_lip),        // rim lip: a thin annulus facing out
        (hw - dish, r - rim_lip), // step inboard (dish wall)
        (hw - dish, hub_r),       // dished web down to the hub seat
    ];
    let tyre =
        revolve(Vec3::X, &tyre, gear.wheel_segments, MaterialRole::Rubber, SmoothingGroup(5));

    // The steel hub boss seats in the dish and stands `hub_overhang` proud of the rim.
    let hub_back = hw - dish;
    let hub_front = hw + gear.hub_overhang;
    let hub = [(hub_back, 0.0), (hub_back, hub_r), (hub_front, hub_r), (hub_front, 0.0)];
    let hub = revolve(
        Vec3::X,
        &hub,
        gear.hub_segments,
        MaterialRole::TrackMetal,
        SmoothingGroup::hard_edges(),
    );

    let wheel = merge(&[tyre, hub]);
    let wheel = if side_x.is_sign_negative() { mirror_x(&wheel) } else { wheel };
    translate(&wheel, Vec3::new(side_x, axle_y, center_z))
}

/// Mirrors an axle-aligned part so its raised details face the exterior of the opposite track run.
/// Mirroring reverses triangle winding, so each triangle is rewound to preserve face culling.
pub(crate) fn mirror_x(mesh: &GeometryMesh) -> GeometryMesh {
    let vertices = mesh
        .vertices()
        .iter()
        .map(|vertex| GeometryVertex {
            position: Vec3::new(-vertex.position.x, vertex.position.y, vertex.position.z),
            normal: Vec3::new(-vertex.normal.x, vertex.normal.y, vertex.normal.z),
            ..*vertex
        })
        .collect();
    let indices = mesh
        .indices()
        .chunks_exact(3)
        .flat_map(|triangle| [triangle[0], triangle[2], triangle[1]])
        .collect();
    GeometryMesh::new(vertices, indices)
}

/// The z of each road-wheel station, front (highest z) to rear. The rearmost wheel sits at the inset
/// train rear; wheels then step forward at the even `spacing`, with the larger characteristic
/// `first_gap` before the front wheel. Both gaps are wider than the wheel diameter, so the wheels are
/// evenly placed and never overlap (the previous formula crammed the rear wheels and intersected).
pub fn road_wheel_stations(gear: &RunningGearVisual) -> Vec<f32> {
    let count = gear.wheel_count;
    let rear_z = gear.first_z + gear.end_inset;
    let mut zs = Vec::with_capacity(count);
    let mut z = rear_z;
    for index in 0..count {
        if index > 0 {
            z += if index == count - 1 { gear.first_gap } else { gear.spacing };
        }
        zs.push(z);
    }
    zs.reverse();
    zs
}

/// A short upright drum (cylinder about Y) centred at `center`: the basis of round fittings such as
/// the cupola hatch lid and the glacis headlight.
pub fn drum(
    center: Vec3,
    radius: f32,
    half_height: f32,
    segments: usize,
    material: MaterialRole,
    smoothing: SmoothingGroup,
) -> GeometryMesh {
    let profile =
        [(-half_height, 0.0), (-half_height, radius), (half_height, radius), (half_height, 0.0)];
    translate(&revolve(Vec3::Y, &profile, segments, material, smoothing), center)
}

/// The road-wheel train: `gear.wheel_count` wheels per side, both sides — repetition of [`road_wheel`].
pub fn t54_running_gear(gear: &RunningGearVisual, axle_y: f32) -> GeometryMesh {
    let mut wheels = Vec::new();
    for z in road_wheel_stations(gear) {
        wheels.push(road_wheel(z, gear.side_x, axle_y, gear));
        wheels.push(road_wheel(z, -gear.side_x, axle_y, gear));
    }
    merge(&wheels)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gear() -> RunningGearVisual {
        game_core::VehicleBlueprint::for_vehicle(game_core::VehicleKind::T54_1951)
            .unwrap()
            .hybrid()
            .unwrap()
            .running_gear
    }

    #[test]
    fn the_running_gear_has_a_wheel_for_each_side_and_station() {
        let g = gear();
        let one = road_wheel(0.0, g.side_x, 0.58, &g).triangle_count();
        assert_eq!(t54_running_gear(&g, 0.58).triangle_count(), one * 2 * g.wheel_count);
    }

    #[test]
    fn the_road_wheel_is_dished_with_a_proud_hub() {
        // The visible (outer, +X) face must not be a flat plate: the rubber web recesses inboard of
        // the rim edge, and the steel hub boss stands proud of that rim — the depth that stops a
        // wheel reading as a black disc.
        let g = gear();
        let wheel = road_wheel(0.0, g.side_x, 0.58, &g);
        let max_x = |m: MaterialRole| {
            wheel
                .vertices()
                .iter()
                .filter(|v| v.material == m)
                .map(|v| v.position.x)
                .fold(f32::NEG_INFINITY, f32::max)
        };
        // Outer rubber surface: distance between the rim edge and the recessed web.
        let outer_rubber: Vec<f32> = wheel
            .vertices()
            .iter()
            .filter(|v| v.material == MaterialRole::Rubber && v.position.x > g.side_x)
            .map(|v| v.position.x)
            .collect();
        let rim = outer_rubber.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let web = outer_rubber.iter().copied().fold(f32::INFINITY, f32::min);
        assert!(rim - web > 0.03, "outer face must be dished (rim {rim:.3} vs web {web:.3})");
        assert!(
            max_x(MaterialRole::TrackMetal) > rim,
            "steel hub must stand proud of the rubber rim"
        );
    }

    #[test]
    fn each_track_side_exposes_its_hub_on_the_outside_face() {
        let g = gear();
        let left = road_wheel(0.0, -g.side_x, 0.58, &g);
        let outer_hub = left
            .vertices()
            .iter()
            .filter(|vertex| vertex.material == MaterialRole::TrackMetal)
            .map(|vertex| vertex.position.x)
            .fold(f32::INFINITY, f32::min);

        assert!(outer_hub < -g.side_x - g.wheel_half_width);
    }

    #[test]
    fn the_front_wheel_gap_is_largest_and_the_road_wheels_never_overlap() {
        let g = gear();
        let zs = road_wheel_stations(&g);
        assert_eq!(zs.len(), g.wheel_count);
        // Front to rear, descending z; the rearmost wheel sits at the inset train rear.
        assert!(
            (zs[zs.len() - 1] - (g.first_z + g.end_inset)).abs() < 1.0e-4,
            "rear wheel at the inset train rear"
        );
        let gaps: Vec<f32> = zs.windows(2).map(|w| w[0] - w[1]).collect();
        let front_gap = gaps[0];
        assert!(
            gaps[1..].iter().all(|&g| front_gap > g + 0.1),
            "the T-54 front wheel gap {front_gap:.2} is clearly the largest: {gaps:?}"
        );
        // The defect this locks: every centre-to-centre gap must clear the wheel diameter, so no two
        // road wheels intersect.
        let diameter = 2.0 * g.wheel_radius;
        assert!(
            gaps.iter().all(|&gap| gap >= diameter - 1.0e-4),
            "road wheels must not overlap: gaps {gaps:?} vs diameter {diameter:.2}"
        );
    }

    #[test]
    fn five_road_wheels_clear_the_idler_and_sprocket() {
        let blueprint = game_core::VehicleBlueprint::for_vehicle(game_core::VehicleKind::T54_1951)
            .expect("T-54 blueprint");
        let visual = blueprint.hybrid().expect("hybrid visual");
        let gear = visual.running_gear;
        let belt = visual.track_belt;
        let stations = road_wheel_stations(&gear);
        let end_radius = belt.radius - belt.half_thickness;
        let clearance = 0.05;

        assert_eq!(stations.len(), 5, "the T-54 retains five road wheels");
        assert!(
            belt.front_z - stations[0] > gear.wheel_radius + end_radius + clearance,
            "front road wheel must read separately from the idler"
        );
        assert!(
            stations[4] - belt.rear_z > gear.wheel_radius + end_radius + clearance,
            "rear road wheel must read separately from the sprocket"
        );
    }

}
