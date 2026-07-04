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

#[test]
fn replicated_shell_deaths_and_armor_hits_burst_into_particles() {
    use game_core::{DamageCause, DamageEvent, ImpactSurface, ShellImpact, TankId};

    let mut app = battle_ready_app();
    assert_eq!(app.fx.live_particles(), 0);

    // One shell swallowed by terrain, one penetrating hit on a tank — both replicated in the
    // same snapshot, both must burst where the server said they happened.
    let mut snapshot =
        app.render_state.latest_snapshot().cloned().expect("battle-ready app has a snapshot");
    snapshot.server_tick += 1;
    snapshot.shell_impacts.push(ShellImpact {
        owner: app.player_tank,
        position: glam::Vec3::new(30.0, 0.5, 40.0),
        surface: ImpactSurface::Terrain,
    });
    snapshot.damage_events.push(DamageEvent {
        source: app.player_tank,
        target: TankId(99),
        hit_position: glam::Vec3::new(50.0, 1.6, 60.0),
        damage_hp: 180,
        penetrated: true,
        cause: DamageCause::Shell,
        ..Default::default()
    });
    app.accept_and_sync(snapshot);

    // 18 terrain particles + 10 sparks + 8 penetration signature.
    assert!(
        app.fx.live_particles() >= 30,
        "both bursts land in the pool, got {}",
        app.fx.live_particles()
    );

    // Ramming damage is not a shell strike and must not spark.
    let mut app = battle_ready_app();
    let mut snapshot = app.render_state.latest_snapshot().cloned().expect("snapshot");
    snapshot.server_tick += 1;
    snapshot.damage_events.push(DamageEvent {
        source: app.player_tank,
        target: TankId(99),
        hit_position: glam::Vec3::ZERO,
        damage_hp: 40,
        penetrated: true,
        cause: DamageCause::Ram,
        ..Default::default()
    });
    app.accept_and_sync(snapshot);
    assert_eq!(app.fx.live_particles(), 0, "ram damage draws no shell burst");
}
