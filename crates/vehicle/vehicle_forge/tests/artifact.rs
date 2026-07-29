use game_core::VehicleKind;
use vehicle_forge::{
    BakeProfile, ForgeArtifact, ReviewCamera, ReviewCameraSet, bake_production_vehicle,
};

#[test]
fn production_t54_bake_is_the_hybrid_description_at_the_requested_lod() {
    let production = bake_production_vehicle(VehicleKind::T54_1951, BakeProfile::Lod0)
        .expect("production T-54 bakes");
    let hybrid = vehicle_build::t54_description().build();

    assert_eq!(production.deterministic_hash(), hybrid.deterministic_hash());
    assert_eq!(production.mounts(), hybrid.mounts());
}

#[test]
fn forge_artifact_manifest_names_the_baked_vehicle_profile_and_sources() {
    let artifact =
        ForgeArtifact::bake(VehicleKind::T54_1951, BakeProfile::Lod0).expect("T-54 Forge artifact");
    let manifest = artifact.manifest();

    assert_eq!(manifest.vehicle_slug(), "t54-1951");
    assert_eq!(manifest.profile(), BakeProfile::Lod0);
    assert_eq!(manifest.source_family_slug(), Some("t54"));
    assert!(manifest.source_hash() != 0, "bake hash must identify the generated source");
    assert!(manifest.mesh_bytes() > 0, "artifact must account for mesh payload bytes");
    assert!(manifest.submeshes().iter().any(|submesh| submesh.kind() == "Hull"));
    assert!(manifest.submeshes().iter().any(|submesh| submesh.kind() == "Turret"));
    assert!(manifest.submeshes().iter().any(|submesh| submesh.kind() == "Gun"));
    // Role-aware material families: five roles × four maps, each tagged with its role and semantic.
    let texture_maps: Vec<&str> = manifest.texture_maps().iter().map(|map| map.file()).collect();
    assert_eq!(texture_maps.len(), 20, "five families × four maps");
    for role in ["rolled_armor", "cast_armor", "barrel_steel", "track_metal", "rubber"] {
        for semantic in ["albedo", "normal", "ao_roughness_metalness", "cavity"] {
            let file = format!("{role}_{semantic}.png");
            let entry = manifest
                .texture_maps()
                .iter()
                .find(|m| m.file() == file)
                .unwrap_or_else(|| panic!("missing material map {file}"));
            assert_eq!(entry.role(), role, "{file} must record its material role");
        }
    }

    // R8 / Task 25: the artifact records the per-vehicle ambient-contact bake — the named cavity
    // signals and a stable config hash — so the surface bake is documented and intentionally
    // invalidated when its configuration changes.
    let surface_bake = manifest.surface_bake().expect("T-54 records its contact bake");
    let signals: Vec<&str> = surface_bake.signals().iter().map(String::as_str).collect();
    assert_eq!(
        signals,
        [
            "turret_ring_seam",
            "mantlet_seat",
            "running_gear_recess",
            "engine_grille",
            // The bore: a gun muzzle is the deepest recess on the vehicle, and it used to be lit
            // exactly like the tube around it (PR-25).
            "gun_bore",
            "glacis_weld",
        ]
    );
    assert!(surface_bake.config_hash() != 0, "the cavity config hash must identify the bake");
}

#[test]
fn every_migrated_vehicle_bakes_a_full_artifact_at_every_lod() {
    for kind in [
        VehicleKind::T54_1951,
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
            // The full baked material-family set is present regardless of vehicle (5 roles × 4 maps).
            assert_eq!(manifest.texture_maps().len(), 20, "{kind:?} material set incomplete");
            // Review cameras come from the per-vehicle forge spec: the T-54 benchmark carries the
            // extra close-ups (11); the geometry-derived line gets the generic six.
            let expected_cameras = if kind == VehicleKind::T54_1951 { 11 } else { 6 };
            assert_eq!(
                manifest.review_cameras().cameras().len(),
                expected_cameras,
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
fn review_camera_sets_are_generic_with_a_t54_benchmark_extension() {
    // The standard set is generic: six all-round silhouette views, no vehicle-specific names.
    let standard: Vec<ReviewCamera> =
        ReviewCameraSet::standard_vehicle_review().cameras().iter().map(|c| c.kind()).collect();
    assert_eq!(
        standard,
        vec![
            ReviewCamera::Front,
            ReviewCamera::Rear,
            ReviewCamera::LeftProfile,
            ReviewCamera::RightProfile,
            ReviewCamera::Top,
            ReviewCamera::BattleOblique,
        ]
    );

    // The T-54 benchmark extends the generic set with five close-up regression views.
    let benchmark: Vec<ReviewCamera> =
        ReviewCameraSet::t54_benchmark_review().cameras().iter().map(|c| c.kind()).collect();
    assert_eq!(
        benchmark,
        vec![
            ReviewCamera::Front,
            ReviewCamera::Rear,
            ReviewCamera::LeftProfile,
            ReviewCamera::RightProfile,
            ReviewCamera::Top,
            ReviewCamera::BattleOblique,
            ReviewCamera::CloseFront,
            ReviewCamera::RunningGear,
            ReviewCamera::TurretMantlet,
            ReviewCamera::TopPlan,
            ReviewCamera::BattleClose,
        ]
    );
}

#[test]
fn forge_texture_maps_are_real_review_sized_pngs() {
    let artifact =
        ForgeArtifact::bake(VehicleKind::T54_1951, BakeProfile::Lod0).expect("T-54 Forge artifact");

    for map in artifact.manifest().texture_maps() {
        let bytes = artifact.texture_payload(map.file()).expect("texture payload");
        let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
        let reader = decoder.read_info().expect("decode texture header");
        let info = reader.info();
        assert!(info.width >= 256, "{} too narrow: {}", map.file(), info.width);
        assert!(info.height >= 256, "{} too short: {}", map.file(), info.height);
    }
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
    // A representative slice of the role-aware material-family set lands on disk.
    assert_png(&out.join("rolled_armor_albedo.png"));
    assert_png(&out.join("cast_armor_normal.png"));
    assert_png(&out.join("track_metal_cavity.png"));
    assert_png(&out.join("rubber_ao_roughness_metalness.png"));
    for file in [
        "front.png",
        "rear.png",
        "left_profile.png",
        "right_profile.png",
        "top.png",
        "battle_oblique.png",
        "close_front.png",
        "running_gear.png",
        "turret_mantlet.png",
        "top_plan.png",
        "battle_close.png",
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
    assert!(report.contains("T-54 Forge reference report"));
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

/// The artifact source hash is the baked-vehicle deterministic hash, so it must move whenever a
/// cast-loft input that changes the shell does: a station, a bump, the cap mode, or the material
/// mapping. This is the invalidation guarantee — a silent cast-loft edit cannot reuse a stale
/// artifact.
#[test]
fn t54_source_hash_reacts_to_cast_loft_inputs() {
    use cast_loft::{CastBump, CastCap, CastCaps, CastLoftSpec, CastSection, try_build_cast_loft};
    use game_core::MountFrames;
    use glam::Vec3;
    use vehicle_geometry::{BakedVehicle, MaterialRole, SmoothingGroup, Submesh, SubmeshKind};

    let sections = [
        CastSection::symmetric(0.0, 0.84, 0.92, 0.0, 2.8),
        CastSection::symmetric(0.2, 0.90, 0.96, -0.02, 2.8),
        CastSection::symmetric(0.5, 0.50, 0.60, -0.06, 2.8),
    ];
    let bump = CastBump {
        azimuth: 0.0,
        az_width: 0.4,
        y: 0.2,
        y_width: 0.2,
        amount: 0.12,
        falloff_exponent: 2.0,
    };
    let planar = CastCaps { bottom: CastCap::Planar, top: CastCap::Planar };

    let hash = |sections: &[CastSection], bumps: &[CastBump], caps, material| {
        let mesh = try_build_cast_loft(&CastLoftSpec {
            sections,
            bumps,
            segments: 32,
            caps,
            material,
            smoothing: SmoothingGroup(2),
        })
        .expect("valid cast loft");
        BakedVehicle::new(
            VehicleKind::T54_1951,
            vec![Submesh { kind: SubmeshKind::Turret, mesh }],
            MountFrames::for_vehicle(VehicleKind::T54_1951),
        )
        .deterministic_hash()
    };

    let baseline = hash(&sections, &[bump], planar, MaterialRole::CastArmor);

    let mut moved = sections;
    moved[1].half_width += 0.05;
    assert_ne!(baseline, hash(&moved, &[bump], planar, MaterialRole::CastArmor), "station");

    let mut bigger = bump;
    bigger.amount += 0.03;
    assert_ne!(baseline, hash(&sections, &[bigger], planar, MaterialRole::CastArmor), "bump");

    let apex =
        CastCaps { bottom: CastCap::Planar, top: CastCap::Apex(Vec3::new(0.0, 0.55, -0.02)) };
    assert_ne!(baseline, hash(&sections, &[bump], apex, MaterialRole::CastArmor), "cap mode");

    assert_ne!(baseline, hash(&sections, &[bump], planar, MaterialRole::RolledArmor), "material");
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
