//! W1/W2 (the one program, GDD row 35 — the owner, 2026-09-07: „lanes, crossfire, cover,
//! hull-down, flanking, sniper positions, rotation paths, fallback positions"): a map's
//! topology is DATA the report proves from geometry, not prose in a dossier. A lane is a
//! polyline with a width that leads from a spawn to a named place over drivable ground; a
//! crossfire is two positions that both see one stretch of a lane from bearings > 60° apart;
//! a sniper perch sees lane samples 250–500 m out; a rotation path is measured by how much of
//! it the other side's eyes cannot see; a fallback stands on a lane's band behind its far end.
//! Prokhorovka was the first map authored for it (the owner: „Prochorowka nie ma torów"); the
//! other four followed the same day (W1b) — every shipped map declares every class, and the
//! numbers below are each map's own, measured 2026-09-08.

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

/// W1b: every shipped map declares every class and the report proves it — and says, on the
/// record, where the geometry does not carry a covered rotation: Orliny's shoulder walk is
/// seen whole from the summits (0 % masked — the massif's own eyes), Mazurski's peat defiles
/// 19 % (a lakeland is open by design, the dossier's own word). The census per map (south
/// lanes; the north twin carries the same numbers) is what the dossiers cite.
#[test]
fn every_shipped_map_declares_every_topology_class_and_the_report_proves_it() {
    let exposed_on_record: [(MapId, &str); 2] =
        [(MapId::OrlinyPereval, "shoulder_rotation"), (MapId::MazurskiPrzesmyk, "defile_rotation")];
    // (map, lane base, [length band, cover per 100 m band, hull-down floor])
    let census_table: [(MapId, &str, [f32; 2], [f32; 2], usize); 15] = [
        (MapId::ProkhorovkaHill252_2, "psel_field_lane", [400.0, 450.0], [0.0, 1.0], 4),
        (MapId::ProkhorovkaHill252_2, "farm_lane", [320.0, 370.0], [3.0, 6.0], 1),
        (MapId::ProkhorovkaHill252_2, "hill_lane", [400.0, 450.0], [0.5, 2.0], 8),
        (MapId::BystraValley, "field_lane", [420.0, 480.0], [1.0, 2.5], 0),
        (MapId::BystraValley, "valley_lane", [440.0, 560.0], [2.0, 4.0], 1),
        (MapId::BystraValley, "ford_lane", [590.0, 650.0], [1.5, 3.5], 3),
        (MapId::OrlinyPereval, "dolina_lane", [620.0, 680.0], [0.5, 2.0], 0),
        (MapId::OrlinyPereval, "pass_lane", [390.0, 430.0], [3.0, 5.5], 0),
        (MapId::OrlinyPereval, "defile_lane", [640.0, 700.0], [0.5, 2.0], 0),
        (MapId::Ostrogorsk, "mill_lane", [580.0, 620.0], [3.0, 5.5], 0),
        (MapId::Ostrogorsk, "boulevard_lane", [400.0, 450.0], [2.0, 4.0], 0),
        (MapId::Ostrogorsk, "outskirts_lane", [470.0, 520.0], [0.5, 2.0], 0),
        (MapId::MazurskiPrzesmyk, "causeway_lane", [460.0, 510.0], [1.5, 3.5], 3),
        (MapId::MazurskiPrzesmyk, "shore_lane", [690.0, 740.0], [0.0, 1.2], 0),
        (MapId::MazurskiPrzesmyk, "moraine_lane", [550.0, 590.0], [1.5, 3.5], 0),
    ];
    for id in MapId::SHIPPED {
        let (map, report) = compile(&map_forge::blueprint_for(*id));
        assert!(topology_errors(&report).is_empty(), "{id:?}: {:?}", topology_errors(&report));
        let warned: Vec<String> = report
            .warnings()
            .filter(|entry| entry.check == "topology")
            .map(|entry| entry.message.clone())
            .collect();
        let expected_warning =
            exposed_on_record.iter().find(|(m, _)| m == id).map(|(_, path)| *path);
        match expected_warning {
            Some(path) => assert!(
                warned.len() == 2
                    && warned.iter().all(|w| w.contains(path) && w.contains("is exposed")),
                "{id:?}: the exposed rotation is on the record and nothing else: {warned:?}"
            ),
            None => assert!(warned.is_empty(), "{id:?}: no topology warning: {warned:?}"),
        }
        let sides = map.spawn_zones.len();
        assert_eq!(map.lanes.len(), 3 * sides, "{id:?}: three lanes a side");
        assert_eq!(map.rotation_paths.len(), sides, "{id:?}: one rotation path a side");
        assert_eq!(map.crossfires.len(), sides, "{id:?}: one crossfire a side");
        let count = |role| map.strategic_points.iter().filter(|p| p.role == role).count();
        assert_eq!(count(StrategicRole::SniperPerch), sides, "{id:?}: one perch a side");
        assert_eq!(count(StrategicRole::Fallback), 2 * sides, "{id:?}: two fallbacks a side");

        let census = lane_census(&map);
        for (m, base, length, cover, hull_down) in &census_table {
            if m != id {
                continue;
            }
            let south = census.iter().find(|c| c.id == format!("{base}_south")).expect(base);
            let north = census.iter().find(|c| c.id == format!("{base}_north")).expect(base);
            assert!(
                (length[0]..=length[1]).contains(&south.length_m),
                "{id:?} {base}: length {} m",
                south.length_m
            );
            assert!(
                (cover[0]..=cover[1]).contains(&south.cover_per_100m),
                "{id:?} {base}: {} boxes per 100 m",
                south.cover_per_100m
            );
            assert!(
                south.hull_down_spots >= *hull_down,
                "{id:?} {base}: {} hull-down spots",
                south.hull_down_spots
            );
            assert_eq!(
                (south.cover_boxes, south.hull_down_spots),
                (north.cover_boxes, north.hull_down_spots),
                "{id:?} {base}: the twin carries the same census"
            );
        }
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
