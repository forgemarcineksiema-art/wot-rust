use client::{VehicleAssetCatalog, tank_vehicle_render_objects};
use game_core::{TankId, TeamId, VehicleKind};
use net::TankSnapshot;
use vehicle_forge::{BakeProfile, ForgeArtifact, bake_production_vehicle};
use vehicle_geometry::{RunningGearKinematics, SubmeshKind};

/// Render objects for one T-54: hull/turret/gun plus the animated running-gear instances (road
/// wheels and their swing arms both sides, two end wheels per side, and the belt links).
fn t54_object_count() -> usize {
    let kin = RunningGearKinematics::for_vehicle(VehicleKind::T54_1951).expect("T-54 gear");
    3 + kin.wheel_zs.len() * 2 * 2 + 4 + kin.link_count() * 2
}

/// Cached meshes for one blueprint vehicle: hull/turret/gun plus six unit gear meshes
/// (road wheel, swing arm, sprocket, idler, track link, return roller).
const BLUEPRINT_MESH_COUNT: usize = 9;

#[test]
fn vehicle_asset_catalog_uploads_pbr_vehicle_meshes_once() {
    let mut catalog = VehicleAssetCatalog::default();
    let snapshot = snapshot(VehicleKind::T54_1951);

    let objects = tank_vehicle_render_objects(&mut catalog, &snapshot, [0.30, 0.40, 0.28]);
    let uploads = catalog.take_pending_vehicle_meshes();

    assert_eq!(objects.len(), t54_object_count());
    assert_eq!(uploads.len(), BLUEPRINT_MESH_COUNT);
    assert!(uploads.iter().all(|(_, mesh)| mesh.index_count() > 0));
    assert!(uploads.iter().all(|(_, mesh)| mesh.vertices().iter().any(|v| v.material_id <= 4)));
    // Armour submeshes carry the team tint; the running-gear unit meshes (rubber/track metal) do
    // not, so it is enough that some uploaded mesh tints.
    assert!(uploads.iter().any(|(_, mesh)| mesh.vertices().iter().any(|v| v.tint_mask == 1.0)));
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
fn runtime_t54_fallback_uses_the_production_bake() {
    let mut catalog = VehicleAssetCatalog::default();
    let snapshot = snapshot(VehicleKind::T54_1951);
    tank_vehicle_render_objects(&mut catalog, &snapshot, [0.30, 0.40, 0.28]);
    let uploads = catalog.take_pending_vehicle_meshes();
    let production = bake_production_vehicle(VehicleKind::T54_1951, BakeProfile::Lod0)
        .expect("production T-54 bakes");

    for (kind, upload) in [
        (SubmeshKind::Hull, &uploads[0].1),
        (SubmeshKind::Turret, &uploads[1].1),
        (SubmeshKind::Gun, &uploads[2].1),
    ] {
        let expected = production.submesh(kind).expect("production submesh");
        assert_eq!(upload.index_count(), expected.mesh.indices().len(), "{kind:?} index count");
    }
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
    assert!(catalog.load_forge_artifact_dir(&out).expect("load Forge artifact into catalog"));
    let snapshot = snapshot(VehicleKind::T54_1951);

    let objects = tank_vehicle_render_objects(&mut catalog, &snapshot, [0.30, 0.40, 0.28]);
    let uploads = catalog.take_pending_vehicle_meshes();

    assert_eq!(objects.len(), t54_object_count());
    assert_eq!(uploads.len(), BLUEPRINT_MESH_COUNT);
    assert_eq!(catalog.cached_vehicle_count(), 1);
    assert_eq!(catalog.material_count(), 1);
    assert!(catalog.take_pending_vehicle_meshes().is_empty());

    std::fs::remove_dir_all(out).expect("remove client artifact");
}

#[test]
fn loaded_forge_artifact_queues_decoded_material_maps_for_gpu_upload() {
    let artifact =
        ForgeArtifact::bake(VehicleKind::T54_1951, BakeProfile::Lod0).expect("T-54 artifact");
    let out = std::env::temp_dir()
        .join(format!("wot_client_artifact_material_test_{}", std::process::id()));
    if out.exists() {
        std::fs::remove_dir_all(&out).expect("remove stale client artifact");
    }
    artifact.write_to_dir(&out).expect("write Forge artifact");

    let mut catalog = VehicleAssetCatalog::default();
    assert!(catalog.load_forge_artifact_dir(&out).expect("load Forge artifact into catalog"));
    let materials = catalog.take_pending_vehicle_materials();

    assert_eq!(materials.len(), 1, "one vehicle should queue one material upload");
    let (_, families) = &materials[0];
    // One role-aware family per material_id layer; each is a square RGBA8 map decoded tightly.
    assert_eq!(families.families().len(), 5, "five material-role layers");
    for layer in families.families() {
        for map in [layer.albedo(), layer.normal(), layer.ao_roughness()] {
            assert!(map.width() >= 256 && map.width() == map.height());
            assert_eq!(map.rgba().len(), map.width() as usize * map.height() as usize * 4);
        }
        assert!(layer.cavity().is_some(), "each baked family includes a cavity map");
    }
    // A second take is empty â€” uploads are drained, not duplicated.
    assert!(catalog.take_pending_vehicle_materials().is_empty());

    std::fs::remove_dir_all(out).expect("remove client artifact");
}

#[test]
fn vehicle_asset_catalog_loads_forge_lineup_artifact_tree() {
    let root =
        std::env::temp_dir().join(format!("wot_client_artifact_tree_test_{}", std::process::id()));
    if root.exists() {
        std::fs::remove_dir_all(&root).expect("remove stale artifact tree");
    }
    for kind in [VehicleKind::T54_1951, VehicleKind::TigerI] {
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
    // Both upload hull/turret/gun + the six gear unit meshes: the Tiger I animates its running
    // gear from the legacy-track table exactly like the blueprint T-54 does from its blueprint.
    assert_eq!(catalog.take_pending_vehicle_meshes().len(), 2 * BLUEPRINT_MESH_COUNT);

    std::fs::remove_dir_all(root).expect("remove artifact tree");
}

#[test]
fn stale_forge_artifact_does_not_hide_current_runtime_bake() {
    let artifact =
        ForgeArtifact::bake(VehicleKind::T54_1951, BakeProfile::Lod0).expect("T-54 artifact");
    let out =
        std::env::temp_dir().join(format!("wot_client_stale_artifact_test_{}", std::process::id()));
    if out.exists() {
        std::fs::remove_dir_all(&out).expect("remove stale artifact test dir");
    }
    artifact.write_to_dir(&out).expect("write stale artifact");
    poison_source_hash(&out.join("manifest.json"));

    let mut catalog = VehicleAssetCatalog::default();
    let loaded = catalog.load_forge_artifact_dir(&out).expect("load stale artifact");

    assert!(!loaded, "stale source hash must skip the artifact preload");
    assert_eq!(catalog.cached_vehicle_count(), 0);

    let objects = tank_vehicle_render_objects(
        &mut catalog,
        &snapshot(VehicleKind::T54_1951),
        [0.3, 0.4, 0.3],
    );
    assert_eq!(objects.len(), t54_object_count());
    assert_eq!(catalog.take_pending_vehicle_meshes().len(), BLUEPRINT_MESH_COUNT);

    std::fs::remove_dir_all(out).expect("remove stale artifact test dir");
}

#[test]
fn duplicate_vehicle_artifacts_requeue_latest_meshes() {
    let root = std::env::temp_dir()
        .join(format!("wot_client_duplicate_artifact_test_{}", std::process::id()));
    if root.exists() {
        std::fs::remove_dir_all(&root).expect("remove duplicate artifact test dir");
    }
    let older =
        ForgeArtifact::bake(VehicleKind::T54_1951, BakeProfile::Lod2).expect("older T-54 artifact");
    let latest = ForgeArtifact::bake(VehicleKind::T54_1951, BakeProfile::Lod0)
        .expect("latest T-54 artifact");
    older.write_to_dir(&root.join("a_old_t54")).expect("write older artifact");
    latest.write_to_dir(&root.join("z_latest_t54")).expect("write latest artifact");

    let latest_hull_indices = latest
        .baked_vehicle()
        .expect("decode latest artifact")
        .submesh(SubmeshKind::Hull)
        .expect("latest hull")
        .mesh
        .indices()
        .len();

    let mut catalog = VehicleAssetCatalog::default();
    let loaded = catalog.load_forge_artifact_tree(&root).expect("load duplicate artifact tree");
    let uploads = catalog.take_pending_vehicle_meshes();
    let hull_uploads: Vec<_> = uploads.iter().filter(|(handle, _)| handle.0 == 0).collect();

    assert_eq!(loaded, 2);
    assert_eq!(catalog.cached_vehicle_count(), 1);
    assert_eq!(hull_uploads.len(), 2, "latest duplicate must upload a replacement hull");
    assert_eq!(hull_uploads[1].1.index_count(), latest_hull_indices);

    std::fs::remove_dir_all(root).expect("remove duplicate artifact test dir");
}

fn poison_source_hash(manifest_path: &std::path::Path) {
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(manifest_path).expect("read manifest"))
            .expect("parse manifest");
    manifest["source_hash"] = serde_json::json!(1_u64);
    std::fs::write(
        manifest_path,
        serde_json::to_vec_pretty(&manifest).expect("serialize manifest"),
    )
    .expect("write manifest");
}

fn snapshot(vehicle: VehicleKind) -> TankSnapshot {
    TankSnapshot {
        tank_id: TankId(11),
        team: TeamId(1),
        vehicle,
        position: [0.0, 0.0, 0.0],
        yaw_rad: 0.0,
        hull_pitch_rad: 0.0,
        hull_roll_rad: 0.0,
        turret_yaw_rad: 0.0,
        turret_yaw_velocity_rad_s: 0.0,
        gun_pitch_rad: 0.0,
        hit_points: 900,
        reload_remaining_s: 0.0,
        aim_dispersion_mrad: vehicle.spec().gun.dispersion_mrad,
        module_hit_points: vehicle.spec().module_health.hit_points_by_slot(),
        destroyed_modules_mask: 0,
        track_damage_mask: 0,
        ammo_counts: game_core::AmmoLoadout::default().counts,
        selected_ammo: 0,
        spotted_by_teams_mask: 0,
    }
}
