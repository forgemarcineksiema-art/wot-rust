use client::{VehicleAssetCatalog, tank_vehicle_render_objects};
use game_core::{TankId, TeamId, VehicleKind};
use net::TankSnapshot;

#[test]
fn vehicle_asset_catalog_uploads_pbr_vehicle_meshes_once() {
    let mut catalog = VehicleAssetCatalog::default();
    let snapshot = snapshot(VehicleKind::T55A);

    let objects = tank_vehicle_render_objects(&mut catalog, &snapshot, [0.30, 0.40, 0.28]);
    let uploads = catalog.take_pending_vehicle_meshes();

    assert_eq!(objects.len(), 3);
    assert_eq!(uploads.len(), 3);
    assert!(uploads.iter().all(|(_, mesh)| mesh.index_count() > 0));
    assert!(uploads.iter().all(|(_, mesh)| mesh.vertices().iter().any(|v| v.material_id <= 4)));
    assert!(uploads.iter().all(|(_, mesh)| mesh.vertices().iter().any(|v| v.tint_mask == 1.0)));
    assert!(uploads.iter().all(|(_, mesh)| {
        mesh.vertices().iter().all(|v| v.tangent.iter().all(|c| c.is_finite()))
    }));

    let material = catalog.vehicle_material(objects[0].material).expect("vehicle material");
    assert_eq!(material.albedo_texture(), "albedo.png");
    assert_eq!(material.normal_texture(), "normal.png");
    assert_eq!(material.ao_roughness_texture(), "ao_roughness.png");
    assert_eq!(material.cavity_texture(), Some("cavity.png"));

    let second = tank_vehicle_render_objects(&mut catalog, &snapshot, [0.46, 0.29, 0.25]);
    assert!(catalog.take_pending_vehicle_meshes().is_empty());
    assert_eq!(objects[0].mesh, second[0].mesh);
    assert_ne!(objects[0].tint, second[0].tint);
}

fn snapshot(vehicle: VehicleKind) -> TankSnapshot {
    TankSnapshot {
        tank_id: TankId(11),
        team: TeamId(1),
        vehicle,
        position: [0.0, 0.0, 0.0],
        yaw_rad: 0.0,
        turret_yaw_rad: 0.0,
        turret_yaw_velocity_rad_s: 0.0,
        gun_pitch_rad: 0.0,
        hit_points: 900,
        reload_remaining_s: 0.0,
        aim_dispersion_mrad: vehicle.spec().gun.dispersion_mrad,
        module_hit_points: vehicle.spec().module_health.hit_points_by_slot(),
        destroyed_modules_mask: 0,
    }
}
