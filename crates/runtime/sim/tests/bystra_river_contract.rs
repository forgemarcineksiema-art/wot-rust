//! The Bystra river's gameplay contract, checked against the REAL water-physics constants:
//! the current drowns everywhere except the crossings, the fords sit inside the wading band,
//! the decks stand clear of the water, and no accidental puddle exists outside the river
//! corridor. This is what "the river is a tactical decision" means in numbers.

use physics::water::FORD_MAX_DEPTH_M;
use sim::DROWN_DEPTH_M;
use terrain::{RIVER_CORRIDOR_HALF_WIDTH_M, bystra_river_center_x, bystra_valley};

const HALF_M: f32 = 500.0;
const FORD_OFFSET_M: f32 = 180.0;
const PLANK_OFFSET_M: f32 = 320.0;

/// z-windows around the crossings where the centerline is legitimately NOT drowning-deep.
fn in_crossing_window(z: f32) -> bool {
    let dz = z - HALF_M;
    dz.abs() < 26.0 // stone bridge causeway
        || (dz.abs() - FORD_OFFSET_M).abs() < 75.0 // ford sills (with their gaussian skirts)
        || (dz.abs() - PLANK_OFFSET_M).abs() < 22.0 // plank decks
}

#[test]
fn the_current_is_a_drowning_decision_everywhere_between_crossings() {
    let map = bystra_valley();
    let water = map.water.expect("the Bystra is the map");
    let mut z = 30.0_f32;
    let mut checked = 0;
    while z <= 970.0 {
        if !in_crossing_window(z) {
            let x = bystra_river_center_x(z);
            let depth = water.depth_over(map.heightmap.sample_height(x, z).unwrap());
            assert!(
                depth >= DROWN_DEPTH_M + 0.2,
                "mid-channel at z {z} is only {depth} m deep — the current must drown"
            );
            checked += 1;
        }
        z += 5.0;
    }
    assert!(checked > 80, "the deep-channel sweep must actually cover the river");
}

#[test]
fn ford_sills_sit_inside_the_honest_wading_band() {
    let map = bystra_valley();
    let water = map.water.unwrap();
    for z in [HALF_M - FORD_OFFSET_M, HALF_M + FORD_OFFSET_M] {
        let x = bystra_river_center_x(z);
        let depth = water.depth_over(map.heightmap.sample_height(x, z).unwrap());
        assert!(
            (0.4..=FORD_MAX_DEPTH_M).contains(&depth),
            "ford sill at z {z} is {depth} m deep — must slow but stay fordable"
        );
    }
}

#[test]
fn crossing_decks_stand_clear_of_the_water() {
    let map = bystra_valley();
    let water = map.water.unwrap();
    // The stone bridge: a full metre of freeboard across the whole channel span.
    let bridge_x = bystra_river_center_x(HALF_M);
    let mut dx = -26.0_f32;
    while dx <= 26.0 {
        let h = map.heightmap.sample_height(bridge_x + dx, HALF_M).unwrap();
        assert!(
            h >= water.surface_level_m + 1.0,
            "bridge deck dips to {h} at dx {dx} — the causeway must clear the water"
        );
        dx += 2.0;
    }
    // The plank crossings: lower and narrower, but still dry decks.
    for z in [HALF_M - PLANK_OFFSET_M, HALF_M + PLANK_OFFSET_M] {
        let x = bystra_river_center_x(z);
        let mut dx = -18.0_f32;
        while dx <= 18.0 {
            let h = map.heightmap.sample_height(x + dx, z).unwrap();
            assert!(h >= water.surface_level_m + 0.3, "plank deck dips to {h} at z {z}, dx {dx}");
            dx += 2.0;
        }
    }
}

#[test]
fn no_water_exists_outside_the_river_corridor() {
    let map = bystra_valley();
    let water = map.water.unwrap();
    let cell = map.heightmap.cell_size_m();
    for zi in 0..map.heightmap.height() {
        for xi in 0..map.heightmap.width() {
            let h = map.heightmap.sample_at_index(xi, zi);
            if water.depth_over(h) > 0.0 {
                let x = xi as f32 * cell;
                let z = zi as f32 * cell;
                let d = (x - bystra_river_center_x(z)).abs();
                assert!(
                    d <= RIVER_CORRIDOR_HALF_WIDTH_M + 3.0,
                    "accidental puddle at ({x}, {z}): depth {} outside the corridor",
                    water.depth_over(h)
                );
            }
        }
    }
}

/// Every crossing is actually drivable: stepping its approach and span, no 5 m step exceeds
/// the climb the drive model can hold.
#[test]
fn crossing_approaches_stay_under_the_climb_wall() {
    let map = bystra_valley();
    let crossings: [(f32, f32); 5] = [
        (HALF_M, 0.0),
        (HALF_M - FORD_OFFSET_M, 0.0),
        (HALF_M + FORD_OFFSET_M, 0.0),
        (HALF_M - PLANK_OFFSET_M, 0.0),
        (HALF_M + PLANK_OFFSET_M, 0.0),
    ];
    for (z, _) in crossings {
        let center_x = bystra_river_center_x(z);
        let mut previous = map.heightmap.sample_height(center_x - 45.0, z).unwrap();
        let mut dx = -40.0_f32;
        while dx <= 45.0 {
            let h = map.heightmap.sample_height(center_x + dx, z).unwrap();
            let grade = ((h - previous) / 5.0).abs();
            assert!(
                grade < 0.5,
                "crossing at z {z} has a {grade} grade step at dx {dx} — not drivable"
            );
            previous = h;
            dx += 5.0;
        }
    }
}
