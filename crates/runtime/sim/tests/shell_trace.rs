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
    let shooter = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::ZERO);
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
        projectile_radius_m: shell.shell.collision_radius_m(),
        tanks: std::slice::from_ref(&trace_tank),
        blockers: &[],
        heightmap: None,
        cover: &[],
        water: None,
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

#[test]
fn projectile_radius_catches_a_cover_edge_and_reports_the_real_surface() {
    use game_core::ImpactSurface;
    use sim::{SegmentImpact, segment_impact};
    use terrain::{StaticCoverKind, StaticCoverObject};

    let cover = [StaticCoverObject {
        id: "edge".into(),
        name: "edge".into(),
        kind: StaticCoverKind::FarmBuilding,
        center: [0.0, 1.0, 10.0],
        half_extents_m: [1.0, 1.0, 1.0],
    }];
    let from = Vec3::new(1.05, 1.0, 0.0);
    let to = Vec3::new(1.05, 1.0, 20.0);
    let ray_world = ShellTraceWorld {
        projectile_radius_m: 0.0,
        tanks: &[],
        blockers: &[],
        heightmap: None,
        cover: &cover,
        water: None,
    };
    assert!(segment_impact(from, to, to - from, &ray_world).is_none(), "center ray clears");

    let shell_world = ShellTraceWorld { projectile_radius_m: 0.06, ..ray_world };
    match segment_impact(from, to, to - from, &shell_world) {
        Some(SegmentImpact::Obstacle { position, surface }) => {
            assert_eq!(surface, ImpactSurface::Cover);
            assert!((position.x - 1.0).abs() < 1.0e-6, "contact is on the real box: {position:?}");
        }
        other => panic!("the 12 cm projectile must clip the edge, got {other:?}"),
    }
}

/// A shell arcing into a flooded basin dies at the SURFACE (`ImpactSurface::Water`), not on the
/// riverbed below — the splash is where the players see the shot end, and the reticle preview
/// runs this exact trace.
#[test]
fn a_shell_falling_into_water_splashes_at_the_surface_not_the_bed() {
    use game_core::ImpactSurface;
    use terrain::{HeightMap, WaterBody};

    let heightmap = HeightMap::flat(60, 60, 5.0, 0.0).expect("flat basin");
    let water = WaterBody { surface_level_m: 2.0 };
    let world = ShellTraceWorld {
        projectile_radius_m: 0.0,
        tanks: &[],
        blockers: &[],
        heightmap: Some(&heightmap),
        cover: &[],
        water: Some(water),
    };

    // Lobbed from above the surface, flying down-range and falling.
    let outcome = trace_shell(
        Vec3::new(30.0, 8.0, 10.0),
        Vec3::new(0.0, -20.0, 60.0),
        0.05,
        1.0 / 60.0,
        SHELL_MAX_AGE_SECONDS,
        &world,
    );

    match outcome {
        TraceOutcome::Obstacle { position, surface } => {
            assert_eq!(surface, ImpactSurface::Water, "the surface eats the shell");
            assert!(
                (position.y - water.surface_level_m).abs() < 0.35,
                "the splash sits on the surface plane, got y={}",
                position.y
            );
        }
        other => panic!("expected a water splash, got {other:?}"),
    }

    // The identical shot over the drained basin reaches the ground instead.
    let dry_world = ShellTraceWorld { water: None, ..world };
    let dry = trace_shell(
        Vec3::new(30.0, 8.0, 10.0),
        Vec3::new(0.0, -20.0, 60.0),
        0.05,
        1.0 / 60.0,
        SHELL_MAX_AGE_SECONDS,
        &dry_world,
    );
    match dry {
        TraceOutcome::Obstacle { surface, .. } => assert_eq!(surface, ImpactSurface::Terrain),
        TraceOutcome::Expired(position) => {
            assert!(position.y <= 0.1, "the dry shot must reach the ground, got {position:?}")
        }
        other => panic!("expected a ground impact on the dry basin, got {other:?}"),
    }
}
