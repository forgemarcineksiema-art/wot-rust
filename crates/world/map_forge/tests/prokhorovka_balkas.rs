//! The rebuilt Prokhorovka's balkas (drawn Valley strokes — the first shipped use of the
//! Rece do terenu vocabulary): the anti-tank-ditch balka is the COVERED east-west rotation
//! of each half (full defilade from the midline, crossable everywhere — the bots' no-wall
//! promise), and the Storozhevoe draw masks the approach to the hill foot from the saddle.

use map_forge::battlefield;
use terrain::{HeightMap, MapId};

fn ground(hm: &HeightMap, x: f32, z: f32) -> (f32, f32, f32) {
    (x, hm.sample_height(x, z).expect("inside map"), z)
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

/// A tank rotating inside the ditch balka is in FULL defilade from the midline: both the
/// hull line and the turret line are masked from an eye on the embankment approach.
#[test]
fn the_ditch_balka_hides_a_rotating_tank_from_the_midline() {
    let map = battlefield(MapId::ProkhorovkaHill252_2);
    let hm = &map.heightmap;
    let eye = ground(hm, 500.0, 505.0);
    let eye = (eye.0, eye.1 + 2.5, eye.2);
    let tank = ground(hm, 450.0, 388.0);
    let hull = (tank.0, tank.1 + 0.9, tank.2);
    let turret = (tank.0, tank.1 + 2.4, tank.2);
    assert!(
        blockage(hm, eye, hull) > 0.3,
        "the balka floor must mask a hull from the midline (got {:.2})",
        blockage(hm, eye, hull)
    );
    assert!(
        blockage(hm, eye, turret) > 0.3,
        "the balka is FULL defilade - the turret line is masked too (got {:.2})",
        blockage(hm, eye, turret)
    );
}

/// The hill's FIRING LINE still looks into the balka: its dominance stays meaningful — a
/// tank in the ditch is not safe from a gun on the west-facing crest. (The peak itself
/// never saw the western lowlands — its own crest shelf masks them; that was always true.)
#[test]
fn the_hill_top_still_looks_into_the_ditch_balka() {
    let map = battlefield(MapId::ProkhorovkaHill252_2);
    let hm = &map.heightmap;
    let crest = ground(hm, 726.0, 350.0);
    let eye = (crest.0, crest.1 + 2.3, crest.2);
    let tank = ground(hm, 450.0, 388.0);
    let turret = (tank.0, tank.1 + 2.4, tank.2);
    assert!(
        blockage(hm, eye, turret) < 0.0,
        "Hill 252.2 must see the turret of a tank in the balka (blockage {:.2})",
        blockage(hm, eye, turret)
    );
}

/// The no-wall promise the bots depend on: crossing the ditch balka anywhere along its run
/// never exceeds the drive graph's climb grade — a covered route, never a trap.
#[test]
fn the_ditch_balka_is_crossable_everywhere() {
    let map = battlefield(MapId::ProkhorovkaHill252_2);
    let hm = &map.heightmap;
    let mut x = 340.0_f32;
    while x <= 690.0 {
        let mut previous = hm.sample_height(x, 350.0).expect("inside map");
        let mut z = 355.0_f32;
        while z <= 440.0 {
            let here = hm.sample_height(x, z).expect("inside map");
            let grade = (here - previous).abs() / 5.0;
            assert!(grade <= 0.55, "the balka must stay crossable: grade {grade:.2} at ({x}, {z})");
            previous = here;
            z += 5.0;
        }
        x += 10.0;
    }
}

/// The Storozhevoe draw masks the approach to the hill foot from the saddle: a tank
/// mid-draw is hull-safe from an eye between the massifs.
#[test]
fn the_flank_balka_masks_the_hill_approach_from_the_saddle() {
    let map = battlefield(MapId::ProkhorovkaHill252_2);
    let hm = &map.heightmap;
    let saddle = ground(hm, 800.0, 500.0);
    let eye = (saddle.0, saddle.1 + 2.3, saddle.2);
    let tank = ground(hm, 700.0, 232.0);
    let hull = (tank.0, tank.1 + 0.9, tank.2);
    assert!(
        blockage(hm, eye, hull) > 0.3,
        "the draw must mask a hull on the hill approach (got {:.2})",
        blockage(hm, eye, hull)
    );
}

/// The draw's mouth opens onto the shelf: the climb out toward the hull-down line stays
/// honestly drivable (no report warning, no bot trap).
#[test]
fn the_flank_balka_opens_onto_the_shelf() {
    let map = battlefield(MapId::ProkhorovkaHill252_2);
    let hm = &map.heightmap;
    let mut previous = hm.sample_height(752.0, 292.0).expect("inside map");
    let mut z = 297.0_f32;
    while z <= 340.0 {
        let here = hm.sample_height(752.0, z).expect("inside map");
        let grade = (here - previous).abs() / 5.0;
        assert!(grade < 0.5, "the mouth must open drivably onto the shelf: {grade:.2} at z {z}");
        previous = here;
        z += 5.0;
    }
}
