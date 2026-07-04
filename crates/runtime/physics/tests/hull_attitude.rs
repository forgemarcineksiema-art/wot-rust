//! Locking tests for the authoritative hull attitude: the hull settles onto slopes, rolls on a
//! traverse, holds level before a crest and noses down past it, freezes its attitude in flight,
//! and stays bit-deterministic.

use game_core::{ContactFootprint, TankSpec, VehicleKind};
use glam::Vec3;
use physics::{
    TankControlInput, TankControllerSettings, TankFootprint, TankKinematicState,
    TankWorldObstacles, step_tank_on_world_with_tanks,
};
use terrain::HeightMap;

const DT: f32 = 1.0 / 60.0;
const HOLD: TankControlInput = TankControlInput { throttle: 0.0, steer: 0.0, brake: 1.0 };

fn map_from(height_at: impl Fn(f32, f32) -> f32) -> HeightMap {
    let mut samples = Vec::with_capacity(61 * 61);
    for z in 0..61 {
        for x in 0..61 {
            samples.push(height_at(x as f32, z as f32));
        }
    }
    HeightMap::new(61, 61, 1.0, samples).expect("test heightmap dimensions are fixed")
}

fn settle(
    map: &HeightMap,
    position: Vec3,
    input: TankControlInput,
    ticks: usize,
) -> TankKinematicState {
    let spec = TankSpec::t54_1951();
    let settings = TankControllerSettings::from_spec(&spec);
    let footprint = ContactFootprint::for_vehicle(VehicleKind::T54_1951);
    let obstacles = TankFootprint::from_hitbox(spec.hitbox);
    let mut state = TankKinematicState { position, ..TankKinematicState::default() };
    for _ in 0..ticks {
        step_tank_on_world_with_tanks(
            &mut state,
            input,
            &settings,
            Some(map),
            TankWorldObstacles::new(&[], obstacles, &[]),
            Some(&footprint),
            DT,
        );
    }
    state
}

#[test]
fn the_hull_settles_onto_a_uniform_slope() {
    // Ground rises 0.3 per metre ahead (+z is the hull's forward at yaw 0): nose up.
    let map = map_from(|_, z| z * 0.3);
    let state = settle(&map, Vec3::new(30.0, 9.0, 30.0), HOLD, 240);
    let expected = 0.3_f32.atan();
    assert!(
        (state.pitch_rad - expected).abs() < 0.03,
        "pitch settles to the slope: {} vs {expected}",
        state.pitch_rad
    );
    assert!(state.roll_rad.abs() < 0.02, "no roll on a pure grade: {}", state.roll_rad);
}

#[test]
fn a_side_slope_rolls_the_hull() {
    // Ground rises 0.25 per metre to the right (+x at yaw 0): right side up.
    let map = map_from(|x, _| x * 0.25);
    let state = settle(&map, Vec3::new(30.0, 8.0, 30.0), HOLD, 240);
    let expected = 0.25_f32.atan();
    assert!(
        (state.roll_rad - expected).abs() < 0.03,
        "roll settles to the traverse: {} vs {expected}",
        state.roll_rad
    );
    assert!(state.pitch_rad.abs() < 0.02, "no pitch on a pure traverse: {}", state.pitch_rad);
}

#[test]
fn the_nose_holds_level_before_a_crest_and_drops_past_it() {
    // Flat plateau at 3.0 breaking into a 0.5-grade downhill at z = 30.
    let map = map_from(|_, z| if z < 30.0 { 3.0 } else { 3.0 - (z - 30.0) * 0.5 });
    let before = settle(&map, Vec3::new(30.0, 3.0, 27.0), HOLD, 240);
    assert!(
        before.pitch_rad.abs() < 0.03,
        "level while the gear is on the plateau: {}",
        before.pitch_rad
    );
    let past = settle(&map, Vec3::new(30.0, 3.0, 33.0), HOLD, 240);
    assert!(past.pitch_rad < -0.35, "nosed down onto the far slope: {}", past.pitch_rad);
}

#[test]
fn flight_freezes_the_attitude_until_landing() {
    // A 6 m plateau ending in a sheer face at z = 30: driving off goes ballistic.
    let map = map_from(|_, z| if z < 30.0 { 6.0 } else { 0.0 });
    let spec = TankSpec::t54_1951();
    let settings = TankControllerSettings::from_spec(&spec);
    let footprint = ContactFootprint::for_vehicle(VehicleKind::T54_1951);
    let obstacles = TankFootprint::from_hitbox(spec.hitbox);
    let mut state = TankKinematicState {
        position: Vec3::new(30.0, 6.0, 20.0),
        ..TankKinematicState::default()
    };
    let full = TankControlInput { throttle: 1.0, steer: 0.0, brake: 0.0 };
    let mut launch_pitch = None;
    let mut checked_mid_flight = false;
    for _ in 0..(20.0 / DT) as usize {
        let step = step_tank_on_world_with_tanks(
            &mut state,
            full,
            &settings,
            Some(&map),
            TankWorldObstacles::new(&[], obstacles, &[]),
            Some(&footprint),
            DT,
        );
        match (step.grounded, launch_pitch) {
            (false, None) => launch_pitch = Some(state.pitch_rad),
            (false, Some(pitch)) => {
                assert_eq!(pitch.to_bits(), state.pitch_rad.to_bits(), "attitude frozen in flight");
                checked_mid_flight = true;
            }
            (true, Some(_)) => break,
            (true, None) => {}
        }
    }
    assert!(launch_pitch.is_some(), "the run must go airborne off the cliff");
    assert!(checked_mid_flight, "the flight must last more than one tick");
}

#[test]
fn attitude_is_bit_deterministic_across_runs() {
    let map = map_from(|x, z| (z * 0.17).sin() * 0.8 + (x * 0.23).cos() * 0.5 + 5.0);
    let drive = TankControlInput { throttle: 0.8, steer: 0.2, brake: 0.0 };
    let a = settle(&map, Vec3::new(30.0, 6.0, 20.0), drive, 600);
    let b = settle(&map, Vec3::new(30.0, 6.0, 20.0), drive, 600);
    assert_eq!(a.pitch_rad.to_bits(), b.pitch_rad.to_bits());
    assert_eq!(a.roll_rad.to_bits(), b.roll_rad.to_bits());
    assert_eq!(a.position, b.position);
}
