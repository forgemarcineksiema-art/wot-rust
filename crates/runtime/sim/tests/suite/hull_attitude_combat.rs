//! The payoff locks for authoritative hull attitude: hull-down genuinely adds gun depression,
//! and terrain tilt genuinely angles armor.

use game_core::math::HullPose;
use game_core::{TankSpec, TeamId, VehicleKind};
use glam::Vec3;
use sim::{
    FixedTimestep, SegmentImpact, ShellTraceWorld, SimulationState, TankCommand, TraceTank,
    segment_impact,
};

fn fire_command() -> TankCommand {
    TankCommand { fire: true, ..TankCommand::drive(0.0, 0.0) }
}

/// Hull-down: a nose-down hull fires its fully depressed gun on a steeper world arc than the
/// hull-frame limit alone allows — the crest is a real gun-depression bonus, like WoT.
#[test]
fn a_nose_down_hull_fires_below_the_flat_ground_depression_limit() {
    let mut state = SimulationState::new();
    let shooter = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::ZERO);
    // The T-54's OWN depression stop (-5 deg since the arc became per-vehicle) — the point of
    // this test is that the crest ADDS to whatever the tank's own limit is.
    let gun_stop = TankSpec::t54_1951().gun_pitch_limits_rad().0;
    {
        let tank = state.tank_mut(shooter).expect("shooter");
        tank.hull_pitch_rad = -0.20; // nose over a crest
        tank.gun_pitch_rad = gun_stop; // full hull-frame depression
    }
    state.apply_commands(&[(shooter, fire_command())], FixedTimestep::from_hz(60));

    let shell = state.shells().first().expect("shell fired");
    let elevation = (shell.velocity_mps.y / shell.velocity_mps.length()).asin();
    let expected = gun_stop - 0.20;
    assert!(
        (elevation - expected).abs() < 0.05,
        "world arc must be hull pitch + gun depression (~{expected}), got {elevation}"
    );
    assert!(
        elevation < gun_stop - 0.1,
        "the arc must dip well below the flat-ground limit, got {elevation}"
    );
}

/// A nose-up (tilted-back) hull angles its glacis: the same level shot meets the plate at a
/// steeper impact angle, so terrain posture buys effective armor.
#[test]
fn hull_tilt_angles_the_glacis_against_a_level_shot() {
    let impact_angle = |pitch_rad: f32| {
        let tank = TraceTank::for_kind(
            game_core::TankId(2),
            Vec3::new(0.0, 0.0, 30.0),
            HullPose { yaw_rad: std::f32::consts::PI, pitch_rad, roll_rad: 0.0 },
            0.0,
            VehicleKind::T54_1951,
        );
        let world = ShellTraceWorld {
            projectile_radius_m: 0.0,
            tanks: std::slice::from_ref(&tank),
            blockers: &[],
            heightmap: None,
            cover: &[],
            water: terrain::WaterView::DRY,
        };
        // Aim at the GLACIS AS POSED, and pin the zone. The old aim was a fixed horizontal
        // line at y 1.0: level it crossed the glacis band, but nose-up lifts the bow's fold by
        // ~0.7 m, so the tilted shot slid onto the lower plate — which, now that that plate
        // plays its authored 55 degrees, answered 40.7 (55 minus the 14.3-degree tilt) and the
        // test read its own aim error as a glacis regression. Same idiom as
        // `hull_down_nose_up_steepens_the_glacis`: a point on the real blueprint glacis plane,
        // carried through the pose, fired through flat.
        let blueprint =
            game_core::VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("blueprint");
        let cy = blueprint.hull.hitbox_center_y;
        let up_the_plate = 0.29;
        let glacis_local = Vec3::new(
            0.0,
            (blueprint.hull.sponson_y - cy) + up_the_plate,
            blueprint.hull.half_len
                - up_the_plate * blueprint.armor.hull_front.0.to_radians().tan(),
        );
        // yaw PI turns the bow toward the shooter; the pose's basis carries the point.
        let pose = HullPose { yaw_rad: std::f32::consts::PI, pitch_rad, roll_rad: 0.0 };
        let world_point =
            Vec3::new(0.0, 0.0, 30.0) + pose.basis() * (Vec3::Y * cy) + pose.basis() * glacis_local;
        let from = Vec3::new(0.0, world_point.y, world_point.z - 20.0);
        let to = Vec3::new(0.0, world_point.y, world_point.z + 10.0);
        match segment_impact(from, to, Vec3::Z, &world) {
            Some(SegmentImpact::Tank { impact_angle_degrees, zone, .. }) => {
                assert_eq!(zone, game_core::ArmorZone::UpperGlacis, "aimed at the glacis");
                impact_angle_degrees
            }
            other => panic!("expected a tank hit, got {other:?}"),
        }
    };

    let level = impact_angle(0.0);
    let tilted = impact_angle(0.25);
    assert!(
        tilted > level + 10.0,
        "a 0.25 rad tilt must add ~14 deg of impact angle: level {level}, tilted {tilted}"
    );
}
