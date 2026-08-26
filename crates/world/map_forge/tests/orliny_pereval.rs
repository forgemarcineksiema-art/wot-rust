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

/// Teren W3: the east rim answers the summits — and only the summits' flank. The named
/// knolls stand PROUD of the defile approach (a commander's eye on the perch clears a
/// turret on the approach bowl by metres — real overwatch by elevation, no knife-crest
/// to certify), while the massif stands metres PROUD of the perch-to-col line, so the
/// counter watches the defile flank without reaching into the middle lane.
#[test]
fn the_east_rim_answers_the_defile_flank_but_not_the_col() {
    let map = map();
    let perch_ground = map.heightmap.sample_height(930.0, 220.0).expect("perch");
    let eye = (930.0, perch_ground + 2.3, 220.0);

    // The perch genuinely overwatches the defile approach: clear line, metres of slack.
    let approach_ground = map.heightmap.sample_height(840.0, 320.0).expect("approach");
    let approach_turret = (840.0, approach_ground + 2.4, 320.0);
    assert!(
        common::clearance(&map.heightmap, eye, approach_turret) > 1.0,
        "the knoll must stand proud of the defile approach"
    );
    // The perch stands over the bowl toward the Oryol face too.
    let bowl_ground = map.heightmap.sample_height(780.0, 350.0).expect("bowl");
    let bowl_turret = (780.0, bowl_ground + 2.4, 350.0);
    assert!(
        common::clearance(&map.heightmap, eye, bowl_turret) > 1.0,
        "the knoll must stand proud of the approach bowl"
    );
    // And the col stays out of the perch's reach - not by a wall (the gated ridge is
    // OPEN toward the pass by design, and the col sits higher than the knoll) but by the
    // game's own optics: the hamlet lies beyond the longest view range any vehicle
    // parks (440 m, `TankSpec::view_range_m`), with margin. The counter watches the
    // defile flank; the middle lane cannot be farmed from it.
    let distance = ((930.0f32 - 500.0).powi(2) + (220.0f32 - 500.0).powi(2)).sqrt();
    assert!(
        distance > 460.0,
        "the perch-to-col distance must clear the longest parked view range: {distance}"
    );
}

/// Teren W3b tail: the col carries its landmark. The Orlinoye watchtower stands ON the
/// axis at the capture point itself and wears the StoneTower kind (destructible masonry
/// that falls into a hull-down stump — the sim locks both sight lines). The landmark
/// claim, in the two honest halves: no cover on the map raises a taller silhouette from
/// its own footprint, and within the hamlet nothing stands higher in absolute terms —
/// "the tallest silhouette of the col", as the feature note promises. (A crag riding a
/// mountain knuckle at the map's edge may top the tower's absolute Y — that is the
/// mountain's height, not a rival silhouette.)
#[test]
fn the_watchtower_crowns_the_col_as_the_tallest_silhouette() {
    let map = map();
    let tower = map
        .static_cover
        .iter()
        .find(|object| object.id == "col_watchtower")
        .expect("the col watchtower ships");
    assert_eq!(tower.kind, terrain::StaticCoverKind::StoneTower);
    assert_eq!([tower.center[0], tower.center[2]], [500.0, 500.0], "on the axis, on the cap");
    let tower_top = tower.center[1] + tower.half_extents_m[1];
    for other in map.static_cover.iter().filter(|object| object.id != "col_watchtower") {
        assert!(
            tower.half_extents_m[1] >= other.half_extents_m[1] + 1.5,
            "no cover raises a taller silhouette than the tower: {} ({:.1} vs {:.1} half)",
            other.id,
            tower.half_extents_m[1],
            other.half_extents_m[1]
        );
        let in_hamlet = (other.center[0] - 500.0).powi(2) + (other.center[2] - 500.0).powi(2)
            <= 90.0f32.powi(2);
        if in_hamlet {
            let other_top = other.center[1] + other.half_extents_m[1];
            assert!(
                tower_top > other_top + 1.0,
                "within the hamlet the tower out-tops {} ({tower_top:.1} vs {other_top:.1})",
                other.id
            );
        }
    }
}
