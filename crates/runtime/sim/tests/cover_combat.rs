use std::f32::consts::PI;

use game_core::{TankId, TankSpec, TeamId};
use glam::Vec3;
use sim::{FixedTimestep, SimulationState, TankCommand};
use terrain::{HeightMap, StaticCoverKind, StaticCoverObject};

#[test]
fn static_cover_absorbs_a_shell_before_it_reaches_the_target() {
    let (mut state, shooter, target) = duel();
    let target_hp = state.tank(target).expect("target").hit_points;

    fire_and_settle(&mut state, shooter, &flat_field(), &[wall_at(0.0)]);

    assert!(state.damage_events().is_empty(), "cover should absorb the shell, no hit");
    assert_eq!(state.tank(target).expect("target").hit_points, target_hp);
    assert!(state.shells().is_empty(), "the absorbed shell should be removed at the cover");
}

#[test]
fn cover_off_to_the_side_does_not_block_a_clear_shot() {
    let (mut state, shooter, target) = duel();
    // Face the target away: at 55 m the flat shot arrives just above the 1.75 hull-roof split, so
    // it meets the turret — the exposed rear plate guarantees the clear shot also penetrates.
    state.tank_mut(target).expect("target").yaw_rad = 0.0;

    fire_and_settle(&mut state, shooter, &flat_field(), &[wall_at(20.0)]);

    let event = state.damage_events().last().expect("a clear shot should resolve on the target");
    assert_eq!(event.target, target);
    assert!(event.penetrated);
}

fn duel() -> (SimulationState, TankId, TankId) {
    let mut state = SimulationState::new();
    let shooter = state.spawn_tank(TeamId(1), TankSpec::t55a(), Vec3::ZERO);
    let target = state.spawn_tank(TeamId(2), TankSpec::t54_1951(), Vec3::new(0.0, 0.0, 55.0));
    {
        let shooter = state.tank_mut(shooter).expect("shooter");
        shooter.aim_dispersion_mrad = 0.0;
        shooter.spec.gun.dispersion_mrad = 0.0;
    }
    state.tank_mut(target).expect("target").yaw_rad = PI;
    (state, shooter, target)
}

fn fire_and_settle(
    state: &mut SimulationState,
    shooter: TankId,
    terrain: &HeightMap,
    cover: &[StaticCoverObject],
) {
    let step = FixedTimestep::from_hz(60);
    state.apply_commands_on_battlefield(
        &[(shooter, TankCommand { fire: true, ..TankCommand::idle() })],
        step,
        terrain,
        cover,
    );
    for _ in 0..30 {
        state.apply_commands_on_battlefield(&[], step, terrain, cover);
        if state.shells().is_empty() || !state.damage_events().is_empty() {
            break;
        }
    }
}

fn flat_field() -> HeightMap {
    HeightMap::flat(64, 64, 2.0, 0.0).expect("flat terrain")
}

/// A wall straddling the shell's path height at z = 27 (between shooter at 0 and target at 55),
/// centered at the given x so it can be placed in the line of fire or off to the side.
fn wall_at(x: f32) -> StaticCoverObject {
    StaticCoverObject {
        id: "test_wall".to_string(),
        name: "test wall".to_string(),
        kind: StaticCoverKind::FarmBuilding,
        center: [x, 1.5, 27.0],
        half_extents_m: [4.0, 2.5, 1.5],
    }
}

#[test]
fn static_cover_stops_a_tank_driving_into_it() {
    let terrain = HeightMap::flat(64, 64, 4.0, 0.0).expect("flat terrain");
    let barn = StaticCoverObject {
        id: "barn".to_string(),
        name: "barn".to_string(),
        kind: StaticCoverKind::FarmBuilding,
        center: [10.0, 1.5, 30.0],
        half_extents_m: [5.0, 2.5, 4.0],
    };

    let mut state = SimulationState::new();
    let tank = state.spawn_tank(TeamId(1), TankSpec::t55a(), Vec3::new(10.0, 0.0, 10.0));
    let step = FixedTimestep::from_hz(60);
    for _ in 0..240 {
        state.apply_commands_on_battlefield(
            &[(tank, TankCommand::drive(1.0, 0.0))],
            step,
            &terrain,
            std::slice::from_ref(&barn),
        );
    }

    let z = state.tank(tank).expect("tank").position.z;
    assert!(z < 25.0, "cover should stop the tank short of the barn (z = {z})");
    assert!(z > 20.0, "the tank should still advance up to the barn (z = {z})");

    // Negative contact lock: the hull's front face must end at (not inside) the barn wall.
    // The old point-radius blocker buried the T-55A's nose 1.6 m deep into the building.
    let tank = state.tank(tank).expect("tank");
    let hull_front_z = tank.position.z + tank.spec.hitbox.half_length_m;
    let barn_face_z = 30.0 - 4.0;
    assert!(
        hull_front_z <= barn_face_z + 1.0e-2,
        "hull front must stay outside the barn footprint (front z {hull_front_z} vs face {barn_face_z})"
    );
}
