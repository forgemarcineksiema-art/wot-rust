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
    {
        let tank = state.tank_mut(shooter).expect("shooter");
        tank.hull_pitch_rad = -0.20; // nose over a crest
        tank.gun_pitch_rad = sim::MIN_GUN_PITCH_RAD; // full hull-frame depression
    }
    state.apply_commands(&[(shooter, fire_command())], FixedTimestep::from_hz(60));

    let shell = state.shells().first().expect("shell fired");
    let elevation = (shell.velocity_mps.y / shell.velocity_mps.length()).asin();
    let expected = sim::MIN_GUN_PITCH_RAD - 0.20;
    assert!(
        (elevation - expected).abs() < 0.05,
        "world arc must be hull pitch + gun depression (~{expected}), got {elevation}"
    );
    assert!(
        elevation < sim::MIN_GUN_PITCH_RAD - 0.1,
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
            water: None,
        };
        let from = Vec3::new(0.0, 1.0, 0.0);
        let to = Vec3::new(0.0, 1.0, 40.0);
        match segment_impact(from, to, Vec3::Z, &world) {
            Some(SegmentImpact::Tank { impact_angle_degrees, .. }) => impact_angle_degrees,
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
