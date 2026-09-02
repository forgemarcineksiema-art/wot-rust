//! Inny Poziom S13: the player's own shot is in the FRAME AFTER THE TRIGGER. The fan-out used to
//! wait for the 20 Hz snapshot's `shots_fired`; now the accepted click is predicted from the
//! client's copy of the server's rule and fanned out in the same fixed tick, and the replicated
//! shot that follows is matched by count and skipped.

use super::fire_fx_tests::{TICKS_PAST_SNAPSHOT, battle_ready_app};

/// The first tick after the trigger already carries the shot: the muzzle FX pool is lit, the
/// shot's light is in the frame's slots, the barrel is in recoil — before any snapshot could
/// have reported the shot.
#[test]
fn the_shot_is_in_the_tick_after_the_trigger_before_any_snapshot() {
    let mut app = battle_ready_app();
    assert_eq!(app.fx.live_particles(), 0);
    // Land exactly on a snapshot so the next single tick cannot cross another one.
    while app.ticks_since_snapshot != 0 {
        app.run_fixed_ticks(1);
    }

    app.input.fire_pending = true;
    app.run_fixed_ticks(1);

    assert_eq!(app.ticks_since_snapshot, 1, "one tick, no snapshot crossed");
    assert_eq!(app.fire_events_applied, 1, "one fan-out, from the prediction");
    assert!(
        app.fx.live_particles() >= 10,
        "the muzzle blast is in the frame after the trigger, got {}",
        app.fx.live_particles()
    );
    assert!(app.fx.local_lights()[0].radius_m > 0.0, "and so is the shot's light");
    app.presentation.advance_time(1.0 / 60.0);
    let tanks = app.project_render_tanks(1.0);
    let player = tanks.iter().find(|tank| tank.id == app.player_tank).expect("player tank");
    assert!(player.gun_recoil_m > 0.01, "barrel in recoil, got {} m", player.gun_recoil_m);
    assert_eq!(app.own_shot.unconfirmed_count(), 1, "waiting for its replicated twin");
}

/// When the snapshot reports the same shot, nothing plays twice: the particle pool and the
/// light slots stay what the prediction put there, and the prediction is confirmed.
#[test]
fn the_replicated_twin_is_skipped_so_nothing_plays_twice() {
    let mut app = battle_ready_app();
    while app.ticks_since_snapshot != 0 {
        app.run_fixed_ticks(1);
    }
    app.input.fire_pending = true;
    app.run_fixed_ticks(1);
    let predicted = app.fx.live_particles();
    assert!(predicted >= 10);

    // Cross the snapshot that carries the server's `ShotFired` for this shot.
    app.run_fixed_ticks(TICKS_PAST_SNAPSHOT);
    assert_eq!(app.fire_events_applied, 1, "the replicated shot added no second fan-out");
    assert_eq!(app.own_shot.unconfirmed_count(), 0, "confirmed by the snapshot");
}

/// A second click inside the reload predicts nothing, even before the snapshot has reported the
/// reload: the client counts its own lockout.
#[test]
fn a_click_inside_the_reload_predicts_no_second_flash() {
    let mut app = battle_ready_app();
    while app.ticks_since_snapshot != 0 {
        app.run_fixed_ticks(1);
    }
    app.input.fire_pending = true;
    app.run_fixed_ticks(1);
    let after_first = app.fx.live_particles();

    app.input.fire_pending = true;
    app.run_fixed_ticks(1);
    assert_eq!(app.fx.live_particles(), after_first, "the local lockout holds the second click");
    assert_eq!(app.own_shot.unconfirmed_count(), 1, "still one prediction outstanding");
}
