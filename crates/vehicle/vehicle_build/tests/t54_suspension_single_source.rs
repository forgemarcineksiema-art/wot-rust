//! ONE ARM PER WHEEL.
//!
//! The T-54 used to grow its swing arms twice. The hybrid bake welded a static box into the hull
//! at each road-wheel station (`t54_chassis::t54_suspension_parts`), and the running gear ALSO
//! instanced an animated trailing arm at the same station. At rest they overlapped, so nobody
//! noticed; over rough ground the animated arm rotated with live suspension travel while its
//! baked twin stayed frozen at rest height, and the pair pulled apart in plain sight.
//!
//! That is the F5 defect from the vehicle-stack audit, and it is not only a visual bug: the baked
//! copy also charged the hull's triangle budget for geometry the player already had.
//!
//! The animated arm is the real mechanism — it pivots, it carries the axle stub into the hub, and
//! it is authored once for the whole fleet. So the hull carries no arms at all, and this test says
//! so from both directions: the bake has none, the gear has exactly one per wheel per side, and
//! the one it has actually moves.

use game_core::VehicleKind;
use vehicle_geometry::{GearPart, RunningGearKinematics, running_gear_placements};

#[test]
fn the_hull_bake_carries_no_swing_arm_of_its_own() {
    let description = vehicle_build::t54_description();
    let baked: Vec<_> = description
        .parts
        .iter()
        .filter(|part| part.key.name.contains("swing_arm"))
        .map(|part| part.key.name)
        .collect();
    assert!(
        baked.is_empty(),
        "the hull bake welds {} swing-arm part(s) into the T-54 ({baked:?}) while the running \
         gear instances an animated arm at the same stations — one wheel, two arms, and only one \
         of them moves",
        baked.len()
    );
}

#[test]
fn the_running_gear_instances_exactly_one_arm_per_wheel_per_side() {
    let kin = RunningGearKinematics::for_vehicle(VehicleKind::T54_1951).expect("running gear");
    let arms = running_gear_placements(&kin, 0.0, 0.0)
        .iter()
        .filter(|placement| placement.part == GearPart::SwingArm)
        .count();
    assert_eq!(
        arms,
        kin.wheel_zs.len() * 2,
        "five stations a side means ten trailing arms — no more (a doubled arm) and no fewer \
         (wheels floating on a bare axle line)"
    );
}

/// Why the baked copy could never have been right: the arm's whole job is to move.
#[test]
fn the_instanced_arm_follows_live_suspension_travel() {
    let kin = RunningGearKinematics::for_vehicle(VehicleKind::T54_1951).expect("running gear");
    let arm_y = |travel: &[f32]| {
        let dynamics = vehicle_geometry::GearDynamics {
            left_travel: travel,
            right_travel: travel,
            ..Default::default()
        };
        vehicle_geometry::running_gear_placements_dynamic(&kin, 0.0, 0.0, dynamics)
            .into_iter()
            .filter(|placement| placement.part == GearPart::SwingArm)
            .map(|placement| {
                // The authored axle tip of the arm, carried into hull space by its placement.
                placement.transform.transform_point3(glam::Vec3::new(0.0, -0.13, -0.26)).y
            })
            .fold(f32::NEG_INFINITY, f32::max)
    };

    let at_rest = arm_y(&[0.0; 5]);
    let compressed = arm_y(&[0.16; 5]);
    assert!(
        compressed > at_rest + 0.05,
        "the arm must swing with the wheel it carries: axle tip {at_rest:.3} m at rest vs \
         {compressed:.3} m under 0.16 m of travel"
    );
}
