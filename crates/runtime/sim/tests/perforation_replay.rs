//! Honest Steel regression: a penetrating shot must carve the same persistent perforation on
//! every run. The wire golden locks the byte layout; THIS locks the battle itself — identical
//! inputs reproduce an identical `ArmorBreachSet` (contours, seeds, frames), which is what
//! replays and late joiners stand on.

use game_core::{TankId, TankSpec, TeamId};
use glam::Vec3;
use serde::Deserialize;
use sim::{FixedTimestep, SimulationState, TankCommand};

#[derive(Debug, Deserialize)]
struct CombatReplay {
    tick_rate_hz: u32,
    shooter: ReplayTank,
    target: ReplayTank,
    frames: Vec<TankCommand>,
}

#[derive(Debug, Deserialize)]
struct ReplayTank {
    team: u16,
    spec: String,
    position: [f32; 3],
    yaw_rad: f32,
}

fn run(replay: &CombatReplay) -> game_core::ArmorBreachSet {
    let mut state = SimulationState::new();
    let shooter = spawn(&mut state, &replay.shooter);
    let target = spawn(&mut state, &replay.target);
    let step = FixedTimestep::from_hz(replay.tick_rate_hz);
    for command in &replay.frames {
        state.apply_commands(&[(shooter, *command)], step);
    }
    state.tank(target).expect("target tank").armor_breaches.clone()
}

fn spawn(state: &mut SimulationState, tank: &ReplayTank) -> TankId {
    let id = state.spawn_tank(
        TeamId(tank.team),
        match tank.spec.as_str() {
            "t55a" => TankSpec::t55a(),
            "t54_1951" => TankSpec::t54_1951(),
            other => panic!("unsupported replay tank spec: {other}"),
        },
        Vec3::from_array(tank.position),
    );
    state.tank_mut(id).expect("spawned tank").yaw_rad = tank.yaw_rad;
    id
}

#[test]
fn a_penetration_carves_the_same_perforation_on_every_run() {
    let replay: CombatReplay =
        serde_json::from_str(include_str!("replays/perforation_v1.json")).expect("valid replay");

    let first = run(&replay);
    let second = run(&replay);

    assert!(
        first.aperture_group_count() >= 1,
        "the penetrating fixture shot must leave a persistent perforation"
    );
    assert_eq!(first, second, "identical inputs must reproduce the identical ArmorBreachSet");

    // Pinned tightly on purpose: the perforation's identity (plate, place, size) is gameplay
    // truth. Update the values deliberately when ballistics or armor geometry are retuned.
    let breach = &first.breaches()[0];
    let lobe = breach.lobes()[0];
    assert_eq!(first.aperture_group_count(), 1);
    assert_eq!(breach.frame, game_core::ArmorFrame::Hull);
    assert_eq!(breach.zone, game_core::ArmorZone::UpperGlacis);
    assert!(
        (lobe.entry_local - Vec3::new(-0.001533, 1.288322, 2.500_61)).length() < 1.0e-3,
        "the entry point drifted: {:?}",
        lobe.entry_local
    );
    // P5 (Fizyczny Świat): the oblique smear is capped at 1.5x the minor radius — on the
    // sloped glacis this fixture shot now reads 7.5 cm, not the raw-1/cos 9.8 cm slot.
    assert!(
        (lobe.outer.major_radius_m - 0.075).abs() < 1.0e-4,
        "the aperture size drifted: {}",
        lobe.outer.major_radius_m
    );
}
