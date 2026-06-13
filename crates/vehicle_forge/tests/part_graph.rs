use game_core::{VehicleBlueprint, VehicleKind};
use vehicle_forge::{ForgePartGraph, ForgePartKind, PartAnchor, ReferencePack};
use vehicle_geometry::{MaterialRole, bake_vehicle};

#[test]
fn t54_part_graph_decomposes_into_expected_semantic_parts() {
    let graph = ForgePartGraph::for_vehicle(VehicleKind::T54_1951).expect("T-54 part graph");

    for kind in [
        ForgePartKind::Hull,
        ForgePartKind::TrackRun,
        ForgePartKind::RoadWheels,
        ForgePartKind::Turret,
        ForgePartKind::Mantlet,
        ForgePartKind::Gun,
        ForgePartKind::Cupola,
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
    for needle in ["Hull", "TrackRun", "RoadWheels", "Turret", "Mantlet", "Gun", "Cupola"] {
        assert!(report.contains(needle), "report must list {needle}");
    }
    assert!(report.contains("CastArmor"));
    assert!(report.contains("TurretRing"));
}

#[test]
fn unmigrated_vehicles_have_no_part_graph_yet() {
    assert!(ForgePartGraph::for_vehicle(VehicleKind::TigerI).is_none());
}
