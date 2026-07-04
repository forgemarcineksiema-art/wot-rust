use std::f32::consts::PI;

use game_core::{TankSpec, TeamId, VehicleKind};
use glam::Vec3;
use sim::{
    FixedTimestep, SHELL_MAX_AGE_SECONDS, ShellTraceWorld, SimulationState, TankCommand,
    TraceOutcome, TraceTank, trace_shell,
};

/// The whole point of sharing one shell-physics implementation: the function the client reticle
/// calls (`trace_shell`) must land on the exact tank face the authoritative server step resolves,
/// so a previewed hit is never one the server rejects.
#[test]
fn reticle_trace_resolves_the_same_tank_impact_as_the_authoritative_step() {
    let mut state = SimulationState::new();
    let shooter = state.spawn_tank(TeamId(1), TankSpec::t55a(), Vec3::ZERO);
    let target = state.spawn_tank(TeamId(2), TankSpec::t54_1951(), Vec3::new(0.0, 0.0, 55.0));
    state.tank_mut(target).expect("target").yaw_rad = PI;
    let step = FixedTimestep::from_hz(60);

    // Fire, then capture the live shell one step into its flight.
    state.apply_commands(&[(shooter, fire_command())], step);
    let shell = state.shells().first().copied().expect("shell in flight after firing");

    // Trace from the shell's current state with the same world the server sees.
    let target_tank = state.tank(target).expect("target");
    let trace_tank = TraceTank::for_kind(
        target,
        target_tank.position,
        target_tank.hull_pose(),
        target_tank.turret_yaw_rad,
        VehicleKind::T54_1951,
    );
    let world = ShellTraceWorld {
        tanks: std::slice::from_ref(&trace_tank),
        blockers: &[],
        heightmap: None,
        cover: &[],
    };
    let outcome = trace_shell(
        shell.position,
        shell.velocity_mps,
        shell.shell.drag_per_s(),
        step.dt_seconds(),
        SHELL_MAX_AGE_SECONDS,
        &world,
    );

    // Drive the authoritative sim to resolution and compare.
    for _ in 0..30 {
        state.apply_commands(&[], step);
        if !state.damage_events().is_empty() {
            break;
        }
    }
    let event = state.damage_events().last().expect("authoritative hit resolves");
    assert_eq!(event.target, target);

    match outcome {
        TraceOutcome::Tank { id, facing, impact_angle_degrees, hit_position, .. } => {
            assert_eq!(id, target, "preview must identify the same target");
            assert_eq!(facing, event.armor_facing, "preview and server must agree on the face hit");
            assert!(
                (hit_position - event.hit_position).length() < 1.0e-3,
                "preview impact {hit_position:?} vs server {:?}",
                event.hit_position
            );
            assert!((impact_angle_degrees - event.impact_angle_degrees).abs() < 1.0e-2);
        }
        other => panic!("reticle trace should report a tank hit, got {other:?}"),
    }
}

fn fire_command() -> TankCommand {
    TankCommand { fire: true, ..TankCommand::idle() }
}
