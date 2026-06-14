use client::{VehicleAssetCatalog, tank_vehicle_render_objects};
use game_core::{TankId, TeamId, VehicleKind};
use net::TankSnapshot;
use vehicle_forge::{BakeProfile, ForgeArtifact};

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

#[test]
fn vehicle_asset_catalog_can_seed_runtime_meshes_from_forge_artifact_folder() {
    let artifact =
        ForgeArtifact::bake(VehicleKind::T54_1951, BakeProfile::Lod0).expect("T-54 artifact");
    let out = std::env::temp_dir()
        .join(format!("wot_client_artifact_catalog_test_{}", std::process::id()));
    if out.exists() {
        std::fs::remove_dir_all(&out).expect("remove stale client artifact");
    }
    artifact.write_to_dir(&out).expect("write Forge artifact");

    let mut catalog = VehicleAssetCatalog::default();
    catalog.load_forge_artifact_dir(&out).expect("load Forge artifact into catalog");
    let snapshot = snapshot(VehicleKind::T54_1951);

    let objects = tank_vehicle_render_objects(&mut catalog, &snapshot, [0.30, 0.40, 0.28]);
    let uploads = catalog.take_pending_vehicle_meshes();

    assert_eq!(objects.len(), 3);
    assert_eq!(uploads.len(), 3);
    assert_eq!(catalog.cached_vehicle_count(), 1);
    assert_eq!(catalog.material_count(), 1);
    assert!(catalog.take_pending_vehicle_meshes().is_empty());

    std::fs::remove_dir_all(out).expect("remove client artifact");
}

#[test]
fn vehicle_asset_catalog_loads_forge_lineup_artifact_tree() {
    let root =
        std::env::temp_dir().join(format!("wot_client_artifact_tree_test_{}", std::process::id()));
    if root.exists() {
        std::fs::remove_dir_all(&root).expect("remove stale artifact tree");
    }
    for kind in [VehicleKind::T54_1951, VehicleKind::T55A] {
        let artifact = ForgeArtifact::bake(kind, BakeProfile::Lod0).expect("Forge artifact");
        artifact
            .write_to_dir(&root.join(artifact.manifest().vehicle_slug()))
            .expect("write artifact tree entry");
    }

    let mut catalog = VehicleAssetCatalog::default();
    let loaded = catalog.load_forge_artifact_tree(&root).expect("load artifact tree");

    assert_eq!(loaded, 2);
    assert_eq!(catalog.cached_vehicle_count(), 2);
    assert_eq!(catalog.material_count(), 2);
    assert_eq!(catalog.take_pending_vehicle_meshes().len(), 6);

    std::fs::remove_dir_all(root).expect("remove artifact tree");
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
