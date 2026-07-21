//! Contract tests for Prokhorovka's render-only dressing (scenery + roads): the steppe is
//! dressed, the dressing is mirror-fair like the ground it stands on, and it respects the
//! map's readability rules (bare killzone, bare railbed, clear spawn circles, no growth
//! through cover or across a road).

use map_forge::battlefield;
use terrain::{MapId, RoadSurface, SceneryKind};

const MAP_SIZE_M: f32 = 1000.0;
const HALF_M: f32 = MAP_SIZE_M * 0.5;

#[test]
fn the_steppe_is_dressed_with_bushes() {
    let map = battlefield(MapId::ProkhorovkaHill252_2);
    let bushes = map.scenery.iter().filter(|s| s.kind == SceneryKind::Bush).count();
    assert!(bushes >= 80, "the steppe reads as a field, not a lawn: {bushes} bushes");
}

#[test]
fn scenery_is_mirror_paired() {
    let map = battlefield(MapId::ProkhorovkaHill252_2);
    assert!(!map.scenery.is_empty());
    for instance in &map.scenery {
        let [x, _, z] = instance.position;
        let mirrored_z = MAP_SIZE_M - z;
        let twin = map.scenery.iter().any(|other| {
            other.kind == instance.kind
                && (other.position[0] - x).abs() < 0.01
                && (other.position[2] - mirrored_z).abs() < 0.01
        });
        assert!(twin, "{:?} at ({x}, {z}) has no northern/southern twin", instance.kind);
    }
}

#[test]
fn the_western_killzone_stays_nearly_bare() {
    let map = battlefield(MapId::ProkhorovkaHill252_2);
    // The Psel field (x < 240) is designed as a coverless killzone: the dressing must not
    // suggest concealment that is not there. The treeline in-fill oaks are the exception —
    // they stand inside real TreeLine cover.
    let field_bushes =
        map.scenery.iter().filter(|s| s.kind == SceneryKind::Bush && s.position[0] < 240.0).count();
    assert_eq!(field_bushes, 0, "the killzone must stay bare, found {field_bushes} bushes");
}

#[test]
fn nothing_grows_on_the_railbed_or_roads() {
    let map = battlefield(MapId::ProkhorovkaHill252_2);
    for instance in &map.scenery {
        let [x, _, z] = instance.position;
        assert!(
            (z - HALF_M).abs() >= 20.0,
            "{:?} at ({x}, {z}) grows on the embankment",
            instance.kind
        );
        for road in &map.roads {
            assert!(
                road.distance_to(x, z) >= road.width_m * 0.5,
                "{:?} at ({x}, {z}) grows on road {}",
                instance.kind,
                road.id
            );
        }
    }
}

#[test]
fn roads_are_mirror_fair() {
    let map = battlefield(MapId::ProkhorovkaHill252_2);
    assert!(map.roads.len() >= 3, "the steppe has its railway and dirt roads");
    assert!(map.roads.iter().any(|road| road.surface == RoadSurface::Ballast));
    for road in &map.roads {
        // Either every waypoint lies on the axis (self-mirrored), or an exact twin road
        // exists with every waypoint reflected across it.
        let on_axis = road.points.iter().all(|p| (p[1] - HALF_M).abs() < 0.01);
        if on_axis {
            continue;
        }
        let twin = map.roads.iter().any(|other| {
            other.id != road.id
                && other.points.len() == road.points.len()
                && other.points.iter().zip(&road.points).all(|(a, b)| {
                    (a[0] - b[0]).abs() < 0.01 && (a[1] - (MAP_SIZE_M - b[1])).abs() < 0.01
                })
        });
        assert!(twin, "road {} has no mirror twin", road.id);
    }
}

#[test]
fn road_distance_measures_to_the_polyline() {
    let map = battlefield(MapId::ProkhorovkaHill252_2);
    let ballast = map.roads.iter().find(|road| road.surface == RoadSurface::Ballast).unwrap();
    assert!(ballast.distance_to(500.0, 500.0) < 0.01);
    assert!((ballast.distance_to(500.0, 480.0) - 20.0).abs() < 0.01);
}

/// Fizyczny Świat P10: the yard fences exist, are crushable matter, and — like everything on
/// the steppe — stand in exact north/south mirror pairs so both teams drive the same yards.
#[test]
fn yard_fences_are_crushable_and_mirror_paired() {
    let map = battlefield(MapId::ProkhorovkaHill252_2);
    let fences: Vec<_> = map
        .static_cover
        .iter()
        .filter(|cover| cover.kind == terrain::StaticCoverKind::WoodenFence)
        .collect();
    assert!(fences.len() >= 4, "the farmyards are fenced: {}", fences.len());
    for fence in &fences {
        assert!(fence.kind.is_crushable(), "a hull drives through a fence");
        assert!(fence.kind.max_health().unwrap_or(u32::MAX) <= 80, "one shell sweeps a span");
        assert!(!fence.kind.leaves_rubble(), "a fence leaves no blocking mound");
        let mirrored_z = MAP_SIZE_M - fence.center[2];
        assert!(
            fences.iter().any(|other| (other.center[0] - fence.center[0]).abs() < 0.01
                && (other.center[2] - mirrored_z).abs() < 0.01
                && other.half_extents_m == fence.half_extents_m),
            "fence {} has no mirror twin",
            fence.id
        );
    }
}
