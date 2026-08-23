//! Orliny Pereval's design locks: the wall IS impassable between the gates, the three gates
//! and the crest walk ARE drivable, both shelf lines offer real hull-down, and the summits
//! are the roof of the map. The playability BFS in the report proves connectivity; these
//! tests lock the *shape* that makes the map what it is.

use map_forge::{Severity, blueprint_for, compile};
use terrain::MapId;

mod common;
use common::{assert_hull_down_line, max_grade};

fn map() -> terrain::BattlefieldMap {
    let blueprint = blueprint_for(MapId::OrlinyPereval);
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

/// The wall between the gates must be a real wall: crossing it anywhere between the gate
/// skirts exceeds the climb grade, so the drive graph flows through the gates BY DESIGN.
/// (The inverse of the usual check — this locks intent, not drivability.)
#[test]
fn the_wall_is_impassable_between_the_gates() {
    let map = map();
    for x in [320.0, 400.0, 620.0, 740.0, 940.0] {
        let grade = max_grade(&map.heightmap, (x, 420.0), (x, 580.0));
        assert!(
            grade > 0.55,
            "the wall at x {x} must exceed the 0.55 climb grade (got {grade:.2}) - \
             a crossable wall unmakes the three-lane design"
        );
    }
}

/// All three gates and both crest walks stay honestly drivable, well under the 0.55 climb
/// wall the playability graph uses.
#[test]
fn the_gates_and_the_crest_walk_stay_drivable() {
    let map = map();
    let hm = &map.heightmap;
    let lanes = [
        ("dolina gate", (200.0, 400.0), (200.0, 600.0)),
        ("pass approach", (500.0, 150.0), (500.0, 500.0)),
        ("defile gate", (840.0, 380.0), (840.0, 620.0)),
        ("crest walk west", (500.0, 500.0), (340.0, 500.0)),
        ("crest walk east", (500.0, 500.0), (680.0, 500.0)),
    ];
    for (name, from, to) in lanes {
        let grade = max_grade(hm, from, to);
        assert!(grade < 0.5, "{name} must stay drivable (worst grade {grade:.2})");
    }
}

/// The summits are the roof of the map: no ground outside their skirts stands higher.
#[test]
fn the_summits_are_the_roof_of_the_map() {
    let map = map();
    let hm = &map.heightmap;
    let west = hm.sample_height(340.0, 500.0).expect("inside map");
    let east = hm.sample_height(680.0, 500.0).expect("inside map");
    assert!(east > west, "Oryol (east) is the taller summit by design");
    let stats = hm.stats();
    assert!(
        east > stats.max_m - 1.0,
        "the east summit ({east:.1} m) must crown the map (max {:.1} m)",
        stats.max_m
    );
    assert!(east > 60.0, "the massif must carry real vertical drama (east summit {east:.1} m)");
}

/// A tank on the Sokol shoulder shelf masks its hull behind the crest while the turret works
/// over it, seen from the Dolina lane below.
#[test]
fn sokol_shoulder_offers_hull_down_over_the_dolina_lane() {
    let map = map();
    assert_hull_down_line(&map.heightmap, (180.0, 330.0), 295.0, 330.0, 325.0);
}

/// A tank on the Oryol face shelf masks its hull behind its crest against the pass approach.
#[test]
fn oryol_face_offers_hull_down_over_the_pass_approach() {
    let map = map();
    assert_hull_down_line(&map.heightmap, (505.0, 300.0), 595.0, 630.0, 330.0);
}
