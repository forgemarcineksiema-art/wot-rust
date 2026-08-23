//! Locks for back-face spalling: a round that FAILED to penetrate but came within the back-face
//! margin of beating the plate breaks fragments off its inner face — and the first thing each
//! fragment meets, a man or a module, takes it.
//!
//! The promises, one per test: a near-penetration rattles the crew behind the plate; a shell far
//! from penetrating spalls nothing; the hull pool is NEVER touched ("my armor held" stays true on
//! the HP bar); at most one crewman is wounded per spalling shell; and HE keeps its own
//! non-penetration identity (the surface chip) instead of spalling. The trigger's other clause —
//! a true ricochet never spalls — is locked at the predicate in `sim/src/combat.rs`.

use std::f32::consts::PI;

use game_core::{CrewMemberState, CrewRole, ShellSpec, TankId, TankSpec, TeamId};
use glam::Vec3;
use sim::SimulationState;

mod common;
use common::run_until_shells_clear;

/// One flank shot at the T-54 tower, aimed down the gunner's station line — the same aim the
/// crew-damage locks use, with the shell swapped so the plate HOLDS. Returns the state after
/// resolution for whatever the test wants to read.
fn tower_flank_shot(shell: ShellSpec, z_offset: f32) -> (SimulationState, TankId) {
    let mut state = SimulationState::new();
    let shooter =
        state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::new(-55.0, 0.0, z_offset));
    let target = state.spawn_tank(TeamId(2), TankSpec::t54_1951(), Vec3::ZERO);
    {
        let shooter = state.tank_mut(shooter).expect("shooter");
        shooter.yaw_rad = PI / 2.0;
        // Depress onto the tower flank at torso height (~1.54 m world), exactly like the
        // crew-damage locks: the tower crew's seated capsules top out near 1.8.
        shooter.gun_pitch_rad = -0.004;
        shooter.aim_dispersion_mrad = 0.0;
        shooter.spec.gun.dispersion_mrad = 0.0;
        shooter.spec.gun.shell = shell;
    }
    run_until_shells_clear(&mut state, shooter);
    (state, target)
}

/// The tower flank measures ~133 mm effective (see `crew_damage.rs`): 125 mm of penetration
/// fails by single millimetres — inside the 12% back-face margin.
fn near_penetration_shell() -> ShellSpec {
    ShellSpec::armor_piercing(100.0, 900.0, 125.0, 320)
}

#[test]
fn a_near_penetration_rattles_the_crew_behind_the_plate() {
    let (state, target) = tower_flank_shot(near_penetration_shell(), 0.26);
    let event = state.damage_events().last().expect("the shot resolved");
    assert!(!event.penetrated, "the lock needs the plate to HOLD");
    assert!(!event.ricocheted, "the lock needs a dull thud, not a skid");
    assert_eq!(
        event.crew_hits_mask,
        CrewRole::Gunner.mask_bit(),
        "the fragments off the inner face took the man behind the plate"
    );
    let tank = state.tank(target).expect("target");
    assert!(
        matches!(tank.crew.state(CrewRole::Gunner), CrewMemberState::Unconscious { .. }),
        "the gunner is down without the shell ever getting in"
    );
}

#[test]
fn a_shell_far_from_penetrating_spalls_nothing() {
    // 60 mm against ~133 mm effective: the plate shrugged it off — no back-face failure.
    let (state, target) =
        tower_flank_shot(ShellSpec::armor_piercing(100.0, 900.0, 60.0, 320), 0.26);
    let event = state.damage_events().last().expect("the shot resolved");
    assert!(!event.penetrated);
    assert_eq!(event.crew_hits_mask, 0, "a shrugged-off hit rattles nobody");
    let tank = state.tank(target).expect("target");
    assert_eq!(tank.crew.state(CrewRole::Gunner), CrewMemberState::Active);
    assert_eq!(tank.crew.state(CrewRole::Commander), CrewMemberState::Active);
}

#[test]
fn backface_spall_never_touches_the_hull_pool() {
    let (state, target) = tower_flank_shot(near_penetration_shell(), 0.26);
    let event = state.damage_events().last().expect("the shot resolved");
    assert_eq!(event.crew_hits_mask, CrewRole::Gunner.mask_bit(), "the spall lock needs its wound");
    assert_eq!(event.damage_hp, 0, "spalling is a crew/module mechanic, never hull damage");
    let tank = state.tank(target).expect("target");
    assert_eq!(
        tank.hit_points, tank.spec.hit_points,
        "the armor held and the HP bar says so, wounded gunner notwithstanding"
    );
}

#[test]
fn one_spalling_shell_wounds_at_most_one_crewman() {
    // The tower flank at z 0.26 has the gunner AND the commander seated within cone reach; the
    // hard cap says the first fragment to find a man ends the crew story for this shell.
    let (state, target) = tower_flank_shot(near_penetration_shell(), 0.26);
    let event = state.damage_events().last().expect("the shot resolved");
    assert!(
        event.crew_hits_mask.count_ones() <= 1,
        "one near-penetration wounds at most one man (mask {:#010b})",
        event.crew_hits_mask
    );
    let tank = state.tank(target).expect("target");
    let down = CrewRole::ALL
        .iter()
        .filter(|role| matches!(tank.crew.state(**role), CrewMemberState::Unconscious { .. }))
        .count();
    assert!(down <= 1, "the cap holds in the crew state too, not only on the wire mask");
}

#[test]
fn a_high_explosive_slap_does_not_spall() {
    // Same penetration deficit as the AP near-penetration — but HE's non-penetration identity is
    // the 18% surface chip plus splash, not back-face fragments.
    let (state, target) =
        tower_flank_shot(ShellSpec::high_explosive(100.0, 900.0, 125.0, 320, 1.6), 0.26);
    let event = state.damage_events().last().expect("the shot resolved");
    assert!(!event.penetrated);
    assert_eq!(event.crew_hits_mask, 0, "HE spalls nobody; its chip damage is its identity");
    assert!(event.damage_hp > 0, "the surface chip is unchanged by the spall feature");
    let tank = state.tank(target).expect("target");
    assert_eq!(tank.crew.state(CrewRole::Gunner), CrewMemberState::Active);
}
