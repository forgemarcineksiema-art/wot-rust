//! A wall in the roster (the one program's X6), as the sim bills and holds it.
//!
//! The physics half proves the contact and the dive; this half proves what the sim makes of it:
//! the charge into a tenement is billed through the same `ContactPair` a ram is, and a hull pinned
//! between a pusher and a wall holds its ground instead of losing its velocity every tick.

use game_core::{DamageCause, TankId, TankSpec, TeamId};
use glam::Vec3;
use sim::{FixedTimestep, SimulationState, TankCommand};
use terrain::{HeightMap, StaticCoverKind, StaticCoverObject};

fn step() -> FixedTimestep {
    FixedTimestep::from_hz(60)
}

fn tenement(z: f32) -> StaticCoverObject {
    StaticCoverObject {
        id: "tenement".into(),
        name: "tenement".into(),
        kind: StaticCoverKind::CityBuilding,
        center: [60.0, 5.0, z],
        half_extents_m: [12.0, 5.0, 6.0],
        yaw_rad: 0.0,
    }
}

fn position(state: &SimulationState, id: TankId) -> Vec3 {
    state.tank(id).expect("tank").position
}

/// The row's lock: a charge into a wall bills through the contact pair — a ram event on the
/// hull, its hit points down, the wall intact (what a hull does TO cover is the crush path's).
#[test]
fn a_wall_hit_bills_the_hull_through_the_contact_pair() {
    let map = HeightMap::flat(129, 129, 1.0, 0.0).expect("flat");
    let wall = tenement(110.0);
    let cover = std::slice::from_ref(&wall);
    let mut state = SimulationState::new();
    let spec = TankSpec::t54_1951();
    let charger = state.spawn_tank_with_yaw(TeamId(1), spec, Vec3::new(60.0, 0.0, 30.0), 0.0);
    let hp_before = state.tank(charger).expect("charger").hit_points;
    let go = [(charger, TankCommand::drive(1.0, 0.0))];
    let mut fastest = 0.0_f32;
    for _ in 0..900 {
        state.apply_commands_on_battlefield(&go, step(), &map, cover);
        fastest = fastest.max(state.tank(charger).expect("charger").velocity_mps.length());
        if state.damage_events().iter().any(|event| event.cause == DamageCause::Ram) {
            break;
        }
    }
    assert!(fastest > 8.0, "the runway must let the hull charge, peaked at {fastest} m/s");
    let hp_after = state.tank(charger).expect("charger").hit_points;
    assert!(
        state.damage_events().iter().any(|event| {
            event.cause == DamageCause::Ram && event.target == charger && event.damage_hp > 0
        }),
        "a charge into a tenement is a ram the hull pays for"
    );
    assert!(hp_after < hp_before, "...in hit points: {hp_before} -> {hp_after}");
    let face_z = wall.center[2] - wall.half_extents_m[2];
    let nose = position(&state, charger).z
        + state.tank(charger).expect("t").spec.hull_plan().half_length_m;
    assert!(nose <= face_z + 0.03, "the hull stops at the face, nose {nose} vs {face_z}");
}

/// ...and the pinned hull holds: a pusher grinding a hull into a wall neither drives it through
/// the wall nor shakes it — the contact holds both, the wall holds the pair.
#[test]
fn a_hull_pinned_against_a_wall_holds() {
    let map = HeightMap::flat(129, 129, 1.0, 0.0).expect("flat");
    let wall = tenement(80.0);
    let cover = std::slice::from_ref(&wall);
    let mut state = SimulationState::new();
    let spec = TankSpec::t54_1951();
    let half_len = spec.hull_plan().half_length_m;
    let face_z = wall.center[2] - wall.half_extents_m[2];
    // The pinned hull parked touching the wall; the pusher a metre behind it, driving.
    let pinned = state.spawn_tank_with_yaw(
        TeamId(1),
        spec.clone(),
        Vec3::new(60.0, 0.0, face_z - half_len - 0.02),
        0.0,
    );
    let pusher = state.spawn_tank_with_yaw(
        TeamId(1),
        spec,
        Vec3::new(60.0, 0.0, face_z - 3.0 * half_len - 1.0),
        0.0,
    );
    let go = [(pinned, TankCommand::drive(0.0, 0.0)), (pusher, TankCommand::drive(1.0, 0.0))];
    let start = position(&state, pinned);
    let mut furthest = 0.0_f32;
    let mut speeds = Vec::new();
    for tick in 0..600 {
        state.apply_commands_on_battlefield(&go, step(), &map, cover);
        furthest = furthest.max((position(&state, pinned) - start).length());
        if tick >= 300 {
            speeds.push(state.tank(pinned).expect("pinned").velocity_mps.length());
        }
    }
    let nose = position(&state, pinned).z + half_len;
    assert!(nose <= face_z + 0.03, "the pinned hull never enters the wall, nose {nose}");
    assert!(furthest < 0.10, "the pinned hull holds its ground, moved {furthest} m");
    let jitter = speeds.iter().copied().fold(0.0_f32, f32::max);
    assert!(jitter < 0.05, "...without shaking: {jitter} m/s in the second half");
}
