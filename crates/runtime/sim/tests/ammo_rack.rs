//! Locks the ammo rack (protocol v15): a spawned tank carries `spec.ammo`, firing consumes the
//! selected slot and an empty one refuses, and switching slots restarts the full reload â€” the
//! simple honest rule: the loader swaps the round out of the breech.

use game_core::{ShellType, TankSpec, TeamId};
use glam::Vec3;
use sim::{FixedTimestep, SimulationState, TankCommand};

fn step() -> FixedTimestep {
    FixedTimestep::from_hz(60)
}

fn fire() -> TankCommand {
    TankCommand { fire: true, ..TankCommand::idle() }
}

fn select(slot: u8) -> TankCommand {
    TankCommand { select_ammo: Some(slot), ..TankCommand::idle() }
}

#[test]
fn a_spawned_tank_carries_its_spec_loadout_and_firing_decrements_the_selected_slot() {
    let mut state = SimulationState::new();
    let spec = TankSpec::medium_test_tank();
    let expected = spec.ammo.counts;
    let id = state.spawn_tank(TeamId(1), spec, Vec3::ZERO);
    assert_eq!(state.tank(id).unwrap().ammo_counts, expected, "spawn starts from spec.ammo");
    assert!(expected[0] > 0, "the default fill is stock-heavy, not empty");

    state.apply_commands(&[(id, fire())], step());

    let tank = state.tank(id).unwrap();
    assert_eq!(state.shells().len(), 1, "a loaded slot fires");
    assert_eq!(tank.ammo_counts[0], expected[0] - 1, "the shot came out of the selected slot");
    assert_eq!(tank.ammo_counts[1], expected[1], "other slots are untouched");
}

#[test]
fn an_empty_slot_refuses_to_fire() {
    let mut state = SimulationState::new();
    let id = state.spawn_tank(TeamId(1), TankSpec::medium_test_tank(), Vec3::ZERO);
    state.tank_mut(id).unwrap().ammo_counts[0] = 0;

    state.apply_commands(&[(id, fire())], step());

    assert!(state.shells().is_empty(), "an empty rack slot must not fire");
    assert_eq!(state.tank(id).unwrap().reload_remaining_s, 0.0, "no reload was spent");
}

#[test]
fn switching_ammo_restarts_the_reload_and_changes_the_fired_shell_type() {
    let mut state = SimulationState::new();
    let id = state.spawn_tank(TeamId(1), TankSpec::medium_test_tank(), Vec3::ZERO);
    let reload_seconds = state.tank(id).unwrap().spec.gun.reload_seconds;

    // Slot 2 is HE in ammo_options() order (stock, APCR, HE).
    state.apply_commands(&[(id, select(2))], step());
    let tank = state.tank(id).unwrap();
    assert_eq!(tank.selected_ammo, 2);
    assert!(
        tank.reload_remaining_s >= reload_seconds - 2.0 * step().dt_seconds(),
        "a real switch restarts the full reload, got {} of {}",
        tank.reload_remaining_s,
        reload_seconds
    );

    // Once reloaded, the gun fires the newly selected shell type.
    state.tank_mut(id).unwrap().reload_remaining_s = 0.0;
    state.apply_commands(&[(id, fire())], step());
    let shell = state.shells().last().expect("the switched slot fires");
    assert_eq!(shell.shell.shell_type, ShellType::HighExplosive);
    assert_eq!(state.tank(id).unwrap().ammo_counts[2], {
        let mut expected = TankSpec::medium_test_tank().ammo.counts[2];
        expected -= 1;
        expected
    });
}

#[test]
fn same_slot_or_invalid_switch_requests_are_no_ops() {
    let mut state = SimulationState::new();
    let id = state.spawn_tank(TeamId(1), TankSpec::medium_test_tank(), Vec3::ZERO);

    state.apply_commands(&[(id, select(0))], step());
    let tank = state.tank(id).unwrap();
    assert_eq!(tank.selected_ammo, 0);
    assert_eq!(tank.reload_remaining_s, 0.0, "re-selecting the loaded slot must not reload");

    state.apply_commands(&[(id, select(7))], step());
    let tank = state.tank(id).unwrap();
    assert_eq!(tank.selected_ammo, 0, "an out-of-range slot is ignored");
    assert_eq!(tank.reload_remaining_s, 0.0);
}
