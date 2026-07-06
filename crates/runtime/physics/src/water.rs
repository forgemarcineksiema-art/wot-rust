//! Wading: what standing water does to a driving hull. Depth comes from the map's
//! [`terrain::WaterBody`] over the heightmap; below [`WADE_DRAG_START_M`] water is cosmetic,
//! above it the hull pushes a bow wave (speed-proportional drag) and the bed softens under the
//! tracks (traction cut). Drowning — the game rule that a flooded engine dies — lives in the
//! sim, not here; physics only makes deep water honest to drive through.
//!
//! Everything is pure f32 arithmetic on the deterministic path shared by the server and the
//! client predictor; with zero depth every formula collapses to exactly the dry-land value, so
//! waterless maps stay bit-identical (replay-locked).

/// Depth where wading effects begin: below the fender line water splashes, nothing more.
pub const WADE_DRAG_START_M: f32 = 0.35;

/// The deepest water a hull can still practically ford. Past this the drag makes progress
/// crawl (and the sim's drowning rule is close overhead) — map fords are designed under it,
/// and bots refuse water deeper than it.
pub const FORD_MAX_DEPTH_M: f32 = 0.9;

/// Bow-wave resistance per metre of depth past the wade start, per m/s of speed (linear drag:
/// pushing water scales with how fast and how deep the hull wades).
const WATER_DRAG_PER_M_PER_MPS: f32 = 1.1;

/// Constant bed resistance per metre of excess depth (soft riverbed sucking at the tracks).
const WATER_RESIST_MPS2_PER_M: f32 = 2.6;

/// Traction multiplier lost per metre of excess depth (silt under the tracks), floored so a
/// wading hull always keeps some grip.
const WATER_TRACTION_CUT_PER_M: f32 = 0.45;
const WATER_TRACTION_FLOOR: f32 = 0.25;

/// Depth past the wade-start threshold; zero in the splash zone.
pub fn wade_excess_m(water_depth_m: f32) -> f32 {
    (water_depth_m - WADE_DRAG_START_M).max(0.0)
}

/// Longitudinal deceleration (m/s²) the water demands at `speed_mps` in `water_depth_m` of
/// water. Zero at or below the wade start — the dry path is untouched.
pub fn wading_resistance_mps2(water_depth_m: f32, speed_mps: f32) -> f32 {
    let excess = wade_excess_m(water_depth_m);
    if excess <= 0.0 {
        return 0.0;
    }
    excess * (WATER_RESIST_MPS2_PER_M + WATER_DRAG_PER_M_PER_MPS * speed_mps.abs())
}

/// Traction after the riverbed's cut; identity at or below the wade start.
pub fn wading_traction(traction: f32, water_depth_m: f32) -> f32 {
    let excess = wade_excess_m(water_depth_m);
    if excess <= 0.0 {
        return traction;
    }
    (traction * (1.0 - WATER_TRACTION_CUT_PER_M * excess).max(0.0)).max(WATER_TRACTION_FLOOR)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_splash_zone_is_free_and_the_dry_path_is_exact() {
        assert_eq!(wading_resistance_mps2(0.0, 8.0), 0.0);
        assert_eq!(wading_resistance_mps2(WADE_DRAG_START_M, 8.0), 0.0);
        assert_eq!(wading_traction(0.8, 0.0), 0.8);
        assert_eq!(wading_traction(0.8, WADE_DRAG_START_M), 0.8);
    }

    #[test]
    fn deeper_and_faster_both_cost_more() {
        let shallow = wading_resistance_mps2(0.6, 5.0);
        let deep = wading_resistance_mps2(0.9, 5.0);
        let fast = wading_resistance_mps2(0.6, 10.0);
        assert!(shallow > 0.0);
        assert!(deep > shallow, "deeper water must drag harder");
        assert!(fast > shallow, "the bow wave grows with speed");
        assert!(wading_traction(1.0, 0.9) < 1.0, "the ford bed softens the tracks");
        assert!(wading_traction(1.0, 5.0) >= WATER_TRACTION_FLOOR);
    }
}
