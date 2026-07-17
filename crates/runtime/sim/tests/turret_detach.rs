//! Locks the ammo-rack detonation → turret pop-off truth (protocol v20): a detonation kill sets
//! `turret_detached`, it rides the snapshot's `detached_turrets` list, and — critically for
//! honesty — the wreck's turret then stops blocking shells, so the collision truth matches the
//! decapitated picture the client draws.

use std::f32::consts::PI;

use game_core::math::HullPose;
use game_core::{ArmorZone, ModuleSlot, TankId, TankSpec, TeamId};
use glam::Vec3;
use sim::{
    FixedTimestep, SegmentImpact, ShellTraceWorld, SimulationState, TankCommand, TraceTank,
    segment_impact,
};

fn run_until_shell_resolved(state: &mut SimulationState, shooter: TankId) {
    let step = FixedTimestep::from_hz(60);
    state.apply_commands(&[(shooter, TankCommand { fire: true, ..TankCommand::idle() })], step);
    for _ in 0..240 {
        if state.shells().is_empty() || !state.damage_events().is_empty() {
            return;
        }
        state.apply_commands(&[(shooter, TankCommand::idle())], step);
    }
    panic!("the shell never resolved");
}

/// An ammo-rack side shot on a nearly-dead tank that finishes both the rack and the hull.
/// The target is a vehicle WITHOUT a narrow-phase damage layout (only the T-54 carries one),
/// so the legacy deterministic zone-roll lands the killing hit on the rack.
fn detonation_kill() -> (SimulationState, TankId) {
    let mut state = SimulationState::new();
    let shooter = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::new(-55.0, 0.0, 0.0));
    let target = state.spawn_tank(TeamId(2), game_core::VehicleKind::T34_85.spec(), Vec3::ZERO);
    {
        let shooter = state.tank_mut(shooter).expect("shooter");
        shooter.yaw_rad = PI / 2.0;
        shooter.gun_pitch_rad = 0.002;
        shooter.spec.gun.shell.penetration_mm_at_100m = 240.0;
    }
    {
        let target = state.tank_mut(target).expect("target");
        // Nearly dead, ammo rack all but gone: this one penetration finishes both.
        target.hit_points = 40;
        target.modules.damage(ModuleSlot::AmmoRack, u32::MAX - 1);
    }
    run_until_shell_resolved(&mut state, shooter);
    (state, target)
}

#[test]
fn an_ammo_rack_detonation_kill_blows_the_turret_off() {
    let (state, target) = detonation_kill();
    let wreck = state.tank(target).expect("target");
    assert_eq!(wreck.hit_points, 0, "the detonation killed it");
    assert!(wreck.turret_detached, "an ammo-rack detonation kill detaches the turret");
    // The wire mapping (turret_detached -> Snapshot.detached_turrets) is locked in net's
    // protocol/filter tests; sim cannot depend on net (net depends on sim).
}

#[test]
fn a_non_detonation_kill_leaves_the_turret_attached() {
    // A frontal glacis kill: damages/kills through the hull front, never the ammo rack.
    let mut state = SimulationState::new();
    let shooter = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::ZERO);
    let target = state.spawn_tank(TeamId(2), TankSpec::t54_1951(), Vec3::new(0.0, 0.0, 55.0));
    {
        let shooter = state.tank_mut(shooter).expect("shooter");
        shooter.gun_pitch_rad = -0.010;
        shooter.aim_dispersion_mrad = 0.0;
        shooter.spec.gun.dispersion_mrad = 0.0;
        shooter.spec.gun.shell.penetration_mm_at_100m = 300.0;
    }
    state.tank_mut(target).expect("target").yaw_rad = PI;
    state.tank_mut(target).expect("target").hit_points = 30;
    run_until_shell_resolved(&mut state, shooter);

    let wreck = state.tank(target).expect("target");
    assert_eq!(wreck.hit_points, 0, "the frontal hit killed it");
    assert!(!wreck.turret_detached, "a non-ammo-rack kill keeps the turret on");
}

/// A turret-height shot that strikes the turret on an intact wreck must pass clean over a
/// decapitated one — the collision truth follows the picture.
#[test]
fn a_detached_turret_no_longer_blocks_a_turret_height_shot() {
    let spec = TankSpec::t54_1951();
    let dome_y = spec.mounts.gun_trunnion.translation.y;
    let from = Vec3::new(0.0, dome_y, 10.0);
    let to = Vec3::new(0.0, dome_y, -2.0);

    let attached = trace_one(&spec, false, from, to);
    let detached = trace_one(&spec, true, from, to);

    match attached {
        Some(SegmentImpact::Tank { zone, .. }) => {
            assert!(
                matches!(zone, ArmorZone::TurretFront | ArmorZone::Mantlet),
                "the intact tank takes the shot on the turret, got {zone:?}"
            );
        }
        other => panic!("the intact tank should be hit on the turret, got {other:?}"),
    }
    assert!(
        detached.is_none(),
        "the decapitated wreck's turret is gone, so the shot passes over: {detached:?}"
    );
}

fn trace_one(
    spec: &TankSpec,
    turret_detached: bool,
    from: Vec3,
    to: Vec3,
) -> Option<SegmentImpact> {
    let mut tank = TraceTank::from_spec(TankId(9), Vec3::ZERO, HullPose::level(0.0), 0.0, spec);
    tank.turret_detached = turret_detached;
    let tanks = [tank];
    let world = ShellTraceWorld {
        projectile_radius_m: 0.0,
        tanks: &tanks,
        blockers: &[],
        heightmap: None,
        cover: &[],
        water: None,
    };
    segment_impact(from, to, to - from, &world)
}
