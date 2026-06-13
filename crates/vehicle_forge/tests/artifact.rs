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

fn assert_png(path: &std::path::Path) {
    let bytes = std::fs::read(path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
    assert!(bytes.len() > 32, "{} must not be an empty placeholder", path.display());
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n", "{} must be a PNG file", path.display());
}
