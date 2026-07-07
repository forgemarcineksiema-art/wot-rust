//! Dolina Bystrej — a fictional Central-European river valley. The Bystra runs south→north
//! ALONG the axis of advance, splitting the map into two flanks with different games: open
//! farmland under the Windmill Hill in the west (the long-range game), and the town of
//! Kamienna climbing the eastern slope toward a quarry ridge (the brawl). Every engagement
//! between the flanks funnels through five crossings: a stone causeway bridge in the town's
//! shadow, a mirrored pair of fords, and a mirrored pair of plank crossings near the spawns.
//!
//! ## Generator design (why this is not a bag of Gaussians)
//!
//! A valley is made of *structural* features that must hold gameplay guarantees by
//! construction, so the generator works in three vocabularies (see `sculpt`):
//!
//! 1. **Swept channel**: the river is a lateral distance `d = x − river_center_x(z)` to a
//!    parametric centerline, carved to an explicit bed target `WATER_LEVEL − depth(z)`.
//!    The depth profile IS the design: drowning-deep everywhere except the ford sills —
//!    "the ford is fordable and the current drowns" cannot regress by accident.
//! 2. **Terrace flattening**: the town bench blends toward an explicit ramp so buildings sit
//!    on honest ground, and rolling relief is damped where structure lives.
//! 3. **Deck overrides, applied last**: the bridge causeway and plank crossings raise the
//!    carved channel to explicit deck elevations above the water line.
//!
//! Mirror symmetry across `z = HALF_M` is the fairness contract (as on Prokhorovka): the
//! centerline and every mask are even functions of `z − HALF_M`, and every feature is either
//! on-axis or a mirrored pair, so `height(x, z) == height(x, MAP_SIZE_M − z)` holds by
//! construction and is locked in `tests/bystra_map.rs`.

use crate::bystra_cover::valley_cover_objects;
use crate::bystra_features::valley_features;
use crate::bystra_layout::{valley_spawn_zones, valley_strategic_points};
use crate::map_build::heightmap_from_fn;
use crate::water::WaterBody;
use crate::{BattlefieldMap, math, sculpt};

const MAP_SIZE_M: f32 = 1000.0;
const CELL_SIZE_M: f32 = 5.0;
const SAMPLES_PER_SIDE: usize = 201;
pub(crate) const HALF_M: f32 = MAP_SIZE_M * 0.5;
const MIN_HEIGHT_M: f32 = 0.2;

/// The Bystra's still-water surface. Depth anywhere is `WATER_LEVEL_M − ground`.
pub(crate) const WATER_LEVEL_M: f32 = 5.0;

// --- The river: a meandering centerline swept with a carved channel cross-profile. ---
/// Nominal centerline east of mid-map; the meander bows it westward through the center,
/// carving the pocket the stone bridge crosses at the town's feet.
pub(crate) const RIVER_X_M: f32 = 585.0;
const MEANDER_BOW_M: f32 = 45.0;
const MEANDER_BOW_SIGMA_M: f32 = 200.0;
const MEANDER_WIGGLE_M: f32 = 14.0;
const MEANDER_WIGGLE_WAVE_M: f32 = 70.0;
/// Half-width of the fully-carved channel; the bank falloff eases back to land around it.
const CHANNEL_HALF_WIDTH_M: f32 = 13.0;
const CHANNEL_BANK_FALLOFF_M: f32 = 12.0;
/// Corridor within which water may exist at all (channel + banks + margin) — the "no
/// accidental puddles" contract in `tests/bystra_water.rs` checks everything outside it.
/// Public: the water mesh, minimap, and bot water probes all reason about the same corridor.
pub const RIVER_CORRIDOR_HALF_WIDTH_M: f32 = CHANNEL_HALF_WIDTH_M + CHANNEL_BANK_FALLOFF_M + 6.0;
/// Mid-channel depth: comfortably past `sim::DROWN_DEPTH_M` — the current is a decision.
const CHANNEL_DEPTH_M: f32 = 2.6;
/// Ford sill depth: under `physics::water::FORD_MAX_DEPTH_M` — slow, exposed, honest.
const FORD_DEPTH_M: f32 = 0.65;
pub(crate) const FORD_OFFSET_M: f32 = 180.0;
const FORD_SIGMA_M: f32 = 26.0;

// --- Crossings above the water: the stone causeway and the plank crossings. ---
/// Stone bridge deck elevation (≈1.4 m over the water) and its footprint.
pub(crate) const BRIDGE_DECK_M: f32 = 6.4;
const BRIDGE_HALF_LENGTH_M: f32 = 34.0;
const BRIDGE_HALF_WIDTH_M: f32 = 7.0;
/// Plank crossings near the spawns: narrow, low, quick flank rotations.
pub(crate) const PLANK_OFFSET_M: f32 = 320.0;
const PLANK_DECK_M: f32 = 5.6;
const PLANK_HALF_LENGTH_M: f32 = 26.0;
const PLANK_HALF_WIDTH_M: f32 = 4.5;

// --- West flank: farmland under the Windmill Hill, with TD knolls on the corners. ---
pub(crate) const WINDMILL_X_M: f32 = 250.0;
const WINDMILL_HEIGHT_M: f32 = 11.0;
/// Hull-down works on the hill's river-facing shoulder: a reverse shelf cut into the slope
/// with a crest ridge EAST of it (between the shelf and the bridge), so the crest masks the
/// hull from the river while the turret works over it. The enemy is downhill here — without
/// the raised crest the naturally falling slope would expose everything.
const WINDMILL_CREST_X_M: f32 = 365.0;
pub(crate) const WINDMILL_SHELF_X_M: f32 = 340.0;
pub(crate) const WINDMILL_SHELF_OFFSET_M: f32 = 90.0;
const CREST_RIDGE_M: f32 = 1.4;
const SHELF_CUT_M: f32 = 1.0;
pub(crate) const KNOLL_X_M: f32 = 105.0;
pub(crate) const KNOLL_OFFSET_M: f32 = 300.0;
const KNOLL_HEIGHT_M: f32 = 5.0;

// --- East flank: the town bench of Kamienna and the quarry ridge behind it. ---
pub(crate) const TOWN_CENTER_X_M: f32 = 745.0;
const TOWN_HALF_DEPTH_X_M: f32 = 95.0;
pub(crate) const TOWN_HALF_SPAN_Z_M: f32 = 165.0;
const TOWN_EDGE_FALLOFF_M: f32 = 45.0;
/// How completely the bench flattens toward its ramp (1.0 would be a snooker table).
const TOWN_FLATTEN: f32 = 0.85;
pub(crate) const RIDGE_PERCH_X_M: f32 = 915.0;
pub(crate) const RIDGE_PERCH_OFFSET_M: f32 = 160.0;
pub(crate) const QUARRY_X_M: f32 = 935.0;
const QUARRY_FLOOR_M: f32 = 17.5;

pub fn bystra_valley() -> BattlefieldMap {
    let heightmap = heightmap_from_fn(SAMPLES_PER_SIDE, CELL_SIZE_M, valley_height_at);
    let static_cover = valley_cover_objects(&heightmap);
    let scenery = crate::bystra_scenery::valley_scenery(&heightmap, &static_cover);
    BattlefieldMap {
        id: "bystra_valley".to_string(),
        name: "Dolina Bystrej".to_string(),
        size_m: [MAP_SIZE_M, MAP_SIZE_M],
        historical_basis: "Fictional: a Central-European river valley composed for balanced \
                           crossing combat — no historical engagement is depicted."
            .to_string(),
        design_notes: vec![
            "the Bystra river runs along the axis of advance, splitting the map into an open \
             farmland flank (west, the long-range game) and the town of Kamienna on the \
             eastern slope (the brawl); five crossings decide the mid-game rotations"
                .to_string(),
            "1000m x 1000m at 5m height samples; mirror-symmetric across the central axis so \
             both teams get equivalent ground, crossings, and town blocks"
                .to_string(),
            "the channel is carved to an explicit bed target: drowning-deep everywhere except \
             the two ford sills, with the causeway and plank decks raised above the water line"
                .to_string(),
        ],
        water: Some(WaterBody { surface_level_m: WATER_LEVEL_M }),
        spawn_zones: valley_spawn_zones(&heightmap),
        strategic_points: valley_strategic_points(&heightmap),
        features: valley_features(&heightmap),
        static_cover,
        scenery,
        heightmap,
    }
}

/// The river centerline's x at a given z. Even about `z = HALF_M` (a Gaussian bow plus a
/// cosine wiggle, both even in `dz`), which is what makes the whole water course mirror-fair.
/// Public: the render mesh, minimap, bots and the contract tests all follow the same line.
pub fn bystra_river_center_x(z: f32) -> f32 {
    let dz = z - HALF_M;
    RIVER_X_M - math::gaussian1(z, HALF_M, MEANDER_BOW_SIGMA_M, MEANDER_BOW_M)
        + MEANDER_WIGGLE_M * (dz / MEANDER_WIGGLE_WAVE_M).cos()
}

/// The world beyond the map's edge, for the render-only backdrop skirt: the same analytic
/// surface the playfield is built from (every component of `valley_height_at` is a pure
/// function, so the fields, the ridge and the RIVER simply continue past the boundary),
/// blending outward into rolling forested hills that close the valley — except along the
/// river's course, which keeps its gap so the Bystra visibly flows in from the south and out
/// to the north. Physics never reads this: `HeightMap::sample_height` still returns `None`
/// outside the map, and the playable world ends exactly where it always did.
///
/// Inside the map bounds this IS `valley_height_at`, so the skirt meets the real heightmap
/// exactly at every shared grid point (seam-locked in `tests/bystra_map.rs`).
pub fn bystra_backdrop_height(x: f32, z: f32) -> f32 {
    let interior = valley_height_at(x, z);
    let outside_m = (-x).max(x - MAP_SIZE_M).max(-z).max(z - MAP_SIZE_M).max(0.0);
    if outside_m <= 0.0 {
        return interior;
    }
    let closure = sculpt::smoothstep01((outside_m - 30.0) / 280.0);
    // The river's exit corridor stays open (the gap follows the extended centerline).
    let river_gap = sculpt::band_mask(x - bystra_river_center_x(z), 34.0, 40.0);
    sculpt::lerp(interior, enclosing_hills(x, z), closure * (1.0 - river_gap))
}

/// The hills that close the horizon: gently rolling, well above the valley floor, never so
/// tall they read as mountains a tank map forgot to mention.
fn enclosing_hills(x: f32, z: f32) -> f32 {
    27.0 + 6.5 * (x * 0.0085 + 1.7).sin() * (z * 0.0060).cos()
        + 4.0 * (x * 0.013).cos()
        + 3.0 * (z * 0.011).sin()
}

/// Terrain height at a world point. Composition order matters and encodes the guarantees:
/// land shape first, structural flattening second, the river carve third (the channel wins
/// over anything on land), and the crossing decks last (a deck wins over the channel).
fn valley_height_at(x: f32, z: f32) -> f32 {
    let dz = z - HALF_M;
    let river_d = x - bystra_river_center_x(z);

    let mut h = valley_base(x);
    h += rolling_relief(x, dz) * relief_damping(x, dz, river_d);
    h += windmill_hill(x, z);
    h += shoulder_terraces(x, z);
    h += td_knolls(x, z);
    // Town bench: flatten toward an explicit ramp so streets and buildings sit honestly.
    h = sculpt::lerp(h, town_ramp(x), TOWN_FLATTEN * town_mask(x, dz));
    // Quarry: a sheltered bowl cut into the ridge on the axis.
    h = sculpt::lerp(h, QUARRY_FLOOR_M, math::gaussian2(x, z, QUARRY_X_M, HALF_M, 45.0, 60.0, 1.0));
    // Floodplain: the meadows lean down toward the river on both banks.
    h -= 1.0 * sculpt::band_mask(river_d, 30.0, 45.0);
    // The river carve: an explicit bed target under the channel mask.
    h = sculpt::lerp(
        h,
        bed_target(z),
        sculpt::band_mask(river_d, CHANNEL_HALF_WIDTH_M, CHANNEL_BANK_FALLOFF_M),
    );
    // Crossing decks, applied after the carve so they always stand above the water.
    h = apply_deck(
        h,
        x,
        dz,
        river_d,
        0.0,
        BRIDGE_DECK_M,
        BRIDGE_HALF_WIDTH_M,
        BRIDGE_HALF_LENGTH_M,
    );
    h = apply_deck(
        h,
        x,
        dz,
        river_d,
        -PLANK_OFFSET_M,
        PLANK_DECK_M,
        PLANK_HALF_WIDTH_M,
        PLANK_HALF_LENGTH_M,
    );
    h = apply_deck(
        h,
        x,
        dz,
        river_d,
        PLANK_OFFSET_M,
        PLANK_DECK_M,
        PLANK_HALF_WIDTH_M,
        PLANK_HALF_LENGTH_M,
    );
    h.max(MIN_HEIGHT_M)
}

/// West→east valley cross-section: the western upland eases down to the floodplain, then the
/// town slope climbs to the quarry ridge. Pure function of `x` — trivially symmetric.
fn valley_base(x: f32) -> f32 {
    12.5 - 5.5 * sculpt::smoothstep01((x - 180.0) / 250.0)
        + 8.0 * sculpt::smoothstep01((x - 640.0) / 200.0)
        + 9.0 * sculpt::smoothstep01((x - 860.0) / 90.0)
}

/// Gentle field relief; `z` enters only through even functions of `dz` (symmetry).
fn rolling_relief(x: f32, dz: f32) -> f32 {
    0.9 * (x / 70.0).sin() + 0.7 * (dz / 60.0).cos() + 0.5 * (x * 0.043).sin() * (dz * 0.031).cos()
}

/// Relief fades where structure lives: inside the town bench and across the river corridor,
/// so streets stay streets and banks stay banks.
fn relief_damping(x: f32, dz: f32, river_d: f32) -> f32 {
    let town = town_mask(x, dz);
    let river = sculpt::band_mask(river_d, RIVER_CORRIDOR_HALF_WIDTH_M, 20.0);
    (1.0 - 0.85 * town) * (1.0 - 0.9 * river)
}

/// The channel's explicit bed elevation: drowning-deep, except where the ford sills rise.
fn bed_target(z: f32) -> f32 {
    let ford = (math::gaussian1(z, HALF_M - FORD_OFFSET_M, FORD_SIGMA_M, 1.0)
        + math::gaussian1(z, HALF_M + FORD_OFFSET_M, FORD_SIGMA_M, 1.0))
    .min(1.0);
    let depth = CHANNEL_DEPTH_M - (CHANNEL_DEPTH_M - FORD_DEPTH_M) * ford;
    WATER_LEVEL_M - depth
}

/// Raise a crossing deck: a band along the river's local crossing axis, lifted toward the
/// deck elevation. `lerp` toward a target ABOVE the carved bed means the deck always clears
/// the water regardless of what the carve did underneath.
#[allow(clippy::too_many_arguments)]
fn apply_deck(
    h: f32,
    _x: f32,
    dz: f32,
    river_d: f32,
    deck_dz: f32,
    deck_m: f32,
    half_width_m: f32,
    half_length_m: f32,
) -> f32 {
    let mask = sculpt::band_mask(dz - deck_dz, half_width_m, 6.0)
        * sculpt::band_mask(river_d, half_length_m, 14.0);
    // Only ever raise: the deck bridges the channel but never trenches the bank road.
    sculpt::lerp(h, deck_m.max(h), mask)
}

fn windmill_hill(x: f32, z: f32) -> f32 {
    math::gaussian2(x, z, WINDMILL_X_M, HALF_M, 90.0, 120.0, WINDMILL_HEIGHT_M)
}

/// Crest-and-shelf pair on the hill's river-facing shoulder, mirrored north/south: a hull on
/// the shelf masks behind the crest while its turret works the bridge and the fords.
fn shoulder_terraces(x: f32, z: f32) -> f32 {
    shoulder_terrace_at(x, z, HALF_M - WINDMILL_SHELF_OFFSET_M)
        + shoulder_terrace_at(x, z, HALF_M + WINDMILL_SHELF_OFFSET_M)
}

fn shoulder_terrace_at(x: f32, z: f32, shelf_z: f32) -> f32 {
    let window = math::gaussian1(z, shelf_z, 60.0, 1.0);
    let crest = math::gaussian1(x, WINDMILL_CREST_X_M, 9.0, CREST_RIDGE_M);
    let shelf = math::gaussian1(x, WINDMILL_SHELF_X_M, 9.0, SHELF_CUT_M);
    (crest - shelf) * window
}

/// Mirrored tank-destroyer knolls on the western corners, watching the fields and fords.
fn td_knolls(x: f32, z: f32) -> f32 {
    math::gaussian2(x, z, KNOLL_X_M, HALF_M - KNOLL_OFFSET_M, 35.0, 45.0, KNOLL_HEIGHT_M)
        + math::gaussian2(x, z, KNOLL_X_M, HALF_M + KNOLL_OFFSET_M, 35.0, 45.0, KNOLL_HEIGHT_M)
}

/// The town bench footprint: full inside the block grid, easing out at the edges.
fn town_mask(x: f32, dz: f32) -> f32 {
    sculpt::band_mask(x - TOWN_CENTER_X_M, TOWN_HALF_DEPTH_X_M, TOWN_EDGE_FALLOFF_M)
        * sculpt::band_mask(dz, TOWN_HALF_SPAN_Z_M, TOWN_EDGE_FALLOFF_M)
}

/// The explicit grade the town is built on: a steady climb from the river gate to the ridge
/// road, so every street holds the same honest slope.
fn town_ramp(x: f32) -> f32 {
    8.0 + (x - (TOWN_CENTER_X_M - TOWN_HALF_DEPTH_X_M)).max(0.0) * 0.037
}
