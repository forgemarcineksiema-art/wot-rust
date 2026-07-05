use client::tank_scene_mesh;
use game_core::{TankId, VehicleKind};
use net::TankSnapshot;

#[test]
fn t55a_client_mesh_uses_rich_procedural_geometry() {
    let snapshot = TankSnapshot {
        tank_id: TankId(1),
        team: game_core::TeamId(1),
        vehicle: VehicleKind::T55A,
        position: [0.0, 0.0, 0.0],
        yaw_rad: 0.0,
        hull_pitch_rad: 0.0,
        hull_roll_rad: 0.0,
        turret_yaw_rad: 0.35,
        turret_yaw_velocity_rad_s: 0.0,
        gun_pitch_rad: 0.08,
        hit_points: 1000,
        reload_remaining_s: 0.0,
        aim_dispersion_mrad: VehicleKind::T55A.spec().gun.dispersion_mrad,
        module_hit_points: VehicleKind::T55A.spec().module_health.hit_points_by_slot(),
        destroyed_modules_mask: 0,
        track_damage_mask: 0,
        ammo_counts: game_core::AmmoLoadout::default().counts,
        selected_ammo: 0,
        spotted_by_teams_mask: 0,
    };

    let (vertices, indices) = tank_scene_mesh(&snapshot);

    assert!(vertices.iter().all(|vertex| vertex.position.iter().all(|value| value.is_finite())));
    assert!(vertices.iter().all(|vertex| vertex.normal.iter().all(|value| value.is_finite())));
    assert!(
        indices.len() / 3 >= 170,
        "T-55A should render through richer baked procedural geometry, not the old box stack"
    );
}
