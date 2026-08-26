//! Standing-water sheets (teren W6): two pools at DIFFERENT levels on one map — the thing
//! a single global table could never say — with every contract that keeps them honest:
//! in-bounds, non-overlapping, dry-edged rects (the shell splash's analytic planes are
//! complete only because the only way into a pool is down through its surface), a table
//! that never doubles a sheet's column, symmetry pairing, and the content hash.

use map_forge::blueprint::{
    BaseSpec, Gauss2Term, GridSpec, MapBlueprint, MetaSpec, SpawnSpec, StandingWaterSpec,
    SymmetrySpec, TerrainOp, TerrainProgram, WaterSpec,
};
use map_forge::compile;

/// A flat 10 m plateau with two carved lake beds and two sheets over them: the western
/// tarn at 8 m, the eastern pond at 6 m. The global table sits at 0 — under the terrain
/// floor everywhere, the honest "no valley water" statement.
fn two_lakes() -> MapBlueprint {
    MapBlueprint {
        meta: MetaSpec {
            version: map_forge::blueprint::BLUEPRINT_VERSION,
            id: "two_lakes_probe".into(),
            name: "Two lakes probe".into(),
            historical_basis: "Synthetic standing-water test map".into(),
            design_notes: Vec::new(),
        },
        grid: GridSpec { size_m: [300.0, 300.0], cell_m: 5.0, min_height_m: 0.2 },
        symmetry: None,
        river: None,
        horizon: None,
        terrain: TerrainProgram {
            base: BaseSpec::Constant(10.0),
            ops: vec![TerrainOp::Gauss2 {
                apply: map_forge::blueprint::Apply::Subtract,
                terms: vec![
                    Gauss2Term { x: 80.0, z: 80.0, sx: 12.0, sz: 12.0, amp: 6.0 },
                    Gauss2Term { x: 220.0, z: 220.0, sx: 12.0, sz: 12.0, amp: 6.0 },
                ],
            }],
        },
        sculpt: None,
        water: Some(WaterSpec {
            surface_level_m: 0.0,
            bodies: vec![
                StandingWaterSpec { rect: [50.0, 50.0, 110.0, 110.0], surface_level_m: 8.0 },
                StandingWaterSpec { rect: [190.0, 190.0, 250.0, 250.0], surface_level_m: 6.0 },
            ],
        }),
        materials: None,
        environment: None,
        objects: Vec::new(),
        scenery: Vec::new(),
        roads: Vec::new(),
        gameplay: map_forge::blueprint::GameplaySpec {
            spawns: vec![
                SpawnSpec { team: 1, at: [150.0, 40.0], facing_yaw_rad: 0.0, radius_m: None },
                SpawnSpec { team: 2, at: [150.0, 260.0], facing_yaw_rad: 0.0, radius_m: None },
            ],
            strategic_points: Vec::new(),
            capture_zones: Vec::new(),
            features: Vec::new(),
        },
    }
}

/// The whole pipeline speaks two levels: the report is clean, and the resolution rule
/// answers each pool's own table inside its rect and NOTHING outside (the global 0 m
/// table is under the floor). The mesh side of the promise is locked in `scene_build`'s
/// own tests (the layer DAG keeps the render crate out of this one).
#[test]
fn two_pools_at_two_levels_compile_and_resolve() {
    let blueprint = two_lakes();
    let (map, report) = compile(&blueprint);
    let errors: Vec<String> =
        report.errors().map(|entry| format!("{}: {}", entry.check, entry.message)).collect();
    assert!(errors.is_empty(), "two honest pools compile clean:\n{}", errors.join("\n"));

    let field = map.water_field();
    assert_eq!(field.level_at(80.0, 80.0), Some(8.0), "the tarn's own table");
    assert_eq!(field.level_at(220.0, 220.0), Some(6.0), "the pond's own table");
    assert!(field.depth_at(4.0, 80.0, 80.0) > 3.9, "real depth in the tarn");
    assert_eq!(
        field.depth_at(10.0, 150.0, 150.0),
        0.0,
        "the 0 m global table is under the floor - dry between the pools"
    );
}

/// The content hash SEES the sheets: moving one pool's level is a different map.
#[test]
fn the_content_hash_sees_a_sheet_level_move() {
    let (map_a, _) = compile(&two_lakes());
    let mut moved = two_lakes();
    moved.water.as_mut().unwrap().bodies[1].surface_level_m = 6.5;
    let (map_b, _) = compile(&moved);
    assert_ne!(
        map_forge::battlefield_hash(&map_a),
        map_forge::battlefield_hash(&map_b),
        "a sheet's level is map content"
    );
}

/// Every sheet gate BITES: overlap, out-of-bounds, a shoreline leaking through the rect
/// edge, and a global table doubling a sheet's column.
#[test]
fn the_sheet_gates_bite() {
    let mut overlapping = two_lakes();
    overlapping.water.as_mut().unwrap().bodies[1].rect = [100.0, 100.0, 250.0, 250.0];
    let (_, report) = compile(&overlapping);
    assert!(
        report
            .errors()
            .any(|entry| entry.check == "standing_water" && entry.message.contains("overlap")),
        "overlap gate"
    );

    let mut outside = two_lakes();
    outside.water.as_mut().unwrap().bodies[0].rect = [-10.0, 50.0, 110.0, 110.0];
    let (_, report) = compile(&outside);
    assert!(
        report.errors().any(|entry| entry.message.contains("leaves the playfield")),
        "bounds gate"
    );

    // Cut the tarn's rect straight through open water: the east edge at x 80 crosses the
    // bowl where the bed (4 m) lies under the 8 m level.
    let mut leaking = two_lakes();
    leaking.water.as_mut().unwrap().bodies[0].rect = [50.0, 50.0, 80.0, 110.0];
    let (_, report) = compile(&leaking);
    assert!(
        report.errors().any(|entry| entry.message.contains("shoreline leaks")),
        "the dry-edge contract is what makes the splash planes complete"
    );

    // Raise the global table over the tarn's bed: one column, two surfaces.
    let mut doubled = two_lakes();
    doubled.water.as_mut().unwrap().surface_level_m = 5.0;
    let (_, report) = compile(&doubled);
    assert!(
        report.errors().any(|entry| entry.message.contains("two surfaces")),
        "the table may not double a sheet's column"
    );
}

/// On a fair map the sheets pair under the symmetry like everything else.
#[test]
fn sheets_pair_under_the_symmetry() {
    let mut fair = two_lakes();
    fair.symmetry = Some(SymmetrySpec::MirrorZ);
    // Mirror the terrain bowls so the height probe passes; pair the sheets to match.
    fair.terrain.ops = vec![TerrainOp::Gauss2 {
        apply: map_forge::blueprint::Apply::Subtract,
        terms: vec![
            Gauss2Term { x: 80.0, z: 80.0, sx: 12.0, sz: 12.0, amp: 6.0 },
            Gauss2Term { x: 80.0, z: 220.0, sx: 12.0, sz: 12.0, amp: 6.0 },
        ],
    }];
    fair.water.as_mut().unwrap().bodies = vec![
        StandingWaterSpec { rect: [50.0, 50.0, 110.0, 110.0], surface_level_m: 8.0 },
        StandingWaterSpec { rect: [50.0, 190.0, 110.0, 250.0], surface_level_m: 8.0 },
    ];
    let (_, report) = compile(&fair);
    assert!(
        !report.errors().any(|entry| entry.check == "symmetry"),
        "a mirrored pair of tarns is fair"
    );

    let mut orphan = fair;
    orphan.water.as_mut().unwrap().bodies.pop();
    let (_, report) = compile(&orphan);
    assert!(
        report
            .errors()
            .any(|entry| entry.check == "symmetry" && entry.message.contains("standing sheet")),
        "an orphaned sheet is named by the twin hunt"
    );
}
