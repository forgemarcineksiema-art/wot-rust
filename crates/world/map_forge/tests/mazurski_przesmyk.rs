//! Mazurski Przesmyk's design locks (teren W6): the water DENIES, the causeway carries,
//! and the signature pair stands. The playability BFS, the Rot180 probe and the
//! standing-sheet contracts run in the report (asserted clean here); these tests lock the
//! SHAPE that makes the map what it is.

use map_forge::{Severity, WaterThresholds, blueprint_for, compile};
use terrain::MapId;

mod common;
use common::max_grade;

fn map() -> terrain::BattlefieldMap {
    let blueprint = blueprint_for(MapId::MazurskiPrzesmyk);
    let (map, report) = compile(&blueprint);
    let errors: Vec<String> = report
        .entries
        .iter()
        .filter(|entry| entry.severity == Severity::Error)
        .map(|entry| format!("{} at {:?}: {}", entry.check, entry.at, entry.message))
        .collect();
    assert!(errors.is_empty(), "the shipped map must pass its own report:\n{}", errors.join("\n"));
    map
}

/// The water architecture, through the live resolution rule: both lakes and both peat
/// ponds reach the drowning band (they DENY - the map's whole grammar rests on it), the
/// two levels genuinely differ (the first two-level map), and the causeway line between
/// the ponds is dry its entire length.
#[test]
fn the_lakes_drown_and_the_causeway_stays_dry() {
    let map = map();
    let field = map.water_field();
    let drown = WaterThresholds::default().drown_depth_m;

    for (x, z, what) in [
        (220.0, 750.0, "west lake"),
        (780.0, 250.0, "east lake"),
        (460.0, 500.0, "west peat pond"),
        (540.0, 500.0, "east peat pond"),
    ] {
        let ground = map.heightmap.sample_height(x, z).expect("probe on the map");
        let depth = field.depth_at(ground, x, z);
        assert!(depth >= drown, "{what} must reach the drowning band, got {depth:.2} m");
    }
    let lake_level = field.level_at(220.0, 750.0).expect("the lake is wet");
    let pond_level = field.level_at(460.0, 500.0).expect("the pond is wet");
    assert!(
        (lake_level - pond_level).abs() > 1.0,
        "two genuinely different levels ({lake_level} vs {pond_level}) - the schema showcase"
    );

    let mut z = 430.0;
    while z <= 570.0 {
        let ground = map.heightmap.sample_height(500.0, z).expect("causeway");
        assert!(
            field.depth_at(ground, 500.0, z) == 0.0,
            "the causeway must stay dry at (500, {z})"
        );
        z += 5.0;
    }
}

/// All three lanes hold under the climb grade, measured as LINES: the causeway approach,
/// the shore road behind the reeds, and the moraine lane into the east defile.
#[test]
fn the_three_lanes_stay_drivable() {
    let map = map();
    let hm = &map.heightmap;
    let lanes = [
        ("causeway south", (455.0, 420.0), (500.0, 500.0)),
        ("causeway north", (545.0, 580.0), (500.0, 500.0)),
        ("shore road west", (110.0, 400.0), (95.0, 620.0)),
        ("shore road north", (130.0, 870.0), (300.0, 958.0)),
        ("moraine lane", (560.0, 300.0), (596.0, 430.0)),
        ("east defile", (596.0, 430.0), (600.0, 478.0)),
    ];
    for (name, from, to) in lanes {
        let grade = max_grade(hm, from, to);
        assert!(grade < 0.5, "{name} must stay drivable (worst grade {grade:.2})");
    }
}

/// The twin mills stand where the dossier says, as exact half-turn twins with a shared
/// box - the pair that names the objective from a kilometre out.
#[test]
fn the_mills_flank_the_causeway_as_a_rot_pair() {
    let map = map();
    let south = map
        .static_cover
        .iter()
        .find(|object| object.id == "causeway_windmill_south")
        .expect("the south mill ships");
    let north = map
        .static_cover
        .iter()
        .find(|object| object.id == "causeway_windmill_north")
        .expect("the north mill ships");
    assert_eq!([south.center[0], south.center[2]], [500.0, 466.0]);
    assert_eq!(
        [1000.0 - north.center[0], 1000.0 - north.center[2]],
        [south.center[0], south.center[2]],
        "the north mill is the exact half-turn twin"
    );
    assert_eq!(south.half_extents_m, north.half_extents_m, "fairness shares the box");
    assert!(south.half_extents_m[1] >= 7.0, "a mill is a tower, not a shed");
}
