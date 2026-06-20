//! T-54 round parts built from the revolve generator: the main gun barrel and the road-wheel train.
//! Proportions are spike-rough (matched to the stub hull block), not the final blueprint.

use glam::Vec3;
use vehicle_geometry::{GeometryMesh, MaterialRole, SmoothingGroup};

use crate::{merge, revolve, translate};

const WHEEL_RADIUS: f32 = 0.42;

/// The main gun barrel as a capped steel cylinder along +Z, with a slight muzzle swell. `length` is
/// the breech-to-muzzle span — driven by the installed gun module, not a post-bake scale.
pub fn gun_barrel(length: f32) -> GeometryMesh {
    let r = 0.09;
    let breech = 1.0;
    let muzzle = breech + length.max(0.5);
    let profile = [(breech, 0.0), (breech, r), (muzzle - 0.2, r), (muzzle, 0.085), (muzzle, 0.0)];
    revolve(Vec3::Z, &profile, 20, MaterialRole::BarrelSteel, SmoothingGroup(4))
}

/// One road wheel — a rubber tyre with a steel hub disc proud of it — revolved about the axle (X)
/// and placed at `(side_x, _, center_z)`, lifted so it rests on the ground plane.
pub fn road_wheel(center_z: f32, side_x: f32) -> GeometryMesh {
    let half_width = 0.12;
    let tyre = [
        (-half_width, 0.0),
        (-half_width, WHEEL_RADIUS),
        (half_width, WHEEL_RADIUS),
        (half_width, 0.0),
    ];
    let tyre = revolve(Vec3::X, &tyre, 20, MaterialRole::Rubber, SmoothingGroup(5));

    let hub_half = half_width + 0.02;
    let hub = [(-hub_half, 0.0), (-hub_half, 0.24), (hub_half, 0.24), (hub_half, 0.0)];
    let hub = revolve(Vec3::X, &hub, 14, MaterialRole::TrackMetal, SmoothingGroup::hard_edges());

    translate(&merge(&[tyre, hub]), Vec3::new(side_x, WHEEL_RADIUS, center_z))
}

/// The road-wheel train: five wheels per side, both sides — repetition of [`road_wheel`].
pub fn t54_running_gear() -> GeometryMesh {
    let mut wheels = Vec::new();
    for i in 0..5 {
        let z = -2.1 + i as f32 * 1.05;
        wheels.push(road_wheel(z, 1.5));
        wheels.push(road_wheel(z, -1.5));
    }
    merge(&wheels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_barrel_reaches_forward_past_the_turret() {
        let b = gun_barrel(3.6).bounds().expect("non-empty");
        assert!(b.max.z > 4.5, "muzzle reaches forward: {:.2}", b.max.z);
        assert!(b.max.x < 0.12 && b.max.y < 0.12, "barrel stays slim");
    }

    #[test]
    fn a_longer_gun_module_makes_a_longer_barrel() {
        let short = gun_barrel(3.0).bounds().unwrap().max.z;
        let long = gun_barrel(4.5).bounds().unwrap().max.z;
        assert!(long > short + 1.0, "barrel length tracks the module: {long:.2} vs {short:.2}");
    }

    #[test]
    fn the_running_gear_has_ten_wheels_worth_of_geometry() {
        let one = road_wheel(0.0, 1.5).triangle_count();
        assert_eq!(t54_running_gear().triangle_count(), one * 10, "five wheels each side");
    }
}
