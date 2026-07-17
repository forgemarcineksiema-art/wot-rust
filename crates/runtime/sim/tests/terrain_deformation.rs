//! Locks true terrain deformation (protocol v31): a high-explosive ground burst excavates a
//! real, replicated crater — the ledger is quantized state, the heightmap overlay folds it into
//! `sample_height`, and the physics seam means a hull genuinely sinks into the hole. Kinetic
//! rounds plough furrows (presentation) but move no earth.

use game_core::{TankSpec, TeamId};
use glam::Vec3;
use sim::{FixedTimestep, MAX_CRATERS, SimulationState, TankCommand, record_high_explosive_burst};
use terrain::HeightMap;

const HE_SLOT: u8 = 2;

fn flat_field() -> HeightMap {
    HeightMap::flat(96, 96, 4.0, 0.0).expect("flat terrain")
}

/// Fire the shooter's currently selected round into the dirt ahead and run the battle until the
/// shell has died.
fn fire_into_the_ground(state: &mut SimulationState, terrain: &HeightMap) {
    let step = FixedTimestep::from_hz(60);
    state.apply_commands_on_battlefield(
        &[(game_core::TankId(1), TankCommand { fire: true, ..TankCommand::idle() })],
        step,
        terrain,
        &[],
    );
    for _ in 0..120 {
        if state.shells().is_empty() {
            break;
        }
        state.apply_commands_on_battlefield(&[], step, terrain, &[]);
    }
    assert!(state.shells().is_empty(), "the shell must have landed");
}

fn spawn_shooter_aiming_at_the_dirt(state: &mut SimulationState, ammo_slot: u8) {
    let shooter = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::new(190.0, 0.0, 150.0));
    let tank = state.tank_mut(shooter).expect("shooter");
    tank.aim_dispersion_mrad = 0.0;
    tank.spec.gun.dispersion_mrad = 0.0;
    tank.gun_pitch_rad = -0.12; // nose the gun down: the round lands a few dozen metres out
    tank.selected_ammo = ammo_slot;
    tank.ammo_counts[ammo_slot as usize] = 5;
}

#[test]
fn a_high_explosive_ground_burst_excavates_a_replicated_crater() {
    let terrain = flat_field();
    let mut state = SimulationState::new();
    spawn_shooter_aiming_at_the_dirt(&mut state, HE_SLOT);

    fire_into_the_ground(&mut state, &terrain);

    assert_eq!(state.craters().len(), 1, "one HE burst, one ledger record");
    let crater = state.craters()[0];
    assert!(
        crater.radius_m() > 1.0 && crater.radius_m() <= 4.0,
        "a 100 mm HE bowl is metres wide: {}",
        crater.radius_m()
    );
    assert!(crater.depth_m() > 0.3, "and genuinely sunk: {}", crater.depth_m());
}

#[test]
fn a_kinetic_round_moves_no_earth() {
    let terrain = flat_field();
    let mut state = SimulationState::new();
    spawn_shooter_aiming_at_the_dirt(&mut state, 0); // slot 0: armor-piercing

    fire_into_the_ground(&mut state, &terrain);

    assert!(
        state.craters().is_empty(),
        "an AP round ploughs a furrow (presentation) but excavates nothing"
    );
}

#[test]
fn reshelling_the_same_spot_deepens_the_crater_instead_of_stacking_records() {
    let mut ledger = Vec::new();
    record_high_explosive_burst(&mut ledger, Vec3::new(100.0, 0.0, 100.0), 122.0);
    let first_depth = ledger[0].depth_m();
    record_high_explosive_burst(&mut ledger, Vec3::new(100.3, 0.0, 100.2), 122.0);

    assert_eq!(ledger.len(), 1, "a burst inside the merge reach re-excavates, not duplicates");
    assert!(ledger[0].depth_m() > first_depth, "and the hole got deeper");
}

#[test]
fn the_ledger_caps_and_the_oldest_crater_weathers_away() {
    let mut ledger = Vec::new();
    for index in 0..(MAX_CRATERS + 8) {
        let x = 50.0 + (index as f32) * 9.0;
        record_high_explosive_burst(&mut ledger, Vec3::new(x, 0.0, 200.0), 122.0);
    }
    assert_eq!(ledger.len(), MAX_CRATERS);
    // The oldest bursts (lowest x) slumped away; the freshest survive.
    assert!(ledger[0].x_m() > 50.0 + 7.0 * 9.0 - 1.0);
}

/// Gameplay sanity (P4c): a crater is cover, not a trap — the bowl's slope stays inside what
/// a tank's drive climbs, so a hull that fell in can always drive back out.
#[test]
fn a_tank_can_climb_out_of_the_deepest_crater() {
    let step = FixedTimestep::from_hz(60);
    let spot = Vec3::new(190.0, 0.0, 150.0);
    let mut shelled = flat_field();
    let crater = terrain::CraterRecord::from_world(
        spot.x,
        spot.z,
        4.0,
        1.2, // the ledger's caps: the deepest, widest hole the wire can carry
        terrain::CRATER_KIND_HIGH_EXPLOSIVE,
    );
    shelled.set_craters(&[crater]);

    let mut state = SimulationState::new();
    let id = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), spot);
    // Settle into the bowl first, then full throttle straight ahead.
    for _ in 0..60 {
        state.apply_commands_on_battlefield(&[(id, TankCommand::idle())], step, &shelled, &[]);
    }
    let full_ahead = TankCommand { throttle: 1.0, ..TankCommand::idle() };
    for _ in 0..600 {
        state.apply_commands_on_battlefield(&[(id, full_ahead)], step, &shelled, &[]);
    }
    let tank = state.tanks().iter().find(|tank| tank.id == id).expect("tank");
    let escaped = (Vec3::new(tank.position.x, 0.0, tank.position.z)
        - Vec3::new(spot.x, 0.0, spot.z))
    .length();
    assert!(
        escaped > crater.influence_radius_m(),
        "ten seconds of throttle clears the hole: {escaped} m from center"
    );
    assert!(tank.position.y > -0.05, "and the hull stands back on grade: y {}", tank.position.y);
}

/// The payoff of the single-seam architecture: fold the ledger into the heightmap the way the
/// battlefield owner does, and a hull parked in the crater PHYSICALLY sits lower than one on
/// virgin ground — hull-down in a fresh shell hole, with no dedicated code path anywhere.
///
/// The hole must out-span the running gear: a 122 mm bowl (2.2 m radius) is BRIDGED — the
/// tracks ride the raised rim spoil, which is itself honest physics. A heavy-howitzer-class
/// crater at the ledger's size cap is what swallows a hull.
#[test]
fn a_hull_in_the_crater_genuinely_sits_lower() {
    let step = FixedTimestep::from_hz(60);
    let spot = Vec3::new(190.0, 0.0, 150.0);

    let settle = |terrain: &HeightMap| {
        let mut state = SimulationState::new();
        let id = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), spot);
        for _ in 0..90 {
            // An idle command each tick: only commanded hulls run ground contact, and the tank
            // must genuinely settle onto (or into) the ground under it.
            state.apply_commands_on_battlefield(&[(id, TankCommand::idle())], step, terrain, &[]);
        }
        state.tanks().iter().find(|tank| tank.id == id).expect("tank").position.y
    };

    let virgin = flat_field();
    let mut shelled = flat_field();
    let crater = terrain::CraterRecord::from_world(
        spot.x,
        spot.z,
        4.0, // the ledger's radius cap: an 8 m bowl, wider than the T-55's wheelbase
        1.2,
        terrain::CRATER_KIND_HIGH_EXPLOSIVE,
    );
    shelled.set_craters(&[crater]);

    let flat_y = settle(&virgin);
    let crater_y = settle(&shelled);
    assert!(
        crater_y < flat_y - 0.25,
        "the tracks stand on the crater floor: {crater_y} vs flat {flat_y}"
    );
}
