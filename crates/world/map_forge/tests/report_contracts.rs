//! Locks for the contract report itself: the checks that guard an AUTHOR's mistakes must
//! fire on a synthetic bad blueprint — a shipped map never exercises them (it ships clean).

use game_core::WeatherVariant;
use map_forge::blueprint::{
    BaseSpec, EnvironmentSpec, Gauss2Term, GridSpec, GroundLayerSpec, GroundMaterialsSpec,
    LightingPreset, LookSpec, MapBlueprint, MetaSpec, SpawnSpec, SymmetrySpec, TerrainOp,
    TerrainProgram,
};
use map_forge::compile;
use terrain::RiverSpec;

/// A minimal valid document to break in one deliberate way per test.
fn flat_square() -> MapBlueprint {
    MapBlueprint {
        meta: MetaSpec {
            version: map_forge::blueprint::BLUEPRINT_VERSION,
            id: "report_probe".into(),
            name: "Report probe".into(),
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

fn error_messages(blueprint: &MapBlueprint) -> Vec<String> {
    let (_, report) = compile(blueprint);
    report.errors().map(|entry| format!("{}: {}", entry.check, entry.message)).collect()
}

#[test]
fn a_rectangular_grid_is_refused_not_silently_miscompiled() {
    let mut blueprint = flat_square();
    blueprint.grid.size_m = [300.0, 400.0];
    let errors = error_messages(&blueprint);
    assert!(
        errors.iter().any(|message| message.starts_with("grid:") && message.contains("square")),
        "a rectangular grid must be a report error, got: {errors:?}"
    );
}

#[test]
fn a_river_off_the_symmetry_axis_is_refused() {
    let mut blueprint = flat_square();
    blueprint.symmetry = Some(SymmetrySpec::MirrorZ);
    blueprint.river = Some(RiverSpec {
        base_x_m: 200.0,
        axis_z_m: 100.0, // the grid's symmetry axis is 150.0
        bow_sigma_m: 50.0,
        bow_amp_m: 10.0,
        wiggle_amp_m: 3.0,
        wiggle_wave_m: 40.0,
        corridor_half_width_m: 20.0,
    });
    let errors = error_messages(&blueprint);
    assert!(
        errors.iter().any(|message| message.starts_with("grid:") && message.contains("axis")),
        "a river off the symmetry axis must be a report error, got: {errors:?}"
    );
}

/// The art-direction ground window is a report contract: a neon vegetation layer (the
/// field-patch lift would push it past soil) is refused, not shipped.
#[test]
fn a_neon_ground_layer_is_refused_by_the_saturation_window() {
    let mut blueprint = flat_square();
    let soil = GroundLayerSpec { albedo: [0.30, 0.30, 0.22], detail: 1.0, gloss: 0.03 };
    blueprint.materials = Some(GroundMaterialsSpec {
        // A screaming green lawn: saturation and lift both leave the window.
        layers: [
            GroundLayerSpec { albedo: [0.10, 0.70, 0.05], detail: 1.0, gloss: 0.03 },
            soil,
            soil,
            soil,
        ],
        macro_normal_strength: 0.65,
        field_patch_strength: 1.0,
    });
    let errors = error_messages(&blueprint);
    assert!(
        errors.iter().any(|message| message.starts_with("materials:")),
        "a neon layer must be a report error, got: {errors:?}"
    );
}

/// One look per variant: authoring the same sky twice is a coherence error, not a shrug.
#[test]
fn a_twice_authored_weather_variant_is_refused() {
    let mut blueprint = flat_square();
    let look = LookSpec {
        variant: WeatherVariant::ClearAfternoon,
        preset: LightingPreset::HazyNoon,
        sky_rgb: [0.55, 0.69, 0.87],
        rain_intensity: 0.0,
        wetness: 0.0,
        overrides: Default::default(),
    };
    blueprint.environment = Some(EnvironmentSpec { looks: vec![look, look] });
    let errors = error_messages(&blueprint);
    assert!(
        errors
            .iter()
            .any(|message| message.starts_with("environment:") && message.contains("twice")),
        "a duplicate variant must be a report error, got: {errors:?}"
    );
}

/// The M3 prerequisite: author input never panics the compiler. A river-relative op on a
/// riverless map compiles (the mask vanishes) and the REPORT carries the Error — a live
/// editor session survives every keystroke.
#[test]
fn river_ops_on_a_riverless_map_compile_and_error_instead_of_panicking() {
    let mut blueprint = flat_square();
    blueprint.terrain.ops.push(TerrainOp::CarveChannel {
        half_width_m: 10.0,
        falloff_m: 8.0,
        water_level_m: 3.0,
        channel_depth_m: 2.0,
        sill_depth_m: 0.6,
        sills: vec![],
    });
    let (map, report) = compile(&blueprint);
    // The op is a no-op without a river: the ground stays the flat base.
    assert_eq!(map.heightmap.sample_height(150.0, 150.0), Some(5.0));
    let errors: Vec<String> =
        report.errors().map(|entry| format!("{}: {}", entry.check, entry.message)).collect();
    assert!(
        errors.iter().any(|message| message.starts_with("river:")),
        "a riverless CarveChannel must be a report error, got: {errors:?}"
    );
}

/// Same rule for coordinates: `RiverCenter` without a river (or in the z slot) resolves to
/// the map's centre and errors in the report — never a panic.
#[test]
fn river_center_coordinates_on_a_riverless_map_error_instead_of_panicking() {
    let mut blueprint = flat_square();
    blueprint.objects.push(map_forge::blueprint::ObjectSpec::Cover {
        id: "adrift".into(),
        name: "A cover with no river to ride".into(),
        kind: terrain::StaticCoverKind::WoodenFence,
        at: [map_forge::blueprint::XCoord::RiverCenter, map_forge::blueprint::XCoord::Fixed(150.0)],
        half_extents_m: [1.0, 0.6, 1.0],
    });
    let (map, report) = compile(&blueprint);
    let cover = map.static_cover.iter().find(|cover| cover.id == "adrift").expect("compiled");
    assert_eq!(cover.center[0], 150.0, "the fallback is the map centre");
    assert!(
        report.errors().any(|entry| entry.check == "river"),
        "a riverless RiverCenter must be a report error"
    );
}

/// The gentle-approach rule holds on DRY maps too — a cliff at the spawn must not hide
/// behind the absence of water (it did once: the probe lived inside the water branch).
#[test]
fn a_steep_spawn_approach_warns_even_on_a_dry_map() {
    let mut blueprint = flat_square();
    blueprint.terrain.ops.push(TerrainOp::Gauss2 {
        apply: map_forge::blueprint::Apply::Add,
        terms: vec![Gauss2Term { x: 150.0, z: 172.0, sx: 8.0, sz: 8.0, amp: 9.0 }],
    });
    let (map, report) = compile(&blueprint);
    assert!(map.water.is_none(), "the probe map must stay dry");
    let warnings: Vec<&str> = report
        .warnings()
        .filter(|entry| entry.check == "spawns")
        .map(|entry| entry.message.as_str())
        .collect();
    assert!(
        warnings.iter().any(|message| message.contains("steep")),
        "a cliff at a dry spawn must warn, got: {warnings:?}"
    );
}

/// The D1 sculpt layer: the delta lands in the compiled heightmap (re-clamped to the
/// floor), and the contract refuses a sculpted border, a non-canonical list, and a broken
/// mirror on a fair map.
#[test]
fn the_sculpt_layer_applies_and_its_contract_bites() {
    use map_forge::blueprint::SculptSpec;
    let side = 61_u32; // 300 m / 5 m + 1
    let index = |xi: u32, zi: u32| zi * side + xi;

    let mut blueprint = flat_square();
    blueprint.sculpt = Some(SculptSpec {
        step_m: 0.05,
        samples: vec![(index(30, 30), 40), (index(31, 30), -200)],
    });
    let (map, report) = compile(&blueprint);
    assert!(!report.errors().any(|entry| entry.check == "sculpt"), "a clean layer passes");
    let h = |xi: u32, zi: u32| map.heightmap.sample_at_index(xi as usize, zi as usize);
    assert!((h(30, 30) - 7.0).abs() < 1.0e-4, "raise: 5 + 40*0.05, got {}", h(30, 30));
    assert!((h(31, 30) - 0.2).abs() < 1.0e-4, "a dig clamps to the floor, got {}", h(31, 30));
    assert_eq!(h(10, 10), 5.0, "unsculpted ground is untouched");

    let refused = |sculpt: SculptSpec, why: &str| {
        let mut blueprint = flat_square();
        blueprint.sculpt = Some(sculpt);
        let (_, report) = compile(&blueprint);
        assert!(
            report.errors().any(|entry| entry.check == "sculpt"),
            "{why} must be a sculpt error"
        );
    };
    refused(SculptSpec { step_m: 0.05, samples: vec![(index(0, 12), 10)] }, "a sculpted border");
    refused(
        SculptSpec { step_m: 0.05, samples: vec![(index(31, 30), 1), (index(30, 30), 1)] },
        "an unsorted list",
    );
    refused(SculptSpec { step_m: 5.0, samples: vec![] }, "a wild quantum");
    refused(
        SculptSpec { step_m: 0.05, samples: vec![(side * side + 7, 1)] },
        "an index off the grid",
    );

    let mut fair = flat_square();
    fair.symmetry = Some(SymmetrySpec::MirrorZ);
    fair.sculpt = Some(SculptSpec { step_m: 0.05, samples: vec![(index(30, 20), 10)] });
    let (_, report) = compile(&fair);
    assert!(
        report.errors().any(|entry| entry.check == "sculpt" && entry.message.contains("mirror")),
        "a one-sided stroke on a fair map must be refused"
    );
}

/// M7 playability: a strategic point walled off by cover is an Error with a position; a
/// starved nav skeleton on a big map warns; a crossing window stripped of its Crossing
/// point is refused on the shipped valley itself.
#[test]
fn playability_bites_walls_starvation_and_unnamed_crossings() {
    use map_forge::blueprint::{ObjectSpec, StrategicPointSpec, XCoord};

    // A courtyard of cover with a point inside: unreachable from the spawns outside.
    let mut walled = flat_square();
    let wall = |id: &str, x: f32, z: f32, half: [f32; 3]| ObjectSpec::Cover {
        id: id.into(),
        name: "wall".into(),
        kind: terrain::StaticCoverKind::FarmBuilding,
        at: [XCoord::Fixed(x), XCoord::Fixed(z)],
        half_extents_m: half,
    };
    walled.objects.push(wall("wall_n", 150.0, 130.0, [20.0, 3.0, 2.0]));
    walled.objects.push(wall("wall_s", 150.0, 170.0, [20.0, 3.0, 2.0]));
    walled.objects.push(wall("wall_w", 130.0, 150.0, [2.0, 3.0, 20.0]));
    walled.objects.push(wall("wall_e", 170.0, 150.0, [2.0, 3.0, 20.0]));
    walled.gameplay.strategic_points.push(StrategicPointSpec {
        id: "courtyard".into(),
        name: "the walled yard".into(),
        role: terrain::StrategicRole::Observation,
        at: [XCoord::Fixed(150.0), XCoord::Fixed(150.0)],
        radius_m: 10.0,
    });
    let (_, report) = compile(&walled);
    let unreachable: Vec<_> = report
        .errors()
        .filter(|entry| entry.check == "playability" && entry.message.contains("unreachable"))
        .collect();
    assert!(!unreachable.is_empty(), "a walled-off point must be a playability Error");
    assert!(unreachable[0].at.is_some(), "the Error carries a jump-to position");

    // ...and it NAMES the wall. Telling an author their point is cut off, while leaving them to
    // hunt through everything they placed for the culprit, is half a report. The courtyard is
    // ringed by four objects and the line must name them by id, nearest first.
    let named = &unreachable[0].message;
    assert!(
        named.contains("walled in by"),
        "the report must name what cut the point off, got: {named}"
    );
    let walls_named =
        ["wall_n", "wall_s", "wall_w", "wall_e"].iter().filter(|id| named.contains(**id)).count();
    assert!(
        walls_named > 0,
        "the named blockers must be the courtyard walls themselves, got: {named}"
    );
    assert!(
        named.contains("FarmBuilding"),
        "naming the KIND too tells the author what they are looking at, got: {named}"
    );

    // Starvation: a big empty map warns that the route planner will starve.
    let mut starved = flat_square();
    starved.grid.size_m = [600.0, 600.0];
    starved.gameplay.spawns[0].at = [300.0, 80.0];
    starved.gameplay.spawns[1].at = [300.0, 520.0];
    let (_, report) = compile(&starved);
    assert!(
        report
            .warnings()
            .any(|entry| entry.check == "playability" && entry.message.contains("starves")),
        "a starved nav skeleton must warn"
    );

    // The shipped valley with one Crossing point stripped: the window loses its name.
    let mut valley = map_forge::blueprint_for(terrain::MapId::BystraValley);
    let before = valley.gameplay.strategic_points.len();
    valley.gameplay.strategic_points.retain(|point| point.id != "bridge_crossing");
    if valley.gameplay.strategic_points.len() == before {
        // The id drifted - drop the first Crossing-role point instead.
        let index = valley
            .gameplay
            .strategic_points
            .iter()
            .position(|point| point.role == terrain::StrategicRole::Crossing)
            .expect("the valley names its crossings");
        valley.gameplay.strategic_points.remove(index);
    }
    let (_, report) = compile(&valley);
    assert!(
        report
            .errors()
            .any(|entry| entry.check == "playability" && entry.message.contains("Crossing")),
        "an unnamed crossing window must be refused"
    );
}

/// THE HULL-DOWN CENSUS: the gauge the terrain-density withdrawal demanded. "Author the relief
/// where the fight happens" is a claim with no number until something counts the places a tank
/// can actually fight from — this is that count, and the sculpting sessions aim at its floor.
#[test]
fn the_census_counts_fightable_crests_and_the_report_warns_below_the_floor() {
    // A flat plain has nowhere to fight from, and the report says so by name.
    let flat = flat_square();
    let (map, report) = map_forge::compile(&flat);
    assert!(map_forge::hull_down_positions(&map).is_empty(), "a plain has no crests");
    assert!(
        report.warnings().any(|entry| entry.check == "hull_down"),
        "below the floor the report must say so, with the count"
    );

    // One authored ridge — 1.3 m, exactly what the Ridge brush lays down — and the map has a
    // fighting line: hulls hide behind it on both sides, turrets work over it.
    let mut ridged = flat_square();
    ridged.terrain.ops.push(map_forge::blueprint::TerrainOp::Gauss1 {
        axis: map_forge::blueprint::MapAxis::Z,
        apply: map_forge::blueprint::Apply::Add,
        terms: vec![map_forge::blueprint::Gauss1Term { center: 150.0, sigma: 6.0, amp: 1.3 }],
    });
    let (map, report) = map_forge::compile(&ridged);
    let spots = map_forge::hull_down_positions(&map);
    assert!(
        spots.len() >= 12,
        "a 300 m ridge line carries positions along its whole length, got {}",
        spots.len()
    );
    assert!(
        !report.warnings().any(|entry| entry.check == "hull_down"),
        "above the floor the gauge is silent"
    );
    // Every position faces its crest: the facing is the census's gift to the bots.
    for spot in &spots {
        let len = (spot.facing[0].powi(2) + spot.facing[1].powi(2)).sqrt();
        assert!((len - 1.0).abs() < 1.0e-4, "facing is a unit direction");
    }

    // A 3 m WALL is not a position: the rise band rejects the near approaches and the
    // beyond-the-crest drop rejects the far ones — a slope that keeps climbing is terrain
    // refusing you, not covering you.
    let mut walled = flat_square();
    walled.terrain.ops.push(map_forge::blueprint::TerrainOp::Gauss1 {
        axis: map_forge::blueprint::MapAxis::Z,
        apply: map_forge::blueprint::Apply::Add,
        terms: vec![map_forge::blueprint::Gauss1Term { center: 150.0, sigma: 6.0, amp: 3.0 }],
    });
    let (map, _) = map_forge::compile(&walled);
    assert!(
        map_forge::hull_down_positions(&map).is_empty(),
        "a wall must count for nothing — the census is a gauge of cover, not of steepness"
    );
}
