use crate::map_build::heightmap_from_fn;
use crate::prokhorovka_cover::static_cover_objects;
use crate::prokhorovka_features::map_features;
use crate::prokhorovka_layout::{spawn_zones, strategic_points};
use crate::{BattlefieldMap, math};

const MAP_SIZE_M: f32 = 1000.0;
const CELL_SIZE_M: f32 = 5.0;
const SAMPLES_PER_SIDE: usize = 201;
pub(crate) const HALF_M: f32 = MAP_SIZE_M * 0.5;
const STEPPE_BASE_M: f32 = 8.0;
const MIN_HEIGHT_M: f32 = 0.2;

// Eastern hill high ground (the Hill 252.2 massif), placed as a north/south mirror pair
// `HILL_OFFSET_M` from the central axis so both teams contest equivalent high ground.
// Phase 3 will sculpt the hull-down crest; for now it is a smooth dome.
pub(crate) const HILL_X_M: f32 = 800.0;
pub(crate) const HILL_OFFSET_M: f32 = 150.0;
const HILL_HEIGHT_M: f32 = 22.0;

// Anti-tank ditch lines flanking the central embankment, one per half.
pub(crate) const DITCH_OFFSET_M: f32 = 100.0;

// Railway embankment: a steep ~9 m railbed on the central axis (the historical ~30 ft
// embankment) opened only at three crossings so it channels movement instead of sealing it.
const EMBANKMENT_HEIGHT_M: f32 = 9.0;
const EMBANKMENT_SIGMA_M: f32 = 7.0;
const CROSSING_SIGMA_M: f32 = 13.0;
const CROSSINGS_X_M: [f32; 3] = [HALF_M - 250.0, HALF_M, HALF_M + 250.0];

// Western Psel field: a deliberately flat, low, coverless killzone. `field_openness` ramps
// from 1.0 in the deep west to 0.0 at the rolling steppe and flattens relief there, while
// `overwatch_knolls` raise mirrored tank-destroyer perches that see across the open ground.
const FIELD_INNER_M: f32 = 160.0;
const FIELD_OUTER_M: f32 = 320.0;
const FIELD_FLATTEN: f32 = 0.92;
pub(crate) const OVERWATCH_X_M: f32 = 235.0;
pub(crate) const OVERWATCH_OFFSET_M: f32 = 300.0;
const OVERWATCH_HEIGHT_M: f32 = 6.0;

// Hull-down terrace cut into each hill's centre-facing (west) shoulder: a crest ridge with a
// reverse-slope shelf just behind it, so a tank on the shelf masks its hull behind the crest
// while its turret fires over it. Localized in z to each hill and mirrored north/south.
const CREST_X_M: f32 = 726.0;
const CREST_RIDGE_M: f32 = 2.5;
pub(crate) const SHELF_X_M: f32 = 752.0;
const SHELF_CUT_M: f32 = 1.6;
const TERRACE_SIGMA_X_M: f32 = 11.0;
const TERRACE_Z_SIGMA_M: f32 = 78.0;

pub fn prokhorovka_hill_252_2() -> BattlefieldMap {
    let heightmap = heightmap_from_fn(SAMPLES_PER_SIDE, CELL_SIZE_M, height_at);
    BattlefieldMap {
        id: "prokhorovka_hill_252_2".to_string(),
        name: "Prokhorovka - Hill 252.2 Sector".to_string(),
        size_m: [MAP_SIZE_M, MAP_SIZE_M],
        historical_basis: "Prokhorovka, 12 July 1943: Hill 252.2, Oktyabrskiy State Farm, the Psel corridor, anti-tank ditch, and railway embankment.".to_string(),
        design_notes: vec![
            "open steppe tank map: a central east-west railway embankment divides the sectors, an open Psel flank in the west, and contested hill high ground in the east".to_string(),
            "1000m x 1000m at 5m height samples; the map is mirror-symmetric across the central embankment axis so both teams get equivalent ground".to_string(),
        ],
        // The Psel is a dry lowland on this cut of the battlefield — no standing water.
        water: None,
        spawn_zones: spawn_zones(&heightmap),
        strategic_points: strategic_points(&heightmap),
        features: map_features(&heightmap),
        static_cover: static_cover_objects(&heightmap),
        // Open steppe: no scenery dressing authored (yet) — the map predates the system.
        scenery: Vec::new(),
        heightmap,
    }
}

/// Terrain height at a world point, mirror-symmetric across the central east-west axis
/// (`z = HALF_M`). Every component is symmetric about that axis -- an on-axis ridge or a
/// north/south mirror pair -- so `height_at(x, z) == height_at(x, MAP_SIZE_M - z)` holds by
/// construction and both teams' halves are identical. See the symmetry test in
/// `tests/historical_map.rs`.
fn height_at(x: f32, z: f32) -> f32 {
    let dz = z - HALF_M;
    let mut h = STEPPE_BASE_M;
    // Flatten relief inside the western field so it offers no folds to hide behind.
    h += rolling_steppe(x, dz) * (1.0 - FIELD_FLATTEN * field_openness(x));
    h += embankment(x, z);
    h += hill_pair(x, z);
    h += hull_down_terrace(x, z);
    h += overwatch_knolls(x, z);
    h -= ditch_pair(z);
    h -= psel_lowland(x);
    h.max(MIN_HEIGHT_M)
}

/// Gentle rolling relief. The `z` dependence goes only through even functions of `dz`, so
/// the field is symmetric about the central axis while staying smooth across it.
fn rolling_steppe(x: f32, dz: f32) -> f32 {
    1.3 * (x / 85.0).sin() + 1.0 * (dz / 70.0).cos() + 0.6 * (x * 0.055).sin() * (dz * 0.035).cos()
}

/// Central railway embankment: a steep ~9 m railbed on the symmetry axis that walls the two
/// sectors apart, opened only at discrete crossings. Its face is steeper than tank
/// gradeability, so movement must funnel through the gaps. Symmetry holds because the ridge
/// profile is even in `z` and the crossing mask depends only on `x`.
fn embankment(x: f32, z: f32) -> f32 {
    let ridge = math::gaussian1(z, HALF_M, EMBANKMENT_SIGMA_M, EMBANKMENT_HEIGHT_M);
    ridge * (1.0 - crossing_openness(x))
}

/// 1.0 inside a crossing gap (railbed removed, ground passable), 0.0 on the solid railbed.
fn crossing_openness(x: f32) -> f32 {
    let openness: f32 = CROSSINGS_X_M
        .iter()
        .map(|&crossing_x| math::gaussian1(x, crossing_x, CROSSING_SIGMA_M, 1.0))
        .sum();
    openness.min(1.0)
}

/// Eastern hill high ground as a north/south mirror pair.
fn hill_pair(x: f32, z: f32) -> f32 {
    math::gaussian2(x, z, HILL_X_M, HALF_M - HILL_OFFSET_M, 150.0, 95.0, HILL_HEIGHT_M)
        + math::gaussian2(x, z, HILL_X_M, HALF_M + HILL_OFFSET_M, 150.0, 95.0, HILL_HEIGHT_M)
}

/// Anti-tank ditch lines flanking the embankment, mirrored north and south.
fn ditch_pair(z: f32) -> f32 {
    math::gaussian1(z, HALF_M - DITCH_OFFSET_M, 10.0, 3.0)
        + math::gaussian1(z, HALF_M + DITCH_OFFSET_M, 10.0, 3.0)
}

/// Open low Psel flank along the western edge. Uniform in `z`, so trivially symmetric.
fn psel_lowland(x: f32) -> f32 {
    math::gaussian1(x, 0.0, 140.0, 4.0)
}

/// 1.0 deep in the western field, ramping to 0.0 at the central rolling steppe. Depends only
/// on `x`, so flattening the field preserves the north/south mirror symmetry.
fn field_openness(x: f32) -> f32 {
    ((FIELD_OUTER_M - x) / (FIELD_OUTER_M - FIELD_INNER_M)).clamp(0.0, 1.0)
}

/// Hull-down terraces on the eastern hills: a crest ridge on the west shoulder with a reverse
/// shelf behind it, as a mirrored north/south pair localized in z to each hill.
fn hull_down_terrace(x: f32, z: f32) -> f32 {
    terrace_at(x, z, HALF_M - HILL_OFFSET_M) + terrace_at(x, z, HALF_M + HILL_OFFSET_M)
}

fn terrace_at(x: f32, z: f32, hill_z: f32) -> f32 {
    let z_local = math::gaussian1(z, hill_z, TERRACE_Z_SIGMA_M, 1.0);
    let crest = math::gaussian1(x, CREST_X_M, TERRACE_SIGMA_X_M, CREST_RIDGE_M);
    let shelf = math::gaussian1(x, SHELF_X_M, TERRACE_SIGMA_X_M, SHELF_CUT_M);
    (crest - shelf) * z_local
}

/// Mirrored tank-destroyer overwatch knolls on the field's flanking corners.
fn overwatch_knolls(x: f32, z: f32) -> f32 {
    math::gaussian2(
        x,
        z,
        OVERWATCH_X_M,
        HALF_M - OVERWATCH_OFFSET_M,
        40.0,
        55.0,
        OVERWATCH_HEIGHT_M,
    ) + math::gaussian2(
        x,
        z,
        OVERWATCH_X_M,
        HALF_M + OVERWATCH_OFFSET_M,
        40.0,
        55.0,
        OVERWATCH_HEIGHT_M,
    )
}
