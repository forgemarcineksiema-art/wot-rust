//! Locks for the drawn-stroke terrain op (Ręce do terenu W1): the band raises/carves/benches
//! exactly where the polyline says and nowhere else, mirrored pairs stay fair to the bit,
//! the compile-time cull is invisible, degenerate gestures land in the report, and a
//! stroke-heavy document still fits the edit-loop budget.

use map_forge::blueprint::{
    BaseSpec, GridSpec, MapBlueprint, MetaSpec, SpawnSpec, StrokeProfile, StrokeSpec, SymmetrySpec,
    TerrainOp, TerrainProgram,
};
use map_forge::{Severity, blueprint_for, compile};
use terrain::MapId;

/// A minimal valid flat document to draw on.
fn flat_square() -> MapBlueprint {
    MapBlueprint {
        meta: MetaSpec {
            version: map_forge::blueprint::BLUEPRINT_VERSION,
            id: "stroke_probe".into(),
            name: "Stroke probe".into(),
            historical_basis: "Synthetic test map".into(),
            design_notes: Vec::new(),
        },
        grid: GridSpec { size_m: [300.0, 300.0], cell_m: 5.0, min_height_m: 0.2 },
        symmetry: None,
        river: None,
        horizon: None,
        terrain: TerrainProgram { base: BaseSpec::Constant(5.0), ops: Vec::new() },
        sculpt: None,
        water: None,
        materials: None,
        environment: None,
        objects: Vec::new(),
        scenery: Vec::new(),
        roads: Vec::new(),
        gameplay: map_forge::blueprint::GameplaySpec {
            spawns: vec![
                SpawnSpec { team: 1, at: [150.0, 150.0], facing_yaw_rad: 0.0, radius_m: None },
                SpawnSpec { team: 2, at: [150.0, 40.0], facing_yaw_rad: 0.0, radius_m: None },
            ],
            strategic_points: Vec::new(),
            capture_zones: Vec::new(),
            features: Vec::new(),
        },
    }
}

fn stroke(points: &[[f32; 2]], profile: StrokeProfile) -> TerrainOp {
    TerrainOp::Stroke(StrokeSpec {
        points: points.to_vec(),
        profile,
        half_width_m: 6.0,
        falloff_m: 6.0,
    })
}

#[test]
fn a_ridge_stroke_raises_along_its_polyline_and_only_within_its_band() {
    let bare = compile(&flat_square()).0;
    let mut blueprint = flat_square();
    let points = [[60.0, 60.0], [60.0, 160.0], [160.0, 220.0]];
    blueprint.terrain.ops.push(stroke(&points, StrokeProfile::Ridge { amp_m: 4.0 }));
    let (map, report) = compile(&blueprint);
    assert!(!report.has_errors());

    // Full strength ON the line — including the middle of a segment, not just waypoints.
    for (x, z) in [(60.0, 60.0), (60.0, 110.0), (110.0, 190.0), (160.0, 220.0)] {
        let h = map.heightmap.sample_height(x, z).unwrap();
        assert_eq!(h, 9.0, "on the centerline the ridge stands at base + amp, got {h}");
    }
    // The skirt eases: inside the falloff the ground is raised but not to full height.
    let skirt = map.heightmap.sample_height(60.0 + 9.0, 110.0).unwrap();
    assert!(skirt > 5.0 && skirt < 9.0, "the skirt eases between base and crest, got {skirt}");
    // Beyond half_width + falloff the map is BITWISE the bare map — the band ends exactly.
    for (x, z) in [(250.0, 60.0), (60.0, 250.0), (200.0, 60.0)] {
        assert_eq!(
            map.heightmap.sample_height(x, z).unwrap().to_bits(),
            bare.heightmap.sample_height(x, z).unwrap().to_bits(),
            "outside its support a stroke must not move a single bit at ({x}, {z})"
        );
    }
}

#[test]
fn a_valley_carves_and_a_plateau_lerps_exactly_to_target_under_a_full_mask() {
    let mut valley = flat_square();
    valley
        .terrain
        .ops
        .push(stroke(&[[80.0, 60.0], [80.0, 240.0]], StrokeProfile::Valley { depth_m: 3.0 }));
    let map = compile(&valley).0;
    assert_eq!(map.heightmap.sample_height(80.0, 150.0).unwrap(), 2.0, "the draw carves down");

    let mut plateau = flat_square();
    plateau
        .terrain
        .ops
        .push(stroke(&[[200.0, 60.0], [200.0, 240.0]], StrokeProfile::Plateau { target_m: 12.0 }));
    let map = compile(&plateau).0;
    assert_eq!(
        map.heightmap.sample_height(200.0, 150.0).unwrap(),
        12.0,
        "under a full mask the bench IS the target (the lerp contract)"
    );
}

#[test]
fn a_mirrored_stroke_pair_keeps_the_heightfield_fair_to_the_millimetre() {
    let mut blueprint = flat_square();
    blueprint.symmetry = Some(SymmetrySpec::MirrorZ);
    // Spawns must mirror for the fairness story to read; keep them paired across z = 150.
    blueprint.gameplay.spawns[1].at = [150.0, 150.0];
    let south: Vec<[f32; 2]> = vec![[70.0, 60.0], [110.0, 90.0], [170.0, 120.0]];
    let north: Vec<[f32; 2]> = south.iter().map(|[x, z]| [*x, 300.0 - z]).collect::<Vec<_>>();
    blueprint.terrain.ops.push(stroke(&south, StrokeProfile::Ridge { amp_m: 5.0 }));
    blueprint.terrain.ops.push(stroke(&north, StrokeProfile::Ridge { amp_m: 5.0 }));
    let (map, report) = compile(&blueprint);
    let symmetry_errors: Vec<_> = report
        .errors()
        .filter(|entry| entry.check == "symmetry")
        .map(|entry| entry.message.clone())
        .collect();
    assert!(symmetry_errors.is_empty(), "a reflected pair is fair: {symmetry_errors:?}");
    // Stronger than the report's 1 mm: on the 0.5 m lattice the reflection is BIT-exact.
    let side = map.heightmap.width();
    for zi in 0..side {
        for xi in 0..side {
            assert_eq!(
                map.heightmap.sample_at_index(xi, zi).to_bits(),
                map.heightmap.sample_at_index(xi, side - 1 - zi).to_bits(),
                "mirror twin samples must match bit-for-bit at ({xi}, {zi})"
            );
        }
    }
}

/// The cull's contract, expressed as its consequence: adding a stroke changes NOTHING
/// outside its support rectangle (bitwise), and does change the band inside it — so the
/// compiler's skip can never be told apart from full evaluation.
#[test]
fn the_stroke_cull_is_bitwise_invisible() {
    let bare = compile(&flat_square()).0;
    let mut blueprint = flat_square();
    let points = [[60.0, 60.0], [60.0, 160.0]];
    blueprint.terrain.ops.push(stroke(&points, StrokeProfile::Ridge { amp_m: 4.0 }));
    let map = compile(&blueprint).0;

    let side = map.heightmap.width();
    let cell = 5.0_f32;
    let reach = 12.0_f32; // half_width + falloff
    let mut touched = 0usize;
    for zi in 0..side {
        for xi in 0..side {
            let (x, z) = (xi as f32 * cell, zi as f32 * cell);
            let inside_rect =
                x >= 60.0 - reach && x <= 60.0 + reach && z >= 60.0 - reach && z <= 160.0 + reach;
            let with = map.heightmap.sample_at_index(xi, zi);
            let without = bare.heightmap.sample_at_index(xi, zi);
            if inside_rect {
                touched += usize::from(with != without);
            } else {
                assert_eq!(
                    with.to_bits(),
                    without.to_bits(),
                    "outside the support rectangle the stroke moved a bit at ({x}, {z})"
                );
            }
        }
    }
    assert!(touched > 50, "the band must actually shape the ground inside its rectangle");
}

#[test]
fn degenerate_strokes_land_in_the_report_not_in_a_panic() {
    let entries = |blueprint: &MapBlueprint| {
        let (_, report) = compile(blueprint);
        report
            .entries
            .iter()
            .filter(|entry| entry.check == "stroke")
            .map(|entry| (entry.severity, entry.message.clone()))
            .collect::<Vec<_>>()
    };

    let mut dot = flat_square();
    dot.terrain.ops.push(stroke(&[[100.0, 100.0]], StrokeProfile::Ridge { amp_m: 4.0 }));
    let found = entries(&dot);
    assert!(
        found.iter().any(|(s, m)| *s == Severity::Error && m.contains("not a dot")),
        "a one-point stroke is an Error, got {found:?}"
    );

    let mut spray = flat_square();
    let many: Vec<[f32; 2]> = (0..70).map(|i| [30.0 + 3.0 * i as f32, 100.0]).collect::<Vec<_>>();
    spray.terrain.ops.push(stroke(&many, StrokeProfile::Ridge { amp_m: 4.0 }));
    let found = entries(&spray);
    assert!(
        found.iter().any(|(s, m)| *s == Severity::Error && m.contains("64-point")),
        "past the point budget is an Error, got {found:?}"
    );

    let mut sliver = flat_square();
    sliver.terrain.ops.push(TerrainOp::Stroke(StrokeSpec {
        points: vec![[60.0, 60.0], [60.0, 200.0]],
        profile: StrokeProfile::Ridge { amp_m: 4.0 },
        half_width_m: 0.1,
        falloff_m: 6.0,
    }));
    let found = entries(&sliver);
    assert!(
        found.iter().any(|(s, m)| *s == Severity::Error && m.contains("half_width_m")),
        "a sliver band leaves the envelope, got {found:?}"
    );

    let mut wiggle = flat_square();
    wiggle.terrain.ops.push(stroke(
        &[[60.0, 60.0], [60.5, 60.0], [60.5, 200.0]],
        StrokeProfile::Ridge { amp_m: 4.0 },
    ));
    let found = entries(&wiggle);
    assert!(
        found.iter().any(|(s, m)| *s == Severity::Warning && m.contains("wiggle")),
        "a sub-metre segment warns, got {found:?}"
    );

    let mut slipped = flat_square();
    slipped
        .terrain
        .ops
        .push(stroke(&[[100.0, 100.0], [340.0, 100.0]], StrokeProfile::Ridge { amp_m: 4.0 }));
    let found = entries(&slipped);
    assert!(
        found.iter().any(|(s, m)| *s == Severity::Warning && m.contains("outside the map")),
        "an off-map point warns, got {found:?}"
    );
}

/// The perf lock's sibling for stroke-heavy documents: the heaviest shipped map plus a
/// whole session of drawn lines still compiles inside the editor's edit-loop budget.
#[test]
fn a_stroke_heavy_bystra_still_fits_the_edit_loop_budget() {
    let mut blueprint = blueprint_for(MapId::BystraValley);
    for index in 0..24 {
        let x0 = 40.0 + (index as f32) * 38.0 % 900.0;
        let points: Vec<[f32; 2]> = (0..32)
            .map(|step| {
                let t = step as f32 / 31.0;
                [
                    (x0 + 60.0 * t).clamp(2.0, 998.0),
                    (60.0 + 380.0 * t + 8.0 * ((step % 4) as f32)).clamp(2.0, 998.0),
                ]
            })
            .collect();
        let clamp_at = blueprint.terrain.ops.len() - 1;
        blueprint.terrain.ops.insert(
            clamp_at,
            TerrainOp::Stroke(StrokeSpec {
                points,
                profile: StrokeProfile::Ridge { amp_m: 1.5 },
                half_width_m: 6.0,
                falloff_m: 8.0,
            }),
        );
    }
    let _warm = compile(&blueprint);
    let start = std::time::Instant::now();
    let _timed = compile(&blueprint);
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_millis() < 250,
        "24 drawn strokes on the heaviest map must stay inside the edit-loop budget \
         (took {elapsed:?})"
    );
}
