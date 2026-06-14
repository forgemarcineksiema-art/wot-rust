use game_core::VehicleKind;
use vehicle_forge::{BakeProfile, ForgeArtifact, ReviewCamera, ReviewCameraSet};

#[test]
fn forge_artifact_manifest_names_the_baked_vehicle_profile_and_sources() {
    let artifact =
        ForgeArtifact::bake(VehicleKind::T54_1951, BakeProfile::Lod0).expect("T-54 Forge artifact");
    let manifest = artifact.manifest();

    assert_eq!(manifest.vehicle_slug(), "t54-1951");
    assert_eq!(manifest.profile(), BakeProfile::Lod0);
    assert_eq!(manifest.source_family_slug(), Some("t54_t55"));
    assert!(manifest.source_hash() != 0, "bake hash must identify the generated source");
    assert!(manifest.mesh_bytes() > 0, "artifact must account for mesh payload bytes");
    assert!(manifest.submeshes().iter().any(|submesh| submesh.kind() == "Hull"));
    assert!(manifest.submeshes().iter().any(|submesh| submesh.kind() == "Turret"));
    assert!(manifest.submeshes().iter().any(|submesh| submesh.kind() == "Gun"));
    let texture_maps: Vec<&str> = manifest.texture_maps().iter().map(|map| map.file()).collect();
    assert_eq!(texture_maps, vec!["albedo.png", "normal.png", "ao_roughness.png", "cavity.png"]);
}

#[test]
fn every_migrated_vehicle_bakes_a_full_artifact_at_every_lod() {
    for kind in [
        VehicleKind::T54_1951,
        VehicleKind::T55A,
        VehicleKind::TigerI,
        VehicleKind::TigerII,
        VehicleKind::Jagdtiger,
        VehicleKind::PantherII,
    ] {
        for profile in [BakeProfile::Lod0, BakeProfile::Lod1, BakeProfile::Lod2] {
            let artifact = ForgeArtifact::bake(kind, profile)
                .unwrap_or_else(|e| panic!("{kind:?} {profile:?} should bake: {e}"));
            let manifest = artifact.manifest();
            assert_eq!(manifest.profile(), profile);
            assert!(manifest.mesh_bytes() > 0, "{kind:?} {profile:?} empty mesh");
            // Hull, turret/casemate, and gun all survive into the artifact at every LOD.
            for sub in ["Hull", "Turret", "Gun"] {
                assert!(
                    manifest.submeshes().iter().any(|s| s.kind() == sub && s.triangles() > 0),
                    "{kind:?} {profile:?} missing {sub}"
                );
            }
            // The full baked material + review set is present regardless of vehicle.
            assert_eq!(manifest.texture_maps().len(), 4, "{kind:?} material set incomplete");
            assert_eq!(
                manifest.review_cameras().cameras().len(),
                6,
                "{kind:?} review set incomplete"
            );
        }
    }
}

#[test]
fn bake_profile_selects_progressively_lighter_geometry() {
    let total_tris = |profile| {
        let artifact = ForgeArtifact::bake(VehicleKind::T54_1951, profile).expect("T-54 artifact");
        artifact.manifest().submeshes().iter().map(|s| s.triangles()).sum::<usize>()
    };
    let lod0 = total_tris(BakeProfile::Lod0);
    let lod1 = total_tris(BakeProfile::Lod1);
    let lod2 = total_tris(BakeProfile::Lod2);

    assert!(
        lod0 > lod1 && lod1 > lod2,
        "bake profiles must produce a real LOD ladder: {lod0}/{lod1}/{lod2}"
    );

    // The reduced profile must also shrink the serialized payload, proving the LOD is baked, not
    // just labelled.
    let lod0_bytes =
        ForgeArtifact::bake(VehicleKind::T54_1951, BakeProfile::Lod0).unwrap().mesh_payload().len();
    let lod2_bytes =
        ForgeArtifact::bake(VehicleKind::T54_1951, BakeProfile::Lod2).unwrap().mesh_payload().len();
    assert!(
        lod2_bytes < lod0_bytes,
        "LOD2 payload {lod2_bytes} must be smaller than LOD0 {lod0_bytes}"
    );
}

#[test]
fn review_camera_set_contains_required_forge_regression_views() {
    let cameras = ReviewCameraSet::standard_vehicle_review();
    let names: Vec<ReviewCamera> = cameras.cameras().iter().map(|camera| camera.kind()).collect();

    assert_eq!(
        names,
        vec![
            ReviewCamera::Front,
            ReviewCamera::Rear,
            ReviewCamera::LeftProfile,
            ReviewCamera::RightProfile,
            ReviewCamera::Top,
            ReviewCamera::BattleOblique,
        ]
    );
}

#[test]
fn forge_artifact_writes_manifest_mesh_payload_report_and_review_folder() {
    let artifact =
        ForgeArtifact::bake(VehicleKind::T54_1951, BakeProfile::Lod0).expect("T-54 Forge artifact");
    let out = std::env::temp_dir().join(format!("wot_forge_artifact_test_{}", std::process::id()));
    if out.exists() {
        std::fs::remove_dir_all(&out).expect("remove stale test artifact");
    }

    artifact.write_to_dir(&out).expect("write Forge artifact");

    assert!(out.join("manifest.json").is_file());
    assert!(out.join("meshes.bin").is_file());
    assert_png(&out.join("albedo.png"));
    assert_png(&out.join("normal.png"));
    assert_png(&out.join("ao_roughness.png"));
    assert_png(&out.join("cavity.png"));
    for file in [
        "front.png",
        "rear.png",
        "left_profile.png",
        "right_profile.png",
        "top.png",
        "battle_oblique.png",
    ] {
        assert_nonblank_png(&out.join("review").join(file));
    }
    assert!(out.join("report.md").is_file());
    assert!(out.join("review").is_dir());
    assert!(
        std::fs::metadata(out.join("meshes.bin")).expect("mesh payload metadata").len() > 0,
        "mesh payload must not be empty"
    );
    let report = std::fs::read_to_string(out.join("report.md")).expect("report markdown");
    assert!(report.contains("T-54/T-55 Forge reference report"));
    assert!(report.contains("HullLengthToWidth"));

    std::fs::remove_dir_all(out).expect("remove test artifact");
}

#[test]
fn written_forge_artifact_loads_back_into_mount_aware_baked_vehicle() {
    let artifact =
        ForgeArtifact::bake(VehicleKind::T54_1951, BakeProfile::Lod0).expect("T-54 Forge artifact");
    let expected_vehicle = artifact.baked_vehicle().expect("decoded baked vehicle");
    let out =
        std::env::temp_dir().join(format!("wot_forge_artifact_load_test_{}", std::process::id()));
    if out.exists() {
        std::fs::remove_dir_all(&out).expect("remove stale test artifact");
    }

    artifact.write_to_dir(&out).expect("write Forge artifact");
    let loaded = ForgeArtifact::read_from_dir(&out).expect("load Forge artifact");
    let loaded_vehicle = loaded.baked_vehicle().expect("decoded loaded baked vehicle");

    assert_eq!(loaded.manifest(), artifact.manifest());
    assert_eq!(loaded.mesh_payload(), artifact.mesh_payload());
    assert_eq!(loaded_vehicle.kind(), VehicleKind::T54_1951);
    assert_eq!(loaded_vehicle.submeshes().len(), expected_vehicle.submeshes().len());
    assert_eq!(loaded_vehicle.mounts(), expected_vehicle.mounts());

    std::fs::remove_dir_all(out).expect("remove test artifact");
}

fn assert_png(path: &std::path::Path) {
    let bytes = std::fs::read(path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
    assert!(bytes.len() > 32, "{} must not be an empty placeholder", path.display());
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n", "{} must be a PNG file", path.display());
}

fn assert_nonblank_png(path: &std::path::Path) {
    assert_png(path);
    let bytes = std::fs::read(path).expect("read png");
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder.read_info().expect("decode png header");
    let mut pixels = vec![0; reader.output_buffer_size().expect("png output buffer size")];
    let info = reader.next_frame(&mut pixels).expect("decode png frame");
    let pixels = &pixels[..info.buffer_size()];
    let dark_pixels = pixels
        .chunks_exact(4)
        .filter(|pixel| pixel[0] < 245 || pixel[1] < 245 || pixel[2] < 245)
        .count();
    assert!(dark_pixels > 400, "{} must contain rendered vehicle pixels", path.display());
}
