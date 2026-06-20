//! T-54 round parts built from the revolve generator: the main gun barrel, the moving cast mantlet,
//! and the road-wheel train. Every dimension is read from the vehicle blueprint's [`GunVisual`] /
//! [`RunningGearVisual`] — the single source — rather than held here, so the geometry cannot drift
//! from the blueprint.

use game_core::{GunVisual, RunningGearVisual};
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

/// One road wheel — a rubber tyre with a steel hub disc proud of it — revolved about the axle (X)
/// and placed at `(side_x, _, center_z)`, lifted so it rests on the ground plane.
pub fn road_wheel(center_z: f32, side_x: f32, gear: &RunningGearVisual) -> GeometryMesh {
    let half_width = gear.wheel_half_width;
    let radius = gear.wheel_radius;
    let tyre = [(-half_width, 0.0), (-half_width, radius), (half_width, radius), (half_width, 0.0)];
    let tyre =
        revolve(Vec3::X, &tyre, gear.wheel_segments, MaterialRole::Rubber, SmoothingGroup(5));

    let hub_half = half_width + gear.hub_overhang;
    let hub = [
        (-hub_half, 0.0),
        (-hub_half, gear.hub_radius),
        (hub_half, gear.hub_radius),
        (hub_half, 0.0),
    ];
    let hub = revolve(
        Vec3::X,
        &hub,
        gear.hub_segments,
        MaterialRole::TrackMetal,
        SmoothingGroup::hard_edges(),
    );

    translate(&merge(&[tyre, hub]), Vec3::new(side_x, radius, center_z))
}

/// The road-wheel train: `gear.wheel_count` wheels per side, both sides — repetition of [`road_wheel`].
pub fn t54_running_gear(gear: &RunningGearVisual) -> GeometryMesh {
    let mut wheels = Vec::new();
    for i in 0..gear.wheel_count {
        let z = gear.first_z + i as f32 * gear.spacing;
        wheels.push(road_wheel(z, gear.side_x, gear));
        wheels.push(road_wheel(z, -gear.side_x, gear));
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
        let one = road_wheel(0.0, g.side_x, &g).triangle_count();
        assert_eq!(t54_running_gear(&g).triangle_count(), one * 2 * g.wheel_count);
    }
}
