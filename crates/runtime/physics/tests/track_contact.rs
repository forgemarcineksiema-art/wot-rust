//! Integration locks for the running-gear support envelope through the full drive step: a
//! trench narrower than the wheelbase is bridged (the hull crosses level), while the legacy
//! centre-probe hull falls into it.

use game_core::{ContactFootprint, TankSpec, VehicleKind};
use glam::Vec3;
use physics::{
    TankControlInput, TankControllerSettings, TankFootprint, TankKinematicState,
    TankWorldObstacles, step_tank_on_world_with_tanks,
};
use terrain::HeightMap;

const DT: f32 = 1.0 / 60.0;

/// 61x61 @ 1 m plateau at 1.0 with a 1.6 m trench across it at z = 30 (z is the driving axis).
fn trench_map() -> HeightMap {
    let mut samples = Vec::with_capacity(61 * 61);
    for z in 0..61 {
        for x in 0..61 {
            let _ = x;
            samples.push(if (29.2..30.8).contains(&(z as f32)) { -2.0 } else { 1.0 });
        }
    }
    HeightMap::new(61, 61, 1.0, samples).expect("test heightmap dimensions are fixed")
}

/// Crawl the T-54 across the trench and report the lowest hull height seen near it.
fn min_height_crossing(footprint: Option<&ContactFootprint>) -> f32 {
    let map = trench_map();
    let spec = TankSpec::t54_1951();
    let settings = TankControllerSettings::from_spec(&spec);
    let obstacles_footprint = TankFootprint::from_hitbox(spec.hitbox);
    let mut state = TankKinematicState {
        position: Vec3::new(30.0, 1.0, 22.0),
        ..TankKinematicState::default()
    };
    let mut min_y = f32::INFINITY;
    for _ in 0..(12.0 / DT) as usize {
        step_tank_on_world_with_tanks(
            &mut state,
            TankControlInput { throttle: 0.3, steer: 0.0, brake: 0.0 },
            &settings,
            Some(&map),
            TankWorldObstacles::new(&[], obstacles_footprint, &[]),
            footprint,
            DT,
        );
        if (26.0..34.0).contains(&state.position.z) {
            min_y = min_y.min(state.position.y);
        }
        if state.position.z > 36.0 {
            break;
        }
    }
    min_y
}

#[test]
fn the_running_gear_bridges_a_trench_the_centre_probe_falls_into() {
    let footprint = ContactFootprint::for_vehicle(VehicleKind::T54_1951);
    let bridged = min_height_crossing(Some(&footprint));
    let legacy = min_height_crossing(None);

    assert!(
        bridged > 0.85,
        "with the support envelope the hull must cross the trench level, got {bridged}"
    );
    assert!(
        legacy < 0.0,
        "the legacy centre probe must drop the hull into the trench, got {legacy}"
    );
}
