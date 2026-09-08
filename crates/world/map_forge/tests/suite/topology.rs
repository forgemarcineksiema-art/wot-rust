//! W1/W2 (the one program, GDD row 35 — the owner, 2026-09-07: „lanes, crossfire, cover,
//! hull-down, flanking, sniper positions, rotation paths, fallback positions"): a map's
//! topology is DATA the report proves from geometry, not prose in a dossier. A lane is a
//! polyline with a width that leads from a spawn to a named place over drivable ground; a
//! crossfire is two positions that both see one stretch of a lane from bearings > 60° apart;
//! a sniper perch sees lane samples 250–500 m out; a rotation path is measured by how much of
//! it the other side's eyes cannot see; a fallback stands on a lane's band behind its far end.
//! Prokhorovka is the first map authored for it (the owner: „Prochorowka nie ma torów"); the
//! numbers below are the map's, measured 2026-09-08, and the other four maps follow in W1b.

use map_forge::{
    blueprint::{CrossfireSpec, LaneSpec, RotationPathSpec, StrategicPointSpec, XCoord},
    compile, lane_census, rotation_masking,
};
use terrain::{MapId, StrategicRole};

/// A flat 300 m square, two spawns, a farm building across z = 100 between x 90 and 210.
fn flat_square_with_a_wall() -> map_forge::blueprint::MapBlueprint {
    use map_forge::blueprint::{
        BaseSpec, GameplaySpec, GridSpec, MapBlueprint, MetaSpec, ObjectSpec, SpawnSpec,
        TerrainProgram,
    };
    MapBlueprint {
        meta: MetaSpec {
            version: map_forge::blueprint::BLUEPRINT_VERSION,
            id: "topology_probe".into(),
            name: "Topology probe".into(),
            historical_basis: "Synthetic test map".into(),
            design_notes: Vec::new(),
        },
        grid: GridSpec { size_m: [300.0, 300.0], cell_m: 2.5, min_height_m: 0.2 },
        symmetry: None,
        river: None,
        horizon: None,
        terrain: TerrainProgram { base: BaseSpec::Constant(5.0), ops: Vec::new() },
        sculpt: None,
        water: None,
        materials: None,
        environment: None,
        objects: vec![ObjectSpec::Cover {
            id: "the_wall".into(),
            name: "the wall".into(),
            kind: terrain::StaticCoverKind::FarmBuilding,
            at: [XCoord::Fixed(150.0), XCoord::Fixed(100.0)],
            half_extents_m: [60.0, 3.0, 3.0],
            yaw_rad: 0.0,
        }],
        scenery: Vec::new(),
        roads: Vec::new(),
        gameplay: GameplaySpec {
            formats: Vec::new(),
            spawns: vec![
                SpawnSpec { team: 1, at: [150.0, 150.0], facing_yaw_rad: 0.0, radius_m: None },
                SpawnSpec { team: 2, at: [150.0, 250.0], facing_yaw_rad: 0.0, radius_m: None },
            ],
            strategic_points: Vec::new(),
            capture_zones: Vec::new(),
            features: Vec::new(),
            lanes: Vec::new(),
            rotation_paths: Vec::new(),
            crossfires: Vec::new(),
        },
    }
}

fn topology_errors(report: &map_forge::MapReport) -> Vec<String> {
    report
        .errors()
        .filter(|entry| entry.check == "topology")
        .map(|entry| entry.message.clone())
        .collect()
}

/// Prokhorovka declares every class, the report proves each, and the census numbers are the
/// map's own: the field lane bare (0.2 boxes per 100 m), the farm corridor dense (4.7), the
/// hill lane the hull-down lane (11 crest spots in its band); the balka masks 59–69 % of its
/// run from the other side's eyes.
#[test]
fn prokhorovka_declares_its_topology_and_the_report_proves_it() {
    let map = map_forge::battlefield(MapId::ProkhorovkaHill252_2);
    let (_, report) = compile(&map_forge::blueprint_for(MapId::ProkhorovkaHill252_2));
    assert!(topology_errors(&report).is_empty(), "{:?}", topology_errors(&report));
    assert!(
        !report.warnings().any(|entry| entry.check == "topology"),
        "no topology warning either: {:?}",
        report
            .warnings()
            .filter(|e| e.check == "topology")
            .map(|e| e.message.clone())
            .collect::<Vec<_>>()
    );
    assert_eq!(map.lanes.len(), 6, "three lanes a side");
    assert_eq!(map.rotation_paths.len(), 2);
    assert_eq!(map.crossfires.len(), 2);
    let perches =
        map.strategic_points.iter().filter(|p| p.role == StrategicRole::SniperPerch).count();
    let fallbacks =
        map.strategic_points.iter().filter(|p| p.role == StrategicRole::Fallback).count();
    assert_eq!((perches, fallbacks), (2, 4));

    // W2: the census, as data.
    let census = lane_census(&map);
    let by_id = |id: &str| census.iter().find(|c| c.id == id).unwrap_or_else(|| panic!("{id}"));
    let field = by_id("psel_field_lane_south");
    let farm = by_id("farm_lane_south");
    let hill = by_id("hill_lane_south");
    assert!(field.cover_per_100m < 1.0, "the naked flank: {}", field.cover_per_100m);
    assert!(farm.cover_per_100m > 3.0, "the brawl corridor: {}", farm.cover_per_100m);
    assert!(hill.hull_down_spots >= 8, "the hull-down duel: {}", hill.hull_down_spots);
    assert!(
        hill.hull_down_spots > field.hull_down_spots && hill.hull_down_spots > farm.hull_down_spots
    );
    // Fairness: the mirrored twin carries the same census.
    for side in ["psel_field_lane", "farm_lane", "hill_lane"] {
        let (s, n) = (by_id(&format!("{side}_south")), by_id(&format!("{side}_north")));
        assert_eq!(
            (s.cover_boxes, s.hull_down_spots),
            (n.cover_boxes, n.hull_down_spots),
            "{side}"
        );
    }
    for (id, masked) in rotation_masking(&map) {
        assert!(masked >= 0.5, "the balka is a covered rotation: {id} {masked}");
    }
}

/// The contract on a synthetic map: a lane that leads nowhere, a lane through a wall, a
/// crossfire nothing can see, a fallback on no lane and a map that declares lanes without
/// every class — each is a named report error.
#[test]
fn the_report_refuses_topology_that_the_geometry_does_not_carry() {
    let mut blueprint = flat_square_with_a_wall();
    // A wall across the middle: a lane through it is blocked across its whole width.
    let wall_lane = LaneSpec::Lane {
        id: "through_the_wall".into(),
        name: "through the wall".into(),
        points: vec![[150.0, 150.0], [150.0, 60.0]],
        width_m: 20.0,
    };
    let nowhere_lane = LaneSpec::Lane {
        id: "to_nowhere".into(),
        name: "to nowhere".into(),
        points: vec![[150.0, 150.0], [40.0, 150.0]],
        width_m: 20.0,
    };
    blueprint.gameplay.lanes = vec![wall_lane, nowhere_lane];
    blueprint.gameplay.crossfires = vec![CrossfireSpec::Crossfire {
        id: "blind".into(),
        a: "eye_a".into(),
        b: "eye_b".into(),
        lane: "to_nowhere".into(),
    }];
    blueprint.gameplay.rotation_paths = vec![RotationPathSpec::Path {
        id: "rotation".into(),
        name: "rotation".into(),
        points: vec![[100.0, 150.0], [200.0, 150.0]],
        width_m: 10.0,
    }];
    blueprint.gameplay.strategic_points.extend([
        StrategicPointSpec {
            id: "eye_a".into(),
            name: "eye a".into(),
            role: StrategicRole::Observation,
            at: [XCoord::Fixed(150.0), XCoord::Fixed(250.0)],
            radius_m: 20.0,
        },
        StrategicPointSpec {
            id: "eye_b".into(),
            name: "eye b".into(),
            role: StrategicRole::Observation,
            at: [XCoord::Fixed(150.0), XCoord::Fixed(255.0)],
            radius_m: 20.0,
        },
        StrategicPointSpec {
            id: "nowhere_fallback".into(),
            name: "fallback on no lane".into(),
            role: StrategicRole::Fallback,
            at: [XCoord::Fixed(250.0), XCoord::Fixed(250.0)],
            radius_m: 20.0,
        },
    ]);
    let (_, report) = compile(&blueprint);
    let errors = topology_errors(&report).join("\n");
    assert!(errors.contains("'through_the_wall' is blocked across its whole width"), "{errors}");
    assert!(errors.contains("'to_nowhere' ends nowhere named"), "{errors}");
    assert!(errors.contains("crossfire 'blind'"), "{errors}");
    assert!(errors.contains("fallback 'nowhere_fallback' stands on no lane"), "{errors}");
    assert!(errors.contains("only 0 sniper perch(s)"), "{errors}");
}
