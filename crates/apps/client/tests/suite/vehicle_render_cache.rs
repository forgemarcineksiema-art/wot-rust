use client::{VehicleMeshCatalog, tank_render_objects};
use game_core::{TankId, VehicleKind};
use glam::{Mat4, Vec3};
use net::TankSnapshot;

#[test]
fn vehicle_mesh_catalog_bakes_and_registers_material_once_per_vehicle() {
    let mut catalog = VehicleMeshCatalog::default();
    let snapshot = snapshot(VehicleKind::T54_1951, 0.0);

    let _ = tank_render_objects(&mut catalog, &snapshot, [0.30, 0.40, 0.28]);
    let _ = tank_render_objects(&mut catalog, &snapshot, [0.46, 0.29, 0.25]);

    assert_eq!(catalog.cached_vehicle_count(), 1);
    assert_eq!(catalog.material_count(), 1);
}

#[test]
fn jagdtiger_render_objects_ignore_stray_turret_yaw() {
    let mut catalog = VehicleMeshCatalog::default();
    let straight = tank_render_objects(
        &mut catalog,
        &snapshot(VehicleKind::Jagdtiger, 0.0),
        [0.30, 0.40, 0.28],
    );
    let stray = tank_render_objects(
        &mut catalog,
        &snapshot(VehicleKind::Jagdtiger, 0.8),
        [0.30, 0.40, 0.28],
    );

    let straight_turret = Mat4::from_cols_array_2d(&straight[1].transform);
    let stray_turret = Mat4::from_cols_array_2d(&stray[1].transform);
    let straight_gun_pos = Mat4::from_cols_array_2d(&straight[2].transform).w_axis.truncate();
    let stray_gun_pos = Mat4::from_cols_array_2d(&stray[2].transform).w_axis.truncate();

    assert!((straight_turret.w_axis.truncate() - stray_turret.w_axis.truncate()).length() < 1.0e-5);
    assert!((straight_gun_pos - stray_gun_pos).length() < 1.0e-5);
    assert!(
        (straight_turret.transform_vector3(Vec3::Z) - stray_turret.transform_vector3(Vec3::Z))
            .length()
            < 1.0e-5
    );
}

fn snapshot(vehicle: VehicleKind, turret_yaw_rad: f32) -> TankSnapshot {
    TankSnapshot {
        tank_id: TankId(8),
        team: game_core::TeamId(1),
        vehicle,
        position: [0.0, 0.0, 0.0],
        yaw_rad: 0.4,
        hull_pitch_rad: 0.0,
        hull_roll_rad: 0.0,
        turret_yaw_rad,
        turret_yaw_velocity_rad_s: 0.0,
        gun_pitch_rad: 0.0,
        hit_points: 1500,
        reload_remaining_s: 0.0,
        aim_dispersion_mrad: vehicle.spec().gun.dispersion_mrad,
        module_hit_points: vehicle.spec().module_health.hit_points_by_slot(),
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
        hull_pitch_velocity_rad_s: 0.0,
        hull_roll_velocity_rad_s: 0.0,
    }
}
