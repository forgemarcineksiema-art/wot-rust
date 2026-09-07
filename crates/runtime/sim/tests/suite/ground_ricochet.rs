//! Z7 (the one program, inherited): shells ricochet off the ground. A kinetic round meeting flat
//! ground at a graze skips off it — once, mirrored, slower, blunted — and flies on; a round
//! plunging into a slope facing it dies there; a high-explosive charge never skips, it goes
//! off where it lands.

use game_core::{TankSpec, TeamId};
use glam::Vec3;
use sim::{FixedTimestep, SimulationState, TankCommand};

const HE_SLOT: u8 = 2;

fn fire(state: &mut SimulationState, shooter: game_core::TankId, heightmap: &terrain::HeightMap) {
    let step = FixedTimestep::from_hz(60);
    state.apply_commands_on_terrain(
        &[(shooter, TankCommand { fire: true, ..TankCommand::idle() })],
        step,
        heightmap,
    );
}

/// Ticks until the first ground impact is recorded; returns whether the shell survived it.
fn survives_the_ground(
    state: &mut SimulationState,
    heightmap: &terrain::HeightMap,
) -> (bool, Vec3) {
    let step = FixedTimestep::from_hz(60);
    for _ in 0..600 {
        state.apply_commands_on_terrain(&[], step, heightmap);
        if let Some(impact) = state
            .shell_impacts()
            .iter()
            .find(|impact| impact.surface == game_core::ImpactSurface::Terrain)
        {
            let position = impact.position;
            let alive = state.shells().first().copied();
            return (alive.is_some_and(|shell| shell.ricocheted_once), position);
        }
        if state.shells().is_empty() {
            break;
        }
    }
    (false, Vec3::ZERO)
}

#[test]
fn a_kinetic_round_skips_off_flat_ground_at_a_graze_and_dies_on_a_facing_slope() {
    let flat = terrain::heightmap_from_fn(81, 5.0, |_, _| 0.0);
    let mut state = SimulationState::new();
    let shooter = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::new(200.0, 0.0, 100.0));
    state.tank_mut(shooter).expect("shooter").gun_pitch_rad = -4.0f32.to_radians();
    fire(&mut state, shooter, &flat);
    let (skipped, impact) = survives_the_ground(&mut state, &flat);
    assert!(skipped, "a four-degree graze skips off the field (impact at {impact})");
    let shell = state.shells()[0];
    assert!(shell.velocity_mps.y > 0.0, "and climbs away from the ground: {}", shell.velocity_mps);
    assert!(shell.velocity_mps.length() < 895.0 * 0.85, "slower");
    let step = FixedTimestep::from_hz(60);
    for _ in 0..1200 {
        state.apply_commands_on_terrain(&[], step, &flat);
    }
    assert!(state.shells().is_empty(), "one skip only; the next ground resolves it");

    // A slope rising one-in-one across the shot's path, twenty metres out: a plunge, no skip.
    let bank = terrain::heightmap_from_fn(81, 5.0, |_, z| (z - 125.0).max(0.0));
    let mut state = SimulationState::new();
    let shooter = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::new(200.0, 0.0, 100.0));
    fire(&mut state, shooter, &bank);
    let (skipped, impact) = survives_the_ground(&mut state, &bank);
    assert!(!skipped, "a forty-five degree plunge dies in the bank (impact at {impact})");
    assert!(state.shells().is_empty());

    // The charge never skips: HE at the same graze goes off on the field.
    let mut state = SimulationState::new();
    let shooter = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::new(200.0, 0.0, 100.0));
    state.tank_mut(shooter).expect("shooter").gun_pitch_rad = -4.0f32.to_radians();
    let step = FixedTimestep::from_hz(60);
    state.apply_commands_on_terrain(
        &[(shooter, TankCommand { select_ammo: Some(HE_SLOT), ..TankCommand::idle() })],
        step,
        &flat,
    );
    fire(&mut state, shooter, &flat);
    let (skipped, _) = survives_the_ground(&mut state, &flat);
    assert!(!skipped, "a high-explosive round goes off where it lands");
}
