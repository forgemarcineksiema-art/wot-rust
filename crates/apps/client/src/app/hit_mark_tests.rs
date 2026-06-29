use game_core::{DamageCause, DamageEvent, TankId};
use net::{Snapshot, TankSnapshot};

use super::ClientApp;

#[test]
fn shell_marker_mesh_includes_world_space_hit_marks_from_damage_events() {
    let mut app = ClientApp::new();
    let player = app.player_tank;
    let hit = glam::Vec3::new(12.0, 1.4, 18.0);
    let mut snapshot = snapshot_at(player, 3);
    snapshot.damage_events.push(DamageEvent {
        source: player,
        target: TankId(99),
        hit_position: hit,
        damage_hp: 120,
        penetrated: true,
        ..Default::default()
    });
    app.accept_and_sync(snapshot);

    let (vertices, indices) = app.shell_marker_mesh();

    assert!(!indices.is_empty(), "damage event should create a world-space hit mark mesh");
    assert!(
        vertices.iter().any(|vertex| {
            let p = glam::Vec3::from_array(vertex.position);
            (p - hit).length() < 0.35
        }),
        "hit mark vertices should sit around the authoritative hit position"
    );
}

#[test]
fn shell_marker_mesh_ignores_non_shell_damage_events() {
    let mut app = ClientApp::new();
    let player = app.player_tank;
    let mut snapshot = snapshot_at(player, 3);
    snapshot.damage_events.push(DamageEvent {
        source: player,
        target: TankId(99),
        hit_position: glam::Vec3::new(12.0, 1.4, 18.0),
        damage_hp: 90,
        penetrated: true,
        cause: DamageCause::Ram,
        ..Default::default()
    });
    app.accept_and_sync(snapshot);

    let (_, indices) = app.shell_marker_mesh();

    assert!(indices.is_empty(), "only projectile hits should create world-space shell marks");
}

fn snapshot_at(tank_id: TankId, server_tick: u64) -> Snapshot {
    let vehicle = game_core::VehicleKind::PrototypeMedium;
    let spec = vehicle.spec();
    Snapshot {
        server_tick,
        tanks: vec![TankSnapshot {
            tank_id,
            team: game_core::TeamId(1),
            vehicle,
            position: [0.0, 0.0, 0.0],
            yaw_rad: 0.0,
            turret_yaw_rad: 0.0,
            turret_yaw_velocity_rad_s: 0.0,
            gun_pitch_rad: 0.0,
            hit_points: spec.hit_points,
            reload_remaining_s: 0.0,
            aim_dispersion_mrad: spec.gun.dispersion_mrad,
            module_hit_points: spec.module_health.hit_points_by_slot(),
            destroyed_modules_mask: 0,
            track_damage_mask: 0,
        }],
        shells: Vec::new(),
        damage_events: Vec::new(),
        shell_impacts: Vec::new(),
    }
}
