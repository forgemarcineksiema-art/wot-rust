//! Ostrogorsk's design locks (urban-map program PR-02, the skeleton stage): the rail berm
//! IS impassable between its three gates, the gates and the city streets ARE drivable, the
//! rise shoulders offer real hull-down over the boulevard exits, and the whole gameplay
//! layer mirrors. The playability BFS in the report proves connectivity; these tests lock
//! the *shape* that makes the map what it is.

use map_forge::{Severity, blueprint_for, compile};
use terrain::{HeightMap, MapId};

fn map() -> terrain::BattlefieldMap {
    let blueprint = blueprint_for(MapId::Ostrogorsk);
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

/// The steepest 5 m-step grade along a straight walk between two points.
fn max_grade(hm: &HeightMap, from: (f32, f32), to: (f32, f32)) -> f32 {
    let length = ((to.0 - from.0).powi(2) + (to.1 - from.1).powi(2)).sqrt();
    let steps = (length / 5.0).ceil().max(1.0) as u32;
    let mut worst = 0.0_f32;
    let mut previous: Option<f32> = None;
    for step in 0..=steps {
        let t = step as f32 / steps as f32;
        let x = from.0 + (to.0 - from.0) * t;
        let z = from.1 + (to.1 - from.1) * t;
        let Some(h) = hm.sample_height(x, z) else { continue };
        if let Some(previous) = previous {
            worst = worst.max((h - previous).abs() / (length / steps as f32));
        }
        previous = Some(h);
    }
    worst
}

/// The berm between the gates must be a real wall: crossing it anywhere between the gate
/// skirts exceeds the climb grade, so the eastern routes flow through the three gates BY
/// DESIGN. (The inverse of the usual check — this locks intent, not drivability.)
#[test]
fn the_berm_is_impassable_between_the_gates() {
    let map = map();
    for z in [120.0, 200.0, 340.0, 400.0, 620.0, 660.0, 800.0, 880.0] {
        let grade = max_grade(&map.heightmap, (790.0, z), (870.0, z));
        assert!(
            grade > 0.55,
            "the berm at z {z} must exceed the 0.55 climb grade (got {grade:.2}) - \
             a crossable berm unmakes the three-gate east flank"
        );
    }
}

/// All three berm gates and the city street corridors stay honestly drivable, well under
/// the 0.55 climb wall the playability graph uses.
#[test]
fn the_gates_and_the_streets_stay_drivable() {
    let map = map();
    let hm = &map.heightmap;
    let lanes = [
        ("level crossing", (770.0, 500.0), (890.0, 500.0)),
        ("south underpass", (770.0, 270.0), (890.0, 270.0)),
        ("north underpass", (770.0, 730.0), (890.0, 730.0)),
        ("boulevard west", (80.0, 500.0), (470.0, 500.0)),
        ("boulevard east", (470.0, 500.0), (770.0, 500.0)),
        ("high street south", (430.0, 245.0), (430.0, 495.0)),
        ("market lane south", (125.0, 446.0), (425.0, 446.0)),
        ("forge lane south", (195.0, 358.0), (425.0, 358.0)),
        ("rail line over the south gate", (830.0, 220.0), (830.0, 320.0)),
    ];
    for (name, from, to) in lanes {
        let grade = max_grade(hm, from, to);
        assert!(grade < 0.5, "{name} must stay drivable (worst grade {grade:.2})");
    }
}

/// The city bench holds one honest grade: every 5 m step inside the town rect stays gentle,
/// so streets never climb stairs (the FlattenToRamp promise, measured).
#[test]
fn the_city_bench_holds_one_grade() {
    let map = map();
    let hm = &map.heightmap;
    for z in [200.0, 300.0, 440.0, 560.0, 700.0, 800.0] {
        let grade = max_grade(hm, (60.0, z), (450.0, z));
        assert!(
            grade < 0.12,
            "the bench walk at z {z} must stay street-gentle (worst grade {grade:.2})"
        );
    }
}

/// A tank on the rise shoulder shelf masks its hull behind the crest while the turret works
/// over it, seen from the boulevard exit below — the open flank's hull-down promise.
#[test]
fn the_rise_shoulder_offers_hull_down_over_the_boulevard_exit() {
    let map = map();
    assert_hull_down_line(&map.heightmap, (500.0, 330.0), 625.0, 660.0, 330.0);
    assert_hull_down_line(&map.heightmap, (500.0, 670.0), 625.0, 660.0, 670.0);
}

/// Every strategic point, spawn and the capture zone sits on the axis or carries its mirror
/// twin — the fairness contract of the whole gameplay layer, asserted from compiled data.
#[test]
fn the_gameplay_layer_mirrors() {
    let map = map();
    let axis = 500.0;
    for point in &map.strategic_points {
        let mirrored_z = 2.0 * axis - point.position[2];
        let has_twin = (point.position[2] - axis).abs() < 0.5
            || map.strategic_points.iter().any(|other| {
                other.role == point.role
                    && (other.position[0] - point.position[0]).abs() < 0.5
                    && (other.position[2] - mirrored_z).abs() < 0.5
            });
        assert!(has_twin, "strategic point {} needs an axis seat or a mirror twin", point.id);
    }
    assert_eq!(map.strategic_points.len(), 13, "the skeleton ships its full 13-point layer");
}

/// The dense core (urban-map PR-12): the city carries a real box count inside the proven
/// bench envelope, its masonry speaks the urban kinds, and the born-ruin pairs are present
/// — punched into the street fabric, mirrored, and named for the birth rule.
#[test]
fn the_core_is_dense_masonry_with_born_ruins_inside_the_envelope() {
    let map = map();
    let total = map.static_cover.len();
    assert!(
        (90..=160).contains(&total),
        "the dense core targets the proven bench envelope (urban_150), got {total} boxes"
    );
    let city_blocks = map
        .static_cover
        .iter()
        .filter(|c| c.kind == terrain::StaticCoverKind::CityBuilding)
        .count();
    assert!(city_blocks >= 50, "the core is masonry, got {city_blocks} CityBuilding boxes");
    let walls =
        map.static_cover.iter().filter(|c| c.kind == terrain::StaticCoverKind::StoneWall).count();
    assert!(walls >= 10, "the yards and the seam are walled, got {walls} StoneWall runs");
    let ruins = map.static_cover.iter().filter(|c| c.id.contains("ruin")).count();
    assert_eq!(ruins, 6, "three mirrored born-ruin pairs open the cross-block sightlines");
}

/// Scans `shelf_from..shelf_to` at `z` for a spot where the hull is masked (> 0.4 m) and the
/// turret clears (> 0.4 m) against an attacker eye at `enemy` — the existence of a usable
/// hull-down line, not one tuned point.
fn assert_hull_down_line(
    hm: &HeightMap,
    enemy: (f32, f32),
    shelf_from: f32,
    shelf_to: f32,
    z: f32,
) {
    let enemy_ground = hm.sample_height(enemy.0, enemy.1).expect("inside map");
    let enemy_eye = (enemy.0, enemy_ground + 2.3, enemy.1);
    let mut found = false;
    let mut bx = shelf_from;
    while bx <= shelf_to {
        let g = hm.sample_height(bx, z).expect("inside map");
        let hull = (bx, g + 0.9, z);
        let turret = (bx, g + 2.4, z);
        if blockage(hm, enemy_eye, hull) > 0.4 && clearance(hm, enemy_eye, turret) > 0.4 {
            found = true;
            break;
        }
        bx += 3.0;
    }
    assert!(
        found,
        "no usable hull-down spot on the shelf x {shelf_from}..{shelf_to} at z {z} \
         against an eye at {enemy:?}"
    );
}

/// Largest amount the terrain rises above the straight sightline (> 0 means it is masked).
fn blockage(hm: &HeightMap, from: (f32, f32, f32), to: (f32, f32, f32)) -> f32 {
    let mut worst = f32::NEG_INFINITY;
    let steps = 120;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let x = from.0 + (to.0 - from.0) * t;
        let z = from.2 + (to.2 - from.2) * t;
        let line_y = from.1 + (to.1 - from.1) * t;
        if let Some(g) = hm.sample_height(x, z) {
            worst = worst.max(g - line_y);
        }
    }
    worst
}

/// Smallest gap the sightline keeps above the terrain (> 0 everywhere means it is visible).
fn clearance(hm: &HeightMap, from: (f32, f32, f32), to: (f32, f32, f32)) -> f32 {
    let mut worst = f32::INFINITY;
    let steps = 120;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let x = from.0 + (to.0 - from.0) * t;
        let z = from.2 + (to.2 - from.2) * t;
        let line_y = from.1 + (to.1 - from.1) * t;
        if let Some(g) = hm.sample_height(x, z) {
            worst = worst.min(line_y - g);
        }
    }
    worst
}
