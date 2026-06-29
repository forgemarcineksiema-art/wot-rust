use game_core::{VehicleBlueprint, VehicleKind};
use vehicle_build::GeneratorKind;
use vehicle_forge::{
    ForgePartGraph, ForgePartKind, GameplayRole, LodPolicy, PartAnchor, PartGroup, ReferencePack,
    part_manifest_report, production_part_manifest, validate_production_manifest,
};
use vehicle_geometry::{MaterialRole, MeshBounds, SubmeshKind, bake_vehicle};

#[test]
fn t54_part_graph_decomposes_into_expected_semantic_parts() {
    let graph = ForgePartGraph::for_vehicle(VehicleKind::T54_1951).expect("T-54 part graph");

    for kind in [
        ForgePartKind::Hull,
        ForgePartKind::UpperGlacis,
        ForgePartKind::LowerPlate,
        ForgePartKind::Fenders,
        ForgePartKind::TrackRun,
        ForgePartKind::TrackBelt,
        ForgePartKind::RoadWheels,
        ForgePartKind::RoadWheelSet,
        ForgePartKind::Idler,
        ForgePartKind::DriveSprocket,
        ForgePartKind::Turret,
        ForgePartKind::TurretCheeks,
        ForgePartKind::Mantlet,
        ForgePartKind::MantletSocket,
        ForgePartKind::MovingMantlet,
        ForgePartKind::Gun,
        ForgePartKind::Cupola,
        ForgePartKind::EngineDeck,
    ] {
        let part = graph.part(kind).unwrap_or_else(|| panic!("graph missing {kind:?}"));
        assert!(
            !part.source().trim().is_empty(),
            "{kind:?} must record where its proportions come from"
        );
        let bounds = part.bounds();
        assert!(bounds.min.is_finite() && bounds.max.is_finite(), "{kind:?} bounds finite");
        assert!(
            bounds.max.x > bounds.min.x
                && bounds.max.y > bounds.min.y
                && bounds.max.z > bounds.min.z,
            "{kind:?} must be a non-degenerate volume"
        );
        assert!(part.frame().translation.is_finite());
    }
}

#[test]
fn t54_parts_partition_into_hull_turret_gun_groups() {
    let graph = ForgePartGraph::for_vehicle(VehicleKind::T54_1951).expect("T-54 part graph");

    // Every part lands in exactly one group, and the three groups cover the whole graph — so the
    // runtime can assemble hull, turret and gun separately with nothing left over or double-counted.
    let total = graph.parts().len();
    let mut counted = 0;
    for group in [PartGroup::Hull, PartGroup::Turret, PartGroup::Gun] {
        let n = graph.parts_in_group(group).count();
        assert!(n > 0, "{group:?} group must have parts");
        counted += n;
        for part in graph.parts_in_group(group) {
            assert_eq!(part.group(), group, "{:?} routed to the wrong group", part.kind());
        }
    }
    assert_eq!(counted, total, "groups must partition the part graph exactly");

    // The group follows the pose anchor: the gun trunnion parts (barrel + moving mantlet) are Gun.
    let gun_kinds: Vec<_> = graph.parts_in_group(PartGroup::Gun).map(|p| p.kind()).collect();
    assert!(gun_kinds.contains(&ForgePartKind::Gun));
    assert!(gun_kinds.contains(&ForgePartKind::MovingMantlet));
    assert!(
        graph.parts_in_group(PartGroup::Turret).any(|p| p.kind() == ForgePartKind::Turret),
        "the turret shell is in the turret group"
    );
}

#[test]
fn t54_parts_carry_gameplay_roles_and_lod_policy() {
    let graph = ForgePartGraph::for_vehicle(VehicleKind::T54_1951).expect("T-54 part graph");

    // Mount-bearing parts are kept through every LOD tier so the pose chain survives at LOD2.
    assert_eq!(graph.part(ForgePartKind::Turret).unwrap().lod_policy(), LodPolicy::MountCritical);
    assert_eq!(graph.part(ForgePartKind::Gun).unwrap().lod_policy(), LodPolicy::MountCritical);
    // Cosmetic fittings are detail that drops out by LOD2.
    assert_eq!(graph.part(ForgePartKind::Fenders).unwrap().lod_policy(), LodPolicy::Detail);

    // Gameplay roles read off correctly: the barrel is the weapon, the wheels are running gear.
    assert_eq!(graph.part(ForgePartKind::Gun).unwrap().gameplay_role(), GameplayRole::Weapon);
    assert_eq!(
        graph.part(ForgePartKind::RoadWheels).unwrap().gameplay_role(),
        GameplayRole::RunningGear
    );
    assert_eq!(graph.part(ForgePartKind::Hull).unwrap().gameplay_role(), GameplayRole::Armor);
}

#[test]
fn t54_hull_and_turret_bounds_fit_the_gameplay_hitbox() {
    let graph = ForgePartGraph::for_vehicle(VehicleKind::T54_1951).expect("T-54 part graph");
    let bp = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("T-54 blueprint");
    let h = bp.hull;
    let (xw, zl) = (h.hitbox_half_width, h.hitbox_half_length);
    let (ylo, yhi) =
        (h.hitbox_center_y - h.hitbox_half_height, h.hitbox_center_y + h.hitbox_half_height);

    // Hitbox honesty: the hull and turret shells live inside the gameplay collision box, so what the
    // shell resolves against is no smaller than what is drawn.
    for kind in [ForgePartKind::Hull, ForgePartKind::Turret] {
        let b = graph.part(kind).expect("part").bounds();
        assert!(b.min.x >= -xw - 1.0e-3 && b.max.x <= xw + 1.0e-3, "{kind:?} x escapes the hitbox");
        assert!(b.min.z >= -zl - 1.0e-3 && b.max.z <= zl + 1.0e-3, "{kind:?} z escapes the hitbox");
        assert!(
            b.min.y >= ylo - 1.0e-3 && b.max.y <= yhi + 1.0e-3,
            "{kind:?} y escapes the hitbox"
        );
    }
}

#[test]
fn t54_road_wheel_count_agrees_with_reference_pack() {
    let graph = ForgePartGraph::for_vehicle(VehicleKind::T54_1951).expect("T-54 part graph");
    let pack = ReferencePack::for_vehicle(VehicleKind::T54_1951).expect("T-54 reference pack");

    assert_eq!(graph.road_wheel_count_per_side(), pack.road_wheel_count_per_side());
    assert_eq!(graph.road_wheel_count_per_side(), 5);
}

#[test]
fn t54_mount_frames_derive_from_the_part_graph() {
    let graph = ForgePartGraph::for_vehicle(VehicleKind::T54_1951).expect("T-54 part graph");
    let blueprint = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("T-54 blueprint");

    // The semantic graph supersedes the flat blueprint: the mount chain it derives from its parts
    // must reproduce the blueprint's mounts exactly, or the two have drifted apart.
    assert_eq!(graph.mount_frames(), blueprint.mount_frames());
}

#[test]
fn t54_turret_traverses_and_reads_as_cast_armor() {
    let graph = ForgePartGraph::for_vehicle(VehicleKind::T54_1951).expect("T-54 part graph");

    let turret = graph.part(ForgePartKind::Turret).expect("turret part");
    assert_eq!(turret.anchor(), PartAnchor::TurretRing);
    assert_eq!(turret.material(), MaterialRole::CastArmor);
    assert!(graph.turret_traverses(), "T-54 turret is not a fixed casemate");

    let gun = graph.part(ForgePartKind::Gun).expect("gun part");
    assert_eq!(gun.anchor(), PartAnchor::GunTrunnion);
    assert_eq!(gun.material(), MaterialRole::BarrelSteel);
}

#[test]
fn t54_part_bounds_stay_inside_the_baked_vehicle() {
    let graph = ForgePartGraph::for_vehicle(VehicleKind::T54_1951).expect("T-54 part graph");
    let baked = bake_vehicle(VehicleKind::T54_1951).expect("T-54 bakes");

    let mut vehicle = None::<vehicle_geometry::MeshBounds>;
    for kind in [
        vehicle_geometry::SubmeshKind::Hull,
        vehicle_geometry::SubmeshKind::Turret,
        vehicle_geometry::SubmeshKind::Gun,
    ] {
        if let Some(bounds) = baked.submesh(kind).and_then(|submesh| submesh.mesh.bounds()) {
            vehicle = Some(match vehicle {
                Some(acc) => acc.union(bounds),
                None => bounds,
            });
        }
    }
    let vehicle = vehicle.expect("baked vehicle has bounds");
    let eps = 0.12;

    for part in graph.parts() {
        let b = part.bounds();
        assert!(
            b.min.x >= vehicle.min.x - eps
                && b.min.y >= vehicle.min.y - eps
                && b.min.z >= vehicle.min.z - eps
                && b.max.x <= vehicle.max.x + eps
                && b.max.y <= vehicle.max.y + eps
                && b.max.z <= vehicle.max.z + eps,
            "{:?} bounds {:?} escape the baked vehicle {:?}",
            part.kind(),
            b,
            vehicle
        );
    }
}

#[test]
fn t54_part_report_explains_existence_role_and_source() {
    let graph = ForgePartGraph::for_vehicle(VehicleKind::T54_1951).expect("T-54 part graph");
    let report = graph.part_report();

    assert!(report.contains("T-54"));
    assert!(report.contains("Source"));
    for needle in [
        "Hull",
        "UpperGlacis",
        "LowerPlate",
        "Fenders",
        "TrackBelt",
        "RoadWheelSet",
        "Idler",
        "DriveSprocket",
        "TurretCheeks",
        "MantletSocket",
        "MovingMantlet",
        "EngineDeck",
    ] {
        assert!(report.contains(needle), "report must list {needle}");
    }
    assert!(report.contains("CastArmor"));
    assert!(report.contains("TurretRing"));
}

#[test]
fn t55a_stays_legacy_compatible_but_has_no_production_part_graph() {
    assert!(VehicleBlueprint::for_vehicle(VehicleKind::T55A).is_some());
    assert!(ForgePartGraph::for_vehicle(VehicleKind::T55A).is_none());
}

#[test]
fn german_line_has_geometry_derived_part_graphs() {
    for kind in
        [VehicleKind::TigerI, VehicleKind::TigerII, VehicleKind::Jagdtiger, VehicleKind::PantherII]
    {
        let graph = ForgePartGraph::for_vehicle(kind).unwrap_or_else(|| panic!("{kind:?} graph"));
        let pack = ReferencePack::for_vehicle(kind).expect("pack");
        assert_eq!(graph.road_wheel_count_per_side(), pack.road_wheel_count_per_side());

        // Every coarse part exists, is sourced, and is a real volume.
        for part_kind in [
            ForgePartKind::Hull,
            ForgePartKind::TrackRun,
            ForgePartKind::RoadWheels,
            ForgePartKind::Turret,
            ForgePartKind::Mantlet,
            ForgePartKind::Gun,
            ForgePartKind::Cupola,
        ] {
            let part =
                graph.part(part_kind).unwrap_or_else(|| panic!("{kind:?} missing {part_kind:?}"));
            assert!(!part.source().trim().is_empty(), "{kind:?} {part_kind:?} unsourced");
            let b = part.bounds();
            assert!(
                b.max.x > b.min.x && b.max.y > b.min.y && b.max.z > b.min.z,
                "{kind:?} {part_kind:?} degenerate"
            );
        }

        // The graph rebuilds the same mount chain the bake authored.
        assert_eq!(graph.mount_frames(), *bake_vehicle(kind).unwrap().mounts());
    }
}

#[test]
fn geometry_derived_vehicles_do_not_inherit_bespoke_t54_parts() {
    // Regression lock for the registry fix: the part-graph strategy is now registered per vehicle,
    // so a vehicle routed through the geometry-derived strategy must never carry the T-54's bespoke
    // fittings. Before the fix, any blueprint-backed vehicle silently ran the T-54 part table.
    for kind in
        [VehicleKind::TigerI, VehicleKind::TigerII, VehicleKind::Jagdtiger, VehicleKind::PantherII]
    {
        let graph = ForgePartGraph::for_vehicle(kind).unwrap_or_else(|| panic!("{kind:?} graph"));
        for bespoke in [
            ForgePartKind::UpperGlacis,
            ForgePartKind::TurretCheeks,
            ForgePartKind::MantletSocket,
            ForgePartKind::MovingMantlet,
            ForgePartKind::EngineDeck,
            ForgePartKind::RoadWheelSet,
        ] {
            assert!(
                graph.part(bespoke).is_none(),
                "{kind:?} must not carry bespoke T-54 part {bespoke:?}"
            );
        }
    }
}

#[test]
fn jagdtiger_part_graph_reads_as_a_fixed_casemate() {
    let graph = ForgePartGraph::for_vehicle(VehicleKind::Jagdtiger).expect("Jagdtiger graph");
    assert!(!graph.turret_traverses(), "Jagdtiger casemate must not traverse");
    assert!(graph.part_report().contains("casemate"));
}

#[test]
fn the_placeholder_prototype_has_no_part_graph() {
    assert!(ForgePartGraph::for_vehicle(VehicleKind::PrototypeMedium).is_none());
}

#[test]
fn the_t54_executable_manifest_is_production_valid() {
    // Every production part must carry a non-empty source note and a non-degenerate volume.
    assert_eq!(validate_production_manifest(VehicleKind::T54_1951), Some(Ok(())));
}

#[test]
fn the_manifest_reports_the_generator_that_built_each_part() {
    let manifest = production_part_manifest(VehicleKind::T54_1951).expect("T-54 manifest");
    let generator = |name: &str| {
        manifest
            .iter()
            .find(|e| e.key.name == name)
            .unwrap_or_else(|| panic!("part {name}"))
            .generator
    };
    // The plan's selection: turret shell from the loft, glacis plates from CAD, barrel revolved.
    // The visible track belt is runtime-instanced running gear, not a static manifest part.
    assert_eq!(generator("turret_shell"), GeneratorKind::CastLoft);
    assert_eq!(generator("upper_hull"), GeneratorKind::Solid);
    assert_eq!(generator("lower_tub"), GeneratorKind::Solid);
    assert_eq!(generator("gun_barrel"), GeneratorKind::Revolve);
    assert!(
        manifest.iter().all(|entry| entry.key.name != "tracks"),
        "runtime running gear must not duplicate a static track belt in the executable manifest"
    );
}

#[test]
fn the_manifest_report_names_every_kernel_and_only_exists_for_migrated_vehicles() {
    let report = part_manifest_report(VehicleKind::T54_1951).expect("T-54 report");
    assert!(report.contains("turret_shell") && report.contains("cast_loft"));
    assert!(report.contains("gun_barrel") && report.contains("revolve"));
    assert!(report.contains("upper_hull") && report.contains("solid"));
    assert!(
        !report.contains("tracks"),
        "static production manifest should leave full track belts to runtime running gear"
    );
    // Legacy / unmigrated vehicles have no executable manifest yet.
    assert!(part_manifest_report(VehicleKind::T55A).is_none());
    assert!(production_part_manifest(VehicleKind::TigerI).is_none());
}

#[test]
fn the_executable_manifest_does_not_drift_from_the_baked_geometry() {
    // The anti-drift lock: the manifest is derived from the same description that bakes, so every
    // part's bounds must sit inside the baked vehicle, and the per-group union of part bounds must
    // match each baked submesh's bounds. Blueprint, semantic graph and executable geometry agree.
    let manifest = production_part_manifest(VehicleKind::T54_1951).expect("manifest");
    // The authoritative production bake is the hybrid description, not the legacy recipe path.
    let baked = vehicle_build::t54_description().build();

    for group in [SubmeshKind::Hull, SubmeshKind::Turret, SubmeshKind::Gun] {
        let baked_bounds =
            baked.submesh(group).and_then(|s| s.mesh.bounds()).expect("group bounds");
        let union = manifest
            .iter()
            .filter(|e| e.group == group)
            .map(|e| e.bounds)
            .reduce(MeshBounds::union)
            .expect("each group has parts");
        // The LOD0 bake is exactly the merged parts, so the union reproduces the baked bounds.
        let eps = 1.0e-3;
        assert!((union.min - baked_bounds.min).abs().max_element() < eps, "{group:?} min drift");
        assert!((union.max - baked_bounds.max).abs().max_element() < eps, "{group:?} max drift");
    }
}
