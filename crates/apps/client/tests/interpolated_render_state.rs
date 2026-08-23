use client::InterpolatedBattleState;
use game_core::TankId;
use net::{ShellSnapshot, Snapshot, TankSnapshot};
use std::f32::consts::PI;

#[test]
fn render_state_tracks_authoritative_snapshots_for_interpolation() {
    let mut render_state = InterpolatedBattleState::default();

    render_state.accept_authoritative_snapshot(snapshot_at(3, 0.0));
    render_state.accept_authoritative_snapshot(snapshot_at(6, 9.0));

    assert_eq!(render_state.previous_snapshot().expect("previous snapshot").server_tick, 3);
    assert_eq!(render_state.latest_snapshot().expect("latest snapshot").server_tick, 6);
    assert_eq!(render_state.interpolation_alpha(), 0.0);

    // Halfway through the snapshot interval, the tank renders halfway between snapshots.
    render_state.set_interpolation_alpha(0.5);
    assert!((render_state.interpolation_alpha() - 0.5).abs() < 1.0e-4);
    let tank = &render_state.interpolated_tanks()[0];
    assert!((tank.position[0] - 4.5).abs() < 1.0e-3, "interpolated x = {}", tank.position[0]);
}

#[test]
fn render_state_ignores_duplicate_or_stale_snapshots() {
    let mut render_state = InterpolatedBattleState::default();

    render_state.accept_authoritative_snapshot(snapshot_at(3, 0.0));
    render_state.accept_authoritative_snapshot(snapshot_at(6, 9.0));
    render_state.set_interpolation_alpha(0.5);
    render_state.accept_authoritative_snapshot(snapshot_at(5, 30.0));
    render_state.accept_authoritative_snapshot(snapshot_at(6, 90.0));

    assert_eq!(render_state.previous_snapshot().expect("previous snapshot").server_tick, 3);
    assert_eq!(render_state.latest_snapshot().expect("latest snapshot").server_tick, 6);
    assert!((render_state.interpolation_alpha() - 0.5).abs() < 1.0e-4);
    let tank = &render_state.interpolated_tanks()[0];
    assert!((tank.position[0] - 4.5).abs() < 1.0e-3, "interpolated x = {}", tank.position[0]);
}

#[test]
fn render_state_interpolates_yaw_across_wrap_boundary() {
    let mut render_state = InterpolatedBattleState::default();

    render_state.accept_authoritative_snapshot(snapshot_with_yaw(3, PI - 0.1));
    render_state.accept_authoritative_snapshot(snapshot_with_yaw(6, -PI + 0.1));
    render_state.set_interpolation_alpha(0.5);

    let tank = &render_state.interpolated_tanks()[0];
    assert!((tank.yaw_rad.abs() - PI).abs() < 1.0e-4, "interpolated yaw = {}", tank.yaw_rad);
}

#[test]
fn render_state_extrapolates_shells_from_latest_velocity() {
    let mut render_state = InterpolatedBattleState::default();

    render_state.accept_authoritative_snapshot(snapshot_with_shell(
        3,
        [1.0, 0.0, 0.0],
        [10.0, 0.0, 0.0],
    ));
    render_state.set_interpolation_alpha(0.5);

    let shell = &render_state.interpolated_shells(0.05)[0];
    assert!((shell.position[0] - 1.25).abs() < 1.0e-4, "extrapolated x = {}", shell.position[0]);
    assert!(shell.position[1] < 0.0, "visual flight must fall under shared gravity");
    assert!(shell.velocity_mps[1] < 0.0, "replicated velocity advances with the arc");
    assert!((shell.age_seconds - 0.025).abs() < 1.0e-6, "visual age advances with flight");
}

#[test]
fn render_state_uses_latest_module_status_without_interpolation() {
    let mut render_state = InterpolatedBattleState::default();
    let mut previous = snapshot_at(3, 0.0);
    previous.tanks[0].destroyed_modules_mask = 1 << 0;
    let mut latest = snapshot_at(6, 9.0);
    latest.tanks[0].destroyed_modules_mask = 1 << 3;

    render_state.accept_authoritative_snapshot(previous);
    render_state.accept_authoritative_snapshot(latest);
    render_state.set_interpolation_alpha(0.5);

    let tank = &render_state.interpolated_tanks()[0];
    assert_eq!(tank.destroyed_modules_mask, 1 << 3);
}

#[test]
fn remote_motion_is_derived_from_the_snapshot_pair_not_the_render_clock() {
    let mut render_state = InterpolatedBattleState::default();

    // Facing +X (yaw PI/2 with the sin/cos heading), advancing 0.5 m per 3-tick window = 10 m/s.
    let yaw = PI / 2.0;
    render_state.accept_authoritative_snapshot(snapshot_moving(3, 0.0, yaw));
    render_state.accept_authoritative_snapshot(snapshot_moving(6, 0.5, yaw));

    let motion = render_state.motion_of(TankId(1));
    assert!(
        (motion.forward_speed_mps - 10.0).abs() < 1.0e-3,
        "forward speed from the pair, got {}",
        motion.forward_speed_mps
    );
    // First pair: no previous speed to difference against, so no fake launch cue.
    assert_eq!(motion.accel_long_mps2, 0.0);

    // The next window covers 1.0 m = 20 m/s: acceleration is (20-10)/0.05 s = 200, tick-exact.
    render_state.accept_authoritative_snapshot(snapshot_moving(9, 1.5, yaw));
    let motion = render_state.motion_of(TankId(1));
    assert!((motion.forward_speed_mps - 20.0).abs() < 1.0e-3);
    assert!(
        (motion.accel_long_mps2 - 200.0).abs() < 0.1,
        "accel from consecutive pair speeds, got {}",
        motion.accel_long_mps2
    );
    assert_eq!(render_state.snapshot_interval_ticks(), Some(3));
}

#[test]
fn a_turning_tank_reports_its_yaw_rate_from_the_pair() {
    let mut render_state = InterpolatedBattleState::default();

    render_state.accept_authoritative_snapshot(snapshot_with_yaw(3, 0.0));
    render_state.accept_authoritative_snapshot(snapshot_with_yaw(6, 0.05));

    let motion = render_state.motion_of(TankId(1));
    assert!(
        (motion.yaw_rate_rad_s - 1.0).abs() < 1.0e-3,
        "0.05 rad over 0.05 s is 1 rad/s, got {}",
        motion.yaw_rate_rad_s
    );
}

#[test]
fn a_tank_new_to_the_pair_has_zero_motion_until_two_snapshots_see_it() {
    let mut render_state = InterpolatedBattleState::default();
    render_state.accept_authoritative_snapshot(snapshot_at(3, 0.0));

    let motion = render_state.motion_of(TankId(1));
    assert_eq!(motion.forward_speed_mps, 0.0);
    assert_eq!(motion.yaw_rate_rad_s, 0.0);
}

fn snapshot_moving(server_tick: u64, x: f32, yaw_rad: f32) -> Snapshot {
    let mut snapshot = snapshot_at(server_tick, x);
    snapshot.tanks[0].yaw_rad = yaw_rad;
    snapshot
}

fn snapshot_at(server_tick: u64, x: f32) -> Snapshot {
    Snapshot {
        server_tick,
        tanks: vec![TankSnapshot {
            tank_id: TankId(1),
            team: game_core::TeamId(1),
            vehicle: game_core::VehicleKind::T54_1951,
            position: [x, 0.0, 0.0],
            yaw_rad: 0.0,
            hull_pitch_rad: 0.0,
            hull_roll_rad: 0.0,
            turret_yaw_rad: 0.0,
            turret_yaw_velocity_rad_s: 0.0,
            gun_pitch_rad: 0.0,
            hit_points: 1000,
            reload_remaining_s: 0.0,
            aim_dispersion_mrad: 2.5,
            module_hit_points: game_core::VehicleKind::T54_1951
                .spec()
                .module_health
                .hit_points_by_slot(),
            destroyed_modules_mask: 0,
            track_damage_mask: 0,
            track_hp: [game_core::TRACK_HP_MAX; 2],
            ammo_counts: game_core::AmmoLoadout::default().counts,
            selected_ammo: 0,
            spotted_by_teams_mask: 0,
            armor_breaches: Default::default(),
            track_break_t: [None, None],
            engine_fire: false,
            fuel_fire: false,
            rack_fire_remaining_s: None,
            crew_unconscious_mask: 0,
            crew_weakened_mask: 0,
            crew_down_remaining_s: Default::default(),
        }],
        shells: Vec::new(),
        damage_events: Vec::new(),
        shell_impacts: Vec::new(),
        detached_turrets: Vec::new(),
        cover_states: Vec::new(),
        craters: Vec::new(),
        cover_scars: Vec::new(),
        shots_fired: Vec::new(),
    }
}

fn snapshot_with_yaw(server_tick: u64, yaw_rad: f32) -> Snapshot {
    let mut snapshot = snapshot_at(server_tick, 0.0);
    snapshot.tanks[0].yaw_rad = yaw_rad;
    snapshot
}

fn snapshot_with_shell(server_tick: u64, position: [f32; 3], velocity_mps: [f32; 3]) -> Snapshot {
    let mut snapshot = snapshot_at(server_tick, 0.0);
    snapshot.shells.push(ShellSnapshot {
        owner: Some(TankId(1)),
        position,
        velocity_mps,
        caliber_mm: 100.0,
        ..Default::default()
    });
    snapshot
}
