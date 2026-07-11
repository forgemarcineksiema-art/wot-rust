//! Rotation is collision-resolved like translation: a pivot that would grind the hull's
//! corners into a wall or a neighbour is refused (yaw reverts, the rotation rate dies) —
//! the old model resolved only position, so a stationary pivot could rotate the footprint
//! straight into an obstacle and rely on the interpenetration-escape crutch to crawl out.

use glam::Vec3;
use physics::{
    TankControlInput, TankControllerSettings, TankFootprint, TankKinematicState,
    TankWorldObstacles, step_tank_on_world_with_tanks,
};
use terrain::{HeightMap, StaticCoverKind, StaticCoverObject};

fn cover_box(center: [f32; 3], half: [f32; 3]) -> StaticCoverObject {
    StaticCoverObject {
        id: "wall".into(),
        name: "wall".into(),
        kind: StaticCoverKind::FarmBuilding,
        center,
        half_extents_m: half,
    }
}

fn pivot_for(seconds: f32, state: &mut TankKinematicState, obstacles: TankWorldObstacles<'_>) {
    let heightmap = HeightMap::flat(64, 64, 4.0, 0.0).expect("flat terrain");
    let settings = TankControllerSettings::arcade_default();
    let ticks = (seconds * 60.0) as u32;
    for _ in 0..ticks {
        step_tank_on_world_with_tanks(
            state,
            TankControlInput { throttle: 0.0, steer: 1.0, brake: 0.0 },
            &settings,
            Some(&heightmap),
            obstacles,
            None,
            1.0 / 60.0,
        );
    }
}

#[test]
fn a_pivot_beside_a_wall_stops_instead_of_grinding_into_it() {
    // A long hull parked parallel to a wall, closer than its half length: rotating in place
    // MUST refuse once the swinging nose would enter the wall.
    let footprint = TankFootprint { half_width_m: 1.75, half_length_m: 3.20 };
    let wall = cover_box([44.0, 1.5, 40.0], [1.0, 2.5, 12.0]);
    let mut state =
        TankKinematicState { position: Vec3::new(40.6, 0.0, 40.0), ..Default::default() };
    // Parallel to the wall (facing +Z): flank gap ~0.65 m, far less than the 3.20 m nose swing.
    pivot_for(
        3.0,
        &mut state,
        TankWorldObstacles::new(std::slice::from_ref(&wall), footprint, &[]),
    );
    // The yaw may creep only as far as the wall allows — the nose corner must never cross the
    // wall face. Compute the worst corner reach toward the wall.
    let (sin, cos) = state.yaw_rad.sin_cos();
    let corner_x = state.position.x
        + (footprint.half_length_m * sin).abs()
        + (footprint.half_width_m * cos).abs();
    assert!(
        corner_x <= 43.0 + 1.0e-2,
        "the swinging corner must stop at the wall face: reach {corner_x:.3}, yaw {:.3}",
        state.yaw_rad
    );
}

#[test]
fn a_pivot_in_the_open_stays_free() {
    let footprint = TankFootprint { half_width_m: 1.75, half_length_m: 3.20 };
    let mut state =
        TankKinematicState { position: Vec3::new(40.0, 0.0, 40.0), ..Default::default() };
    pivot_for(1.0, &mut state, TankWorldObstacles::new(&[], footprint, &[]));
    assert!(
        state.yaw_rad.abs() > 0.3,
        "an unobstructed pivot must turn freely, got {:.3}",
        state.yaw_rad
    );
}

#[test]
fn an_already_overlapped_hull_keeps_its_freedom_to_rotate_out() {
    // Spawn accident: the hull STARTS inside the wall. The rotation gate must not freeze it —
    // only rotations from a CLEAR pose into a blocked one are refused.
    let footprint = TankFootprint { half_width_m: 1.75, half_length_m: 3.20 };
    let wall = cover_box([40.0, 1.5, 40.0], [2.0, 2.5, 2.0]);
    let mut state =
        TankKinematicState { position: Vec3::new(40.5, 0.0, 40.0), ..Default::default() };
    pivot_for(
        1.0,
        &mut state,
        TankWorldObstacles::new(std::slice::from_ref(&wall), footprint, &[]),
    );
    assert!(
        state.yaw_rad.abs() > 0.3,
        "an overlapped hull must still rotate toward escape, got {:.3}",
        state.yaw_rad
    );
}
