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
    render_state.advance(0.025, 0.05);
    assert!((render_state.interpolation_alpha() - 0.5).abs() < 1.0e-4);
    let tank = &render_state.interpolated_tanks()[0];
    assert!((tank.position[0] - 4.5).abs() < 1.0e-3, "interpolated x = {}", tank.position[0]);
}

#[test]
fn render_state_ignores_duplicate_or_stale_snapshots() {
    let mut render_state = InterpolatedBattleState::default();

    render_state.accept_authoritative_snapshot(snapshot_at(3, 0.0));
    render_state.accept_authoritative_snapshot(snapshot_at(6, 9.0));
    render_state.advance(0.025, 0.05);
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
    render_state.advance(0.025, 0.05);

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
    render_state.advance(0.025, 0.05);

    let shell = &render_state.interpolated_shells(0.05)[0];
    assert!((shell.position[0] - 1.25).abs() < 1.0e-4, "extrapolated x = {}", shell.position[0]);
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
    render_state.advance(0.025, 0.05);

    let tank = &render_state.interpolated_tanks()[0];
    assert_eq!(tank.destroyed_modules_mask, 1 << 3);
}

fn snapshot_at(server_tick: u64, x: f32) -> Snapshot {
    Snapshot {
        server_tick,
        tanks: vec![TankSnapshot {
            tank_id: TankId(1),
            team: game_core::TeamId(1),
            vehicle: game_core::VehicleKind::PrototypeMedium,
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
            module_hit_points: game_core::VehicleKind::PrototypeMedium
                .spec()
                .module_health
                .hit_points_by_slot(),
            destroyed_modules_mask: 0,
            track_damage_mask: 0,
            ammo_counts: game_core::AmmoLoadout::default().counts,
            selected_ammo: 0,
        }],
        shells: Vec::new(),
        damage_events: Vec::new(),
        shell_impacts: Vec::new(),
    }
}

fn snapshot_with_yaw(server_tick: u64, yaw_rad: f32) -> Snapshot {
    let mut snapshot = snapshot_at(server_tick, 0.0);
    snapshot.tanks[0].yaw_rad = yaw_rad;
    snapshot
}

fn snapshot_with_shell(server_tick: u64, position: [f32; 3], velocity_mps: [f32; 3]) -> Snapshot {
    let mut snapshot = snapshot_at(server_tick, 0.0);
    snapshot.shells.push(ShellSnapshot { owner: TankId(1), position, velocity_mps });
    snapshot
}
