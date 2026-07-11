//! The fire input buffer: a click landing a hair before the reload completes (inside
//! `FIRE_BUFFER_S`) is HELD and released on the exact tick the breech closes — a click on the
//! visually-ready reticle is never silently swallowed by a few hundredths of a second of
//! authoritative reload. Earlier clicks stay genuine misfires; an ammo switch drops the held
//! click; a gun that dies while the click is held fires nothing.

use game_core::{ModuleSlot, TankSpec, TeamId};
use glam::Vec3;
use sim::{FixedTimestep, SimulationState, TankCommand};

fn fire() -> TankCommand {
    TankCommand { fire: true, ..TankCommand::idle() }
}

fn idle() -> TankCommand {
    TankCommand::idle()
}

/// Rounds fired so far, measured by ammo spent (live-shell counts decay as shells land).
fn shots_fired(state: &SimulationState, id: game_core::TankId, initial_total: u32) -> u32 {
    let tank = state.tank(id).expect("tank");
    initial_total - tank.ammo_counts.iter().map(|&c| c as u32).sum::<u32>()
}

fn total_ammo(state: &SimulationState, id: game_core::TankId) -> u32 {
    state.tank(id).expect("tank").ammo_counts.iter().map(|&c| c as u32).sum()
}

/// Run ticks until the tank's reload sits just under `target_s` remaining.
fn run_reload_down_to(state: &mut SimulationState, id: game_core::TankId, target_s: f32) {
    let step = FixedTimestep::from_hz(60);
    for _ in 0..20 * 60 {
        if state.tank(id).expect("tank").reload_remaining_s <= target_s {
            return;
        }
        state.apply_commands(&[(id, idle())], step);
    }
    panic!("reload never reached {target_s}");
}

#[test]
fn a_click_just_before_ready_fires_the_tick_the_breech_closes() {
    let mut state = SimulationState::new();
    let shooter = state.spawn_tank(TeamId(1), TankSpec::t55a(), Vec3::ZERO);
    let step = FixedTimestep::from_hz(60);

    let ammo0 = total_ammo(&state, shooter);
    // First shot starts the reload.
    state.apply_commands(&[(shooter, fire())], step);
    assert_eq!(shots_fired(&state, shooter, ammo0), 1);

    // Click again with ~0.2 s of reload left: inside the buffer window - held, not swallowed.
    run_reload_down_to(&mut state, shooter, 0.2);
    state.apply_commands(&[(shooter, fire())], step);
    assert_eq!(shots_fired(&state, shooter, ammo0), 1, "the click is held, not fired early");

    // No further clicks: the held one fires by itself the moment the reload completes.
    for _ in 0..30 {
        state.apply_commands(&[(shooter, idle())], step);
    }
    assert_eq!(shots_fired(&state, shooter, ammo0), 2, "the held click fired on reload completion");
    assert!(
        state.tank(shooter).expect("shooter").reload_remaining_s > 0.0,
        "the released shot started its own reload"
    );
}

#[test]
fn a_click_long_before_ready_still_refuses() {
    let mut state = SimulationState::new();
    let shooter = state.spawn_tank(TeamId(1), TankSpec::t55a(), Vec3::ZERO);
    let step = FixedTimestep::from_hz(60);

    let ammo0 = total_ammo(&state, shooter);
    state.apply_commands(&[(shooter, fire())], step);
    assert_eq!(shots_fired(&state, shooter, ammo0), 1);

    // Click with most of the reload left: a genuine misfire, nothing buffers.
    state.apply_commands(&[(shooter, fire())], step);
    for _ in 0..20 * 60 {
        state.apply_commands(&[(shooter, idle())], step);
        if state.tank(shooter).expect("shooter").reload_remaining_s <= 0.0 {
            break;
        }
    }
    // Give the (nonexistent) buffer a few ticks to betray itself.
    for _ in 0..10 {
        state.apply_commands(&[(shooter, idle())], step);
    }
    assert_eq!(
        shots_fired(&state, shooter, ammo0),
        1,
        "an early click must not fire a delayed shot"
    );
}

#[test]
fn an_ammo_switch_drops_the_held_click() {
    let mut state = SimulationState::new();
    let shooter = state.spawn_tank(TeamId(1), TankSpec::t55a(), Vec3::ZERO);
    let step = FixedTimestep::from_hz(60);

    let ammo0 = total_ammo(&state, shooter);
    state.apply_commands(&[(shooter, fire())], step);
    run_reload_down_to(&mut state, shooter, 0.2);
    state.apply_commands(&[(shooter, fire())], step);

    // Switch ammo while the click is held: the reload restarts and the click must die with it.
    let other_slot = (state.tank(shooter).expect("shooter").selected_ammo + 1) % 2;
    state.apply_commands(
        &[(shooter, TankCommand { select_ammo: Some(other_slot), ..TankCommand::idle() })],
        step,
    );
    for _ in 0..20 * 60 {
        state.apply_commands(&[(shooter, idle())], step);
    }
    assert_eq!(
        shots_fired(&state, shooter, ammo0),
        1,
        "the switched reload must not release the stale held click"
    );
}

#[test]
fn a_gun_that_dies_while_the_click_is_held_fires_nothing() {
    let mut state = SimulationState::new();
    let shooter = state.spawn_tank(TeamId(1), TankSpec::t55a(), Vec3::ZERO);
    let step = FixedTimestep::from_hz(60);

    let ammo0 = total_ammo(&state, shooter);
    state.apply_commands(&[(shooter, fire())], step);
    run_reload_down_to(&mut state, shooter, 0.2);
    state.apply_commands(&[(shooter, fire())], step);

    // The gun module dies before the reload completes.
    let gun_hp = state.tank(shooter).expect("shooter").modules.hit_points(ModuleSlot::Gun);
    state.tank_mut(shooter).expect("shooter").modules.damage(ModuleSlot::Gun, gun_hp);
    for _ in 0..60 {
        state.apply_commands(&[(shooter, idle())], step);
    }
    assert_eq!(shots_fired(&state, shooter, ammo0), 1, "a dead gun releases nothing");
}
