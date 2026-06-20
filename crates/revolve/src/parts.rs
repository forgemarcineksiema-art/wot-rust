//! T-54 round parts built from the revolve generator: the main gun barrel, the moving cast mantlet,
//! and the road-wheel train. Every dimension is read from the vehicle blueprint's [`GunVisual`] /
//! [`RunningGearVisual`] — the single source — rather than held here, so the geometry cannot drift
//! from the blueprint.

use game_core::{GunVisual, RunningGearVisual, TrackBeltVisual};
use glam::Vec3;
use vehicle_geometry::{GeometryMesh, GeometryVertex, MaterialRole, SmoothingGroup};

use crate::{merge, revolve, translate};

/// The main gun barrel as a capped steel cylinder along +Z, with a slight muzzle swell. `length` is
/// the breech-to-muzzle span — driven by the installed gun module, not a post-bake scale.
pub fn gun_barrel(length: f32, gun: &GunVisual) -> GeometryMesh {
    let r = gun.barrel_radius;
    let breech = 1.0;
    let muzzle = breech + length.max(0.5);
    let profile = [
        (breech, 0.0),
        (breech, r),
        (muzzle - gun.muzzle_taper, r),
        (muzzle, gun.muzzle_radius),
        (muzzle, 0.0),
    ];
    revolve(Vec3::Z, &profile, gun.barrel_segments, MaterialRole::BarrelSteel, SmoothingGroup(4))
}

/// A barrel mounted between the authoritative trunnion and muzzle frames in vehicle-local space.
pub fn gun_barrel_between(trunnion: Vec3, muzzle: Vec3, gun: &GunVisual) -> GeometryMesh {
    let length = (muzzle.z - trunnion.z).max(0.5);
    let r = gun.barrel_radius;
    let profile = [
        (0.0, 0.0),
        (0.0, r),
        (length - gun.muzzle_taper, r),
        (length, gun.muzzle_radius),
        (length, 0.0),
    ];
    translate(
        &revolve(
            Vec3::Z,
            &profile,
            gun.barrel_segments,
            MaterialRole::BarrelSteel,
            SmoothingGroup(4),
        ),
        trunnion,
    )
}

/// The moving cast mantlet mask, seated at the trunnion: a profile revolved about Z then squashed to
/// a wide flat oval (so it reads as a mask, not a round ball) by the blueprint's mantlet scale.
pub fn moving_mantlet(trunnion: Vec3, gun: &GunVisual) -> GeometryMesh {
    let base = revolve(
        Vec3::Z,
        &gun.mantlet_profile,
        gun.mantlet_segments,
        MaterialRole::CastArmor,
        SmoothingGroup(2),
    );
    let s = gun.mantlet_scale;
    let vertices = base
        .vertices()
        .iter()
        .map(|v| GeometryVertex {
            position: trunnion
                + Vec3::new(v.position.x * s.x, v.position.y * s.y, v.position.z * s.z),
            normal: Vec3::new(v.normal.x / s.x, v.normal.y / s.y, v.normal.z / s.z)
                .normalize_or_zero(),
            ..*v
        })
        .collect();
    GeometryMesh::new(vertices, base.indices().to_vec()).weld_and_smooth()
}

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
    // never crosses itself for any reasonable wheel.
    let dish = hw * 0.5;
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

    translate(&merge(&[tyre, hub]), Vec3::new(side_x, axle_y, center_z))
}

/// The z of each road-wheel station, front (highest z) to rear, with the T-54's characteristic large
/// gap between the front two wheels and the rest spaced evenly — endpoints fixed at the train span.
pub fn road_wheel_stations(gear: &RunningGearVisual) -> Vec<f32> {
    let count = gear.wheel_count;
    let span = (count - 1) as f32 * gear.spacing;
    let front_z = gear.first_z + span;
    let mut zs = vec![front_z];
    if count >= 2 {
        zs.push(front_z - gear.first_gap);
        let rear_interval = (span - gear.first_gap) / (count - 2).max(1) as f32;
        for _ in 2..count {
            let next = zs.last().copied().unwrap() - rear_interval;
            zs.push(next);
        }
    }
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

/// A wheel revolved about the axle (X) and centred at `center` (not lifted to the ground): used for
/// the track-end idler and drive sprocket, which sit on the axle line at the ends of the belt run.
fn end_wheel(
    center: Vec3,
    radius: f32,
    half_width: f32,
    segments: usize,
    material: MaterialRole,
    smoothing: SmoothingGroup,
) -> GeometryMesh {
    let tyre = [(-half_width, 0.0), (-half_width, radius), (half_width, radius), (half_width, 0.0)];
    translate(&revolve(Vec3::X, &tyre, segments, material, smoothing), center)
}

/// The track-end wheels, distinct from the road wheels: a smooth front idler and a faceted rear drive
/// sprocket (low segment count, reading as toothed), both sides, sitting in the belt's end wraps.
pub fn t54_track_ends(gear: &RunningGearVisual, belt: &TrackBeltVisual) -> GeometryMesh {
    let r = belt.radius - belt.half_thickness;
    let mut parts = Vec::new();
    for side in [gear.side_x, -gear.side_x] {
        parts.push(end_wheel(
            Vec3::new(side, belt.axle_y, belt.front_z),
            r,
            gear.wheel_half_width,
            gear.wheel_segments,
            MaterialRole::TrackMetal,
            SmoothingGroup(5),
        ));
        parts.push(end_wheel(
            Vec3::new(side, belt.axle_y, belt.rear_z),
            r,
            gear.wheel_half_width + 0.02,
            8,
            MaterialRole::TrackMetal,
            SmoothingGroup::hard_edges(),
        ));
    }
    merge(&parts)
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

    fn gun() -> GunVisual {
        game_core::VehicleBlueprint::for_vehicle(game_core::VehicleKind::T54_1951)
            .unwrap()
            .hybrid()
            .unwrap()
            .gun
    }

    #[test]
    fn the_barrel_reaches_forward_past_the_turret() {
        let b = gun_barrel(3.6, &gun()).bounds().expect("non-empty");
        assert!(b.max.z > 4.5, "muzzle reaches forward: {:.2}", b.max.z);
        assert!(b.max.x < 0.12 && b.max.y < 0.12, "barrel stays slim");
    }

    #[test]
    fn mounted_barrel_uses_the_authoritative_frames() {
        let trunnion = Vec3::new(0.0, 1.70, 1.10);
        let muzzle = Vec3::new(0.0, 1.70, 5.20);
        let barrel = gun_barrel_between(trunnion, muzzle, &gun());
        let b = barrel.bounds().expect("non-empty");
        assert!(b.min.y < trunnion.y && b.max.y > trunnion.y, "barrel surrounds trunnion height");
        assert!((b.max.z - muzzle.z).abs() < 1.0e-4, "muzzle reaches its authoritative frame");
    }

    #[test]
    fn a_longer_gun_module_makes_a_longer_barrel() {
        let short = gun_barrel(3.0, &gun()).bounds().unwrap().max.z;
        let long = gun_barrel(4.5, &gun()).bounds().unwrap().max.z;
        assert!(long > short + 1.0, "barrel length tracks the module: {long:.2} vs {short:.2}");
    }

    #[test]
    fn the_moving_mantlet_is_a_wide_flat_oval() {
        let m = moving_mantlet(Vec3::new(0.0, 1.70, 1.10), &gun()).bounds().expect("non-empty");
        assert!((m.max.x - m.min.x) > (m.max.y - m.min.y), "mantlet reads wider than tall");
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
    fn the_front_wheel_gap_is_the_largest_and_the_train_keeps_its_span() {
        let g = gear();
        let zs = road_wheel_stations(&g);
        assert_eq!(zs.len(), g.wheel_count);
        // Front to rear, descending z; the train still spans first_z .. front.
        let span = (g.wheel_count - 1) as f32 * g.spacing;
        assert!((zs[0] - (g.first_z + span)).abs() < 1.0e-4, "front wheel at the train front");
        assert!((zs[zs.len() - 1] - g.first_z).abs() < 1.0e-4, "rear wheel at the train rear");
        let gaps: Vec<f32> = zs.windows(2).map(|w| w[0] - w[1]).collect();
        let front_gap = gaps[0];
        assert!(
            gaps[1..].iter().all(|&g| front_gap > g + 0.1),
            "the T-54 front wheel gap {front_gap:.2} is clearly the largest: {gaps:?}"
        );
    }
}
