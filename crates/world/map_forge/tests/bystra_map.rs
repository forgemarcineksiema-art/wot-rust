use map_forge::{backdrop_height, battlefield, cached_blueprint_by_id};
use terrain::{MapId, StrategicRole, bystra_river_center_x};

const MAP_M: f32 = 1000.0;
const HALF_M: f32 = 500.0;

/// The fairness contract: both halves of the valley are the same ground. Every terrain
/// component is an even function about the central axis or a mirrored pair, so this holds by
/// construction — and stays held.
#[test]
fn bystra_heightmap_is_mirror_symmetric_across_the_central_axis() {
    let map = battlefield(MapId::BystraValley);
    let mut max_delta = 0.0_f32;
    for zi in 0..=50 {
        for xi in 0..=50 {
            let x = xi as f32 * (MAP_M / 50.0);
            let z = zi as f32 * (MAP_M / 50.0);
            let here = map.heightmap.sample_height(x, z).unwrap();
            let mirrored = map.heightmap.sample_height(x, MAP_M - z).unwrap();
            max_delta = max_delta.max((here - mirrored).abs());
        }
    }
    assert!(max_delta < 1.0e-3, "mirror symmetry broke: max delta {max_delta}");
}

/// Layout fairness: spawns, strategic points and cover are on-axis or mirrored pairs.
#[test]
fn bystra_layout_is_mirrored_or_on_axis() {
    let map = battlefield(MapId::BystraValley);

    assert_eq!(map.spawn_zones.len(), 2);
    let south = &map.spawn_zones[0];
    let north = &map.spawn_zones[1];
    assert!((south.center[0] - north.center[0]).abs() < 1.0e-3);
    assert!((south.center[2] - (MAP_M - north.center[2])).abs() < 1.0e-3);

    for point in &map.strategic_points {
        let z = point.position[2];
        if (z - HALF_M).abs() < 1.0 {
            continue; // on-axis
        }
        let twin = map.strategic_points.iter().find(|other| {
            other.role == point.role
                && (other.position[0] - point.position[0]).abs() < 1.0
                && (other.position[2] - (MAP_M - z)).abs() < 1.0
        });
        assert!(twin.is_some(), "strategic point {} has no mirror twin", point.id);
    }

    for cover in &map.static_cover {
        let z = cover.center[2];
        if (z - HALF_M).abs() < 1.0 {
            continue; // on-axis
        }
        let twin = map.static_cover.iter().find(|other| {
            other.kind == cover.kind
                && (other.center[0] - cover.center[0]).abs() < 1.0
                && (other.center[2] - (MAP_M - z)).abs() < 1.0
                && (other.half_extents_m == cover.half_extents_m)
        });
        assert!(twin.is_some(), "cover {} has no mirror twin", cover.id);
    }
}

#[test]
fn bystra_scale_water_and_anatomy_are_declared() {
    let map = battlefield(MapId::BystraValley);
    assert_eq!(map.id, "bystra_valley");
    assert_eq!(map.size_m, [MAP_M, MAP_M]);
    let water = map.water.expect("the Bystra is the map");
    assert!((water.surface_level_m - 5.0).abs() < 1.0e-6);

    assert!(map.strategic_points.len() >= 10, "the bots need a dense navigation skeleton");
    let crossings =
        map.strategic_points.iter().filter(|point| point.role == StrategicRole::Crossing).count();
    assert_eq!(crossings, 5, "bridge + two fords + two plank crossings");
    assert!(map.static_cover.len() >= 30, "the town alone is a block grid");

    let stats = map.heightmap.stats();
    assert!(stats.max_m - stats.min_m > 20.0, "a valley has real relief");
}

// The two sight-line contracts (the Windmill crest overwatch and the shelf hull-down)
// moved to `crates/runtime/sim/tests/bystra_hull_down.rs`: the local `sight_clear` copy
// they leaned on carried the OPPOSITE slack sign to the sim's spotting rule and ignored
// cover boxes, so it certified promises the live game did not keep (its crest probe even
// stood inside the windmill tower). A sight promise is certified by `sim::line_of_sight`
// or it is not certified at all.

/// Spawns are dry, gentle ground — nobody deploys into the river or onto a cliff.
#[test]
fn spawns_sit_on_dry_gentle_ground() {
    let map = battlefield(MapId::BystraValley);
    let water = map.water.unwrap();
    for zone in &map.spawn_zones {
        for (dx, dz) in [(0.0, 0.0), (-25.0, -25.0), (25.0, -25.0), (-25.0, 25.0), (25.0, 25.0)] {
            let x = zone.center[0] + dx;
            let z = zone.center[2] + dz;
            let h = map.heightmap.sample_height(x, z).unwrap();
            assert!(water.depth_over(h) == 0.0, "spawn ground at ({x}, {z}) is wet (h = {h})");
            let ahead = map.heightmap.sample_height(x, z + 10.0).unwrap();
            assert!(((ahead - h) / 10.0).abs() < 0.2, "spawn approach at ({x}, {z}) is too steep");
        }
    }
}

/// The town bench is honestly flat along its ramp: a street between the block columns stays
/// on grade, so houses sit clean and hulls drive streets, not moguls.
#[test]
fn kamienna_streets_hold_the_bench_ramp() {
    let map = battlefield(MapId::BystraValley);
    let street_x = 753.0; // between block columns 732 and 774
    let expected = map.heightmap.sample_height(street_x, HALF_M).unwrap();
    let mut dz = -150.0_f32;
    while dz <= 150.0 {
        let h = map.heightmap.sample_height(street_x, HALF_M + dz).unwrap();
        assert!((h - expected).abs() < 0.8, "street height wobbles at dz {dz}: {h} vs {expected}");
        dz += 10.0;
    }
}

/// The backdrop skirt of THIS map's blueprint (the horizon enclosure lives in the document).
fn skirt_at(x: f32, z: f32) -> f32 {
    backdrop_height(cached_blueprint_by_id("bystra_valley").expect("bystra is shipped"), x, z)
}

/// The dressing plays fair too: every scenery instance is mirrored (position + kind + scale),
/// nothing grows in the river corridor, the crossing lanes, or the spawn circles, and the
/// total stays inside the render budget.
#[test]
fn bystra_scenery_is_mirrored_excluded_and_budgeted() {
    let map = battlefield(MapId::BystraValley);
    // The hero-flora program (2026-08-05) thinned the dressing on purpose: the procedural
    // willows and poplars are retired and the hero oak stands sparsely, so "dressed" now
    // means rocks plus a handful of landmark oaks (~40 instances), not a forest.
    assert!(map.scenery.len() >= 30, "the valley is dressed, got {}", map.scenery.len());
    assert!(map.scenery.len() <= 1200, "instance budget breached: {}", map.scenery.len());
    assert!(map.scenery.len().is_multiple_of(2), "mirrored pairs come in twos");

    for instance in &map.scenery {
        let [x, _, z] = instance.position;
        let twin = map.scenery.iter().find(|other| {
            other.kind == instance.kind
                && (other.position[0] - x).abs() < 1.0e-3
                && (other.position[2] - (MAP_M - z)).abs() < 1.0e-3
                && (other.scale - instance.scale).abs() < 1.0e-6
        });
        assert!(twin.is_some(), "scenery at ({x}, {z}) has no mirror twin");

        let river_d = (x - bystra_river_center_x(z)).abs();
        assert!(
            river_d > terrain::RIVER_CORRIDOR_HALF_WIDTH_M + 1.5,
            "scenery grows in the river at ({x}, {z})"
        );
        for spawn_z in [150.0_f32, 850.0] {
            let d2 = (x - 400.0).powi(2) + (z - spawn_z).powi(2);
            assert!(d2 > 65.0 * 65.0, "scenery inside a spawn circle at ({x}, {z})");
        }
    }

    // Determinism: the same map builds the same forest, twig for twig.
    assert_eq!(map.scenery, battlefield(MapId::BystraValley).scenery);
}

/// The backdrop skirt is the SAME analytic surface as the playfield: at every heightmap grid
/// point on the border the two agree exactly, so the horizon meets the map without a seam.
/// Beyond the border the enclosure rises - except along the river gap, which stays low so the
/// Bystra visibly continues.
#[test]
fn the_backdrop_meets_the_map_exactly_and_closes_the_valley() {
    let map = battlefield(MapId::BystraValley);
    let mut probe = 0.0_f32;
    while probe <= MAP_M {
        for (x, z) in [(probe, 0.0), (probe, MAP_M), (0.0, probe), (MAP_M, probe)] {
            let edge = map.heightmap.sample_height(x, z).unwrap();
            let skirt = skirt_at(x, z);
            assert!(
                (edge - skirt).abs() < 1.0e-3,
                "seam at ({x}, {z}): heightmap {edge} vs backdrop {skirt}"
            );
        }
        probe += 5.0;
    }

    // Far outside, the hills close the view on the dry sides...
    assert!(skirt_at(-400.0, 500.0) > 16.0);
    assert!(skirt_at(1400.0, 500.0) > 16.0);
    // ...and the river gap stays below the water level so the Bystra flows out.
    let far_river_x = bystra_river_center_x(-400.0);
    assert!(skirt_at(far_river_x, -400.0) < 5.0);
    let far_river_x = bystra_river_center_x(1400.0);
    assert!(skirt_at(far_river_x, 1400.0) < 5.0);
    // Physics never follows the eye out there.
    assert!(map.heightmap.sample_height(-10.0, 500.0).is_none());
}

/// The fightable-crest inventory, ratcheted: the W2 approach sculpt (census-band lips on
/// the east bank, the 3 m levees as movement masks) must never quietly erode. The floor is
/// the measured count minus a hair of slack; burn UPWARD by sculpting, never down.
#[test]
fn bystra_carries_a_fightable_crest_inventory() {
    let map = battlefield(MapId::BystraValley);
    let spots = map_forge::hull_down_positions(&map).len();
    assert!(spots >= 28, "the crest inventory eroded: {spots} against the measured 32");
}
