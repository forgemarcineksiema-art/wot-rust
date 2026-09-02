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
fn a_road_profile_rides_its_named_road_bit_for_bit() {
    use map_forge::blueprint::{RoadProfileSpec, RoadSpec, StrokeProfile, StrokeSpec};
    let points = vec![[40.0, 150.0], [260.0, 150.0]];
    let road = RoadSpec::Road {
        id: "lane".into(),
        surface: terrain::RoadSurface::Dirt,
        points: points.clone(),
        width_m: 8.0,
    };

    // The profile op names the road and carries NO points of its own…
    let mut profiled = flat_square();
    profiled.roads = vec![road.clone()];
    profiled.terrain.ops.push(TerrainOp::RoadProfile(RoadProfileSpec {
        road_id: "lane".into(),
        profile: StrokeProfile::Ridge { amp_m: 1.5 },
        half_width_m: 4.0,
        falloff_m: 6.0,
    }));
    let (profiled_map, profiled_report) = compile(&profiled);
    assert!(profiled_report.errors().next().is_none(), "a well-named profile compiles clean");

    // …and the compiled ground is BIT-IDENTICAL to a hand-authored stroke on the same line:
    // the resolution is the whole mechanism, and this is its lock.
    let mut stroked = flat_square();
    stroked.roads = vec![road];
    stroked.terrain.ops.push(TerrainOp::Stroke(StrokeSpec {
        points,
        profile: StrokeProfile::Ridge { amp_m: 1.5 },
        half_width_m: 4.0,
        falloff_m: 6.0,
    }));
    let (stroked_map, _) = compile(&stroked);
    assert_eq!(
        profiled_map.heightmap.samples(),
        stroked_map.heightmap.samples(),
        "RoadProfile must resolve to exactly the stroke its road draws"
    );
    let mid = profiled_map.heightmap.sample_height(150.0, 150.0).expect("inside");
    assert!((mid - 6.5).abs() < 1.0e-3, "the roadbed rides the embankment: {mid}");
}

#[test]
fn a_road_profile_naming_a_ghost_road_is_an_error_not_a_panic() {
    use map_forge::blueprint::{RoadProfileSpec, StrokeProfile};
    let mut blueprint = flat_square();
    blueprint.terrain.ops.push(TerrainOp::RoadProfile(RoadProfileSpec {
        road_id: "ghost".into(),
        profile: StrokeProfile::Ridge { amp_m: 1.0 },
        half_width_m: 4.0,
        falloff_m: 6.0,
    }));
    let (map, _) = compile(&blueprint);
    assert_eq!(
        map.heightmap.sample_height(150.0, 150.0),
        Some(5.0),
        "an unresolved profile is the identity, never a panic"
    );
    let (_, report) = compile(&blueprint);
    assert!(
        report
            .errors()
            .any(|entry| entry.check == "road_profile" && entry.message.contains("ghost")),
        "the report must name the ghost road"
    );
}

#[test]
fn a_road_profile_reaches_the_backdrop_skirt() {
    use map_forge::blueprint::{RoadProfileSpec, RoadSpec, StrokeProfile};
    // A road running to the map edge: the apron's analytic continuation must agree with
    // the compiled border EXACTLY, or the seam shows — the backdrop walks the same
    // effective op list, and this is the lock on that sentence.
    let mut blueprint = flat_square();
    blueprint.roads = vec![RoadSpec::Road {
        id: "edge_lane".into(),
        surface: terrain::RoadSurface::Dirt,
        points: vec![[0.0, 150.0], [300.0, 150.0]],
        width_m: 8.0,
    }];
    blueprint.terrain.ops.push(TerrainOp::RoadProfile(RoadProfileSpec {
        road_id: "edge_lane".into(),
        profile: StrokeProfile::Ridge { amp_m: 1.5 },
        half_width_m: 4.0,
        falloff_m: 6.0,
    }));
    let (map, _) = compile(&blueprint);
    for z in [140.0_f32, 150.0, 160.0] {
        let compiled = map.heightmap.sample_height(300.0, z).expect("border node");
        let continued = map_forge::backdrop_height(&blueprint, 300.0, z);
        assert!(
            (compiled - continued).abs() < 1.0e-4,
            "the apron must not tear at the profiled road (z {z}: {compiled} vs {continued})"
        );
    }
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

    // One authored ridge, above the fleet-derived floor (lowest hull-centre 1.19 + the
    // 0.3 m sight slack = 1.49), and the map has a fighting line: hulls hide behind it,
    // turrets work over it. Two things moved on purpose against the old fixture: 1.3 m of
    // amplitude sat below the floor (a crest that height cannot block even the lowest
    // hull-centre sight line once the graze slack is priced in), and sigma 6 was too WIDE
    // to fight from — standing close enough to see the full crest means standing on its
    // own skirt, so the RELATIVE rise never clears the floor. A fightable crest is tall
    // enough AND narrow enough; that pair is the sculpting lesson this gauge now teaches.
    let mut ridged = flat_square();
    ridged.terrain.ops.push(map_forge::blueprint::TerrainOp::Gauss1 {
        axis: map_forge::blueprint::MapAxis::Z,
        apply: map_forge::blueprint::Apply::Add,
        terms: vec![map_forge::blueprint::Gauss1Term { center: 150.0, sigma: 3.0, amp: 1.65 }],
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

/// The scenery contract, exercised at last (it was on the `data_contracts` allowlist as
/// "no test authors scenery that violates the scenery rules"). It guards an AUTHOR's two
/// mistakes, and both are warnings rather than errors because either can be deliberate: a
/// dressing point may be pushed off the map by a region that overhangs the border, and a
/// scatter with no `cover_margin_m` may drop a stone inside a barn.
///
/// This test IS the documentation of what the check means — which is the whole reason the gate
/// asks for the check's name to appear in one.
#[test]
fn scenery_is_refused_when_it_leaves_the_map_or_grows_through_cover() {
    use map_forge::blueprint::{ObjectSpec, SceneryOp, XCoord};
    use terrain::{SceneryKind, StaticCoverKind};

    // Clean first: a stone on open ground inside the map trips nothing. Without this the test
    // could pass on a check that fires for everything.
    let mut clean = flat_square();
    clean.scenery.push(SceneryOp::Fixed {
        kind: SceneryKind::Rock,
        spots: vec![[80.0, 80.0]],
        yaw_rad: 0.0,
        scale: 1.0,
    });
    let (_, report) = compile(&clean);
    assert!(
        !report.warnings().any(|entry| entry.check == "scenery"),
        "a stone on open ground inside the map is not a contract violation"
    );

    // The border half of this check is GONE, and finding that out is what this test was for:
    // an out-of-map dressing point never reaches a compiled map at all, because both scenery
    // expanders drop a point the heightmap refuses to ground. Proven here rather than asserted,
    // so the day someone adds a path that skips grounding, this fails and the guard comes back.
    let mut outside = flat_square();
    outside.scenery.push(SceneryOp::Fixed {
        kind: SceneryKind::Rock,
        spots: vec![[420.0, 80.0]],
        yaw_rad: 0.0,
        scale: 1.0,
    });
    let (map, report) = compile(&outside);
    assert!(
        map.scenery.is_empty(),
        "grounding is the real guard: an off-map dressing point must never be emitted"
    );
    assert!(
        !report.warnings().any(|entry| entry.check == "scenery"),
        "nothing was emitted, so there is nothing to warn about"
    );

    // Through a cover footprint: a tree growing out of a barn's roof.
    let mut inside_cover = flat_square();
    inside_cover.objects.push(ObjectSpec::Cover {
        id: "barn".into(),
        name: "Barn".into(),
        kind: StaticCoverKind::FarmBuilding,
        at: [XCoord::Fixed(150.0), XCoord::Fixed(200.0)],
        half_extents_m: [6.0, 4.0, 9.0],
    });
    inside_cover.scenery.push(SceneryOp::Fixed {
        kind: SceneryKind::Oak,
        spots: vec![[150.0, 200.0]],
        yaw_rad: 0.0,
        scale: 1.0,
    });
    let (_, report) = compile(&inside_cover);
    assert!(
        report
            .warnings()
            .any(|entry| entry.check == "scenery"
                && entry.message.contains("through a cover footprint")),
        "a tree standing inside a barn must be reported"
    );
}

/// The drive graph speaks for a hull, not a point: a slot wide enough for a grid node to
/// clear the old hand-written 0.3 m margin but far too narrow for the fleet's widest hull
/// must refuse the route. The east wall leaves a 2.4 m slot centred on the node at z = 150
/// — exactly the geometry the old gate certified and the battle then walled off.
#[test]
fn a_gap_narrower_than_the_widest_hull_does_not_certify_a_route() {
    use map_forge::blueprint::{ObjectSpec, StrategicPointSpec, XCoord};
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
    // The east wall in two runs, leaving the 2.4 m slot centred on the node at z = 150.
    walled.objects.push(wall("wall_e_n", 170.0, 159.4, [2.0, 3.0, 8.2]));
    walled.objects.push(wall("wall_e_s", 170.0, 140.6, [2.0, 3.0, 8.2]));
    walled.gameplay.strategic_points.push(StrategicPointSpec {
        id: "courtyard".into(),
        name: "the slotted yard".into(),
        role: terrain::StrategicRole::Observation,
        at: [XCoord::Fixed(150.0), XCoord::Fixed(150.0)],
        radius_m: 10.0,
    });
    let (_, report) = compile(&walled);
    assert!(
        report
            .errors()
            .any(|entry| entry.check == "playability" && entry.message.contains("unreachable")),
        "a 2.4 m slot is a wall to a 3.5 m hull - the route must be refused"
    );
}

/// The margin is the fleet's own measurement, not a number someone typed: the widest
/// hull's half-width, so a new wider vehicle moves the gate without anyone remembering to.
#[test]
fn the_passability_margin_is_the_fleet_measurement() {
    let widest = game_core::VehicleKind::ALL
        .iter()
        .map(|kind| kind.spec().hitbox.half_width_m)
        .fold(0.0, f32::max);
    assert!(widest > 1.5, "the fleet fields real hulls (got {widest})");
    assert_eq!(map_forge::cover_passability_margin_m(), widest);
}

/// The census floor is the fleet's own measurement composed with the sim's sight slack —
/// never a number someone typed: the lowest hull-centre plus SIGHT_GRAZE_SLACK_M. A crest
/// below this sum cannot block even the lowest vehicle's hull-centre sight line, so a
/// census that blessed it would be steering the brush (and the bots) at half-cover.
#[test]
fn the_census_rise_floor_is_the_fleet_measurement() {
    let lowest_centre = game_core::VehicleKind::ALL
        .iter()
        .map(|kind| kind.spec().hitbox.center_y_m)
        .fold(f32::INFINITY, f32::min);
    assert!(lowest_centre > 1.0, "the fleet fields real hulls (got {lowest_centre})");
    assert_eq!(map_forge::hull_down_rise_min_m(), lowest_centre + game_core::SIGHT_GRAZE_SLACK_M);
}

/// Inny Poziom F2 — the monoculture gate (`species_mix`), tripped three ways and cleared once.
/// A dressed map plants at least three tree species, no species is more than 70 % of its
/// trees, and every species its horizon names at a tenth or more stands inside the map. Below
/// `DRESSED_MAP_TREES` the check warns instead of failing: a fixture with five oaks is
/// undressed, not a monoculture. This test IS the documentation of what the check means.
#[test]
fn species_mix_refuses_a_monoculture_and_a_horizon_the_map_does_not_plant() {
    use map_forge::blueprint::{HorizonSpec, SceneryOp};
    use terrain::SceneryKind;

    let spots = |count: usize, x: f32| -> Vec<[f32; 2]> {
        (0..count).map(|index| [x, 40.0 + 18.0 * index as f32]).collect()
    };
    let fixed = |kind: SceneryKind, spots: Vec<[f32; 2]>| SceneryOp::Fixed {
        kind,
        spots,
        yaw_rad: 0.0,
        scale: 1.0,
    };
    let species_mix = |report: &map_forge::MapReport| -> Vec<String> {
        report
            .errors()
            .filter(|entry| entry.check == "species_mix")
            .map(|entry| entry.message.clone())
            .collect()
    };

    // Undressed: five oaks warn and never fail — the rule bites at DRESSED_MAP_TREES.
    let mut sparse = flat_square();
    sparse.scenery.push(fixed(SceneryKind::Oak, spots(5, 60.0)));
    let (map, report) = compile(&sparse);
    assert!(map.scenery.len() < map_forge::DRESSED_MAP_TREES, "five spots stay undressed");
    assert!(species_mix(&report).is_empty(), "an undressed map is not a monoculture");
    assert!(
        report.warnings().any(|entry| entry.check == "species_mix"),
        "...but the author is told it is undressed"
    );

    // A dressed monoculture: fourteen oaks and nothing else — too few species AND one over 70 %.
    let mut mono = flat_square();
    mono.scenery.push(fixed(SceneryKind::Oak, spots(14, 60.0)));
    let (_, report) = compile(&mono);
    let errors = species_mix(&report);
    assert!(
        errors.iter().any(|message| message.contains("tree species planted")),
        "fewer than three species is refused: {errors:?}"
    );
    assert!(
        errors.iter().any(|message| message.contains("Oak is 100 %")),
        "a species over 70 % is refused: {errors:?}"
    );

    // Dressed and mixed: six oaks, five poplars, four willows — clean, and the shape every
    // shipped map now has.
    let mixed = || {
        let mut blueprint = flat_square();
        blueprint.scenery.push(fixed(SceneryKind::Oak, spots(6, 60.0)));
        blueprint.scenery.push(fixed(SceneryKind::Poplar, spots(5, 120.0)));
        blueprint.scenery.push(fixed(SceneryKind::Willow, spots(4, 180.0)));
        blueprint
    };
    let (map, report) = compile(&mixed());
    assert!(map.scenery.len() >= map_forge::DRESSED_MAP_TREES, "fifteen spots dress the map");
    assert!(
        !report.entries.iter().any(|entry| entry.check == "species_mix"),
        "three species under 70 % each is the contract: {:?}",
        species_mix(&report)
    );

    // The horizon grows pines at half weight and the map plants none: the ring past the
    // border would invent a country the map does not have.
    let mut ring = mixed();
    ring.horizon = Some(HorizonSpec {
        hills_base_m: 20.0,
        swell: [0.0; 4],
        x_roll: [0.0; 2],
        z_roll: [0.0; 2],
        closure_start_m: 40.0,
        closure_span_m: 80.0,
        river_gap_half_m: 0.0,
        river_gap_falloff_m: 1.0,
        flora: vec![(SceneryKind::Pine, 0.5), (SceneryKind::Oak, 0.5)],
    });
    let (_, report) = compile(&ring);
    let errors = species_mix(&report);
    assert!(
        errors.iter().any(|message| message.contains("Pine")),
        "a horizon species the map does not plant is refused: {errors:?}"
    );
}
