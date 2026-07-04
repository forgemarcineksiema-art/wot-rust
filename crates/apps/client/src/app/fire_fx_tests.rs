//! Locks the replicated-shot fan-out: firing the player's gun must light the muzzle FX pool,
//! throw the barrel into recoil in the presentation world, and do neither on idle ticks.

use super::ClientApp;

/// Run enough fixed ticks to cross at least one 20 Hz snapshot boundary at the 60 Hz sim rate.
const TICKS_PAST_SNAPSHOT: u32 = 6;

fn battle_ready_app() -> ClientApp {
    let mut app = ClientApp::new();
    app.confirm_garage_selection();
    // Seed prediction + land the first snapshots, then project the presentation entities the
    // recoil rides on (the render loop does this every frame; tests do it explicitly).
    app.run_fixed_ticks(TICKS_PAST_SNAPSHOT);
    app.presentation.advance_time(1.0 / 60.0);
    app.project_render_tanks(1.0);
    app
}

#[test]
fn a_player_shot_spawns_muzzle_fx_and_barrel_recoil() {
    let mut app = battle_ready_app();
    assert_eq!(app.fx.live_particles(), 0, "no FX before the first shot");

    app.input.fire_pending = true;
    app.run_fixed_ticks(TICKS_PAST_SNAPSHOT);

    assert!(
        app.fx.live_particles() >= 10,
        "the muzzle blast fills the pool, got {}",
        app.fx.live_particles()
    );

    // The recoil impulse landed in the presentation world: stepping it a frame shows the stroke.
    app.presentation.advance_time(1.0 / 60.0);
    let tanks = app.project_render_tanks(1.0);
    let player = tanks.iter().find(|tank| tank.id == app.player_tank).expect("player tank");
    assert!(player.gun_recoil_m > 0.01, "barrel in recoil, got {} m", player.gun_recoil_m);
}

#[test]
fn idle_ticks_fire_no_fx_and_reload_blocks_a_second_blast() {
    let mut app = battle_ready_app();

    app.run_fixed_ticks(TICKS_PAST_SNAPSHOT * 2);
    assert_eq!(app.fx.live_particles(), 0, "no shot, no FX");

    app.input.fire_pending = true;
    app.run_fixed_ticks(TICKS_PAST_SNAPSHOT);
    let after_first = app.fx.live_particles();
    assert!(after_first > 0);

    // A second trigger pull during reload must not double the muzzle FX.
    app.input.fire_pending = true;
    app.run_fixed_ticks(1);
    assert_eq!(app.fx.live_particles(), after_first, "reload gate holds the second blast");
}
