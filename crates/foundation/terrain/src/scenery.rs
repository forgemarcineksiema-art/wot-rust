//! Render-only scenery: procedural trees and rocks scattered deterministically over a map.
//! Purely presentation — nothing here blocks movement, shells, spotting or the camera (the
//! honest LOS blockers remain the solid `TreeLine` cover boxes, which scenery visually
//! in-fills). Every scatter is seeded and MIRRORED: instances are generated on the southern
//! half and reflected across the axis (position and yaw), so the map *looks* as fair as it
//! plays. `serde(default)` on the map field keeps pre-scenery baked assets loading.

use serde::{Deserialize, Serialize};

use crate::{HeightMap, StaticCoverObject};

/// What to grow. Wire/asset identity — append, never reorder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SceneryKind {
    /// Broad field tree: short trunk, wide layered crown.
    Oak,
    /// Tall narrow windbreak tree, planted in rows along lanes.
    Poplar,
    /// Riverbank tree: leaning trunk, low wide drooping crown.
    Willow,
    /// Orchard/garden tree: small, round, planted in loose rows.
    FruitTree,
    /// A field boulder.
    Rock,
    /// Low steppe shrub: a squat leafy mound, the open field's only vertical accent.
    Bush,
    /// Conifer: a bare lower trunk under a stacked conical crown — the alpine tree.
    Pine,
    /// Street furniture (urban-map PR-11): a cast-iron lamp column with its arm and head.
    /// Render-only like all scenery — it never blocks anything.
    Lamppost,
    /// A knee-high masonry debris pile — street dressing for a fought-over city. Low enough
    /// that it can never read as the cover it honestly is not.
    DebrisHeap,
    /// RETIRED (procedural-only, Świat 2.0, 2026-08-06): the imported CC0 broadleaf tree.
    /// Kept for wire identity only (append-only enum) — never authored, bakes to nothing.
    FloraTree,
    /// RETIRED imported flora: the CC0 pine. Wire identity only.
    FloraPine,
    /// RETIRED imported flora: the CC0 bush. Wire identity only.
    FloraBush,
}

impl SceneryKind {
    /// Every dressing object a map may place. Append-only: the compiled map blueprints store these.
    ///
    /// Locked variant-by-variant against the declaration by `quality`, not by counting: a
    /// length assertion cannot tell a forgotten variant from a shorter enum.
    pub const ALL: [SceneryKind; 12] = [
        SceneryKind::Oak,
        SceneryKind::Poplar,
        SceneryKind::Willow,
        SceneryKind::FruitTree,
        SceneryKind::Rock,
        SceneryKind::Bush,
        SceneryKind::Pine,
        SceneryKind::Lamppost,
        SceneryKind::DebrisHeap,
        SceneryKind::FloraTree,
        SceneryKind::FloraPine,
        SceneryKind::FloraBush,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SceneryInstance {
    pub kind: SceneryKind,
    /// World position, grounded on the heightmap at authoring time.
    pub position: [f32; 3],
    pub yaw_rad: f32,
    /// Uniform scale around 1.0 (0.8–1.3 keeps a scatter from reading as clones).
    pub scale: f32,
}

/// Deterministic splitmix64 — the same generator the FX pool trusts, no dependency needed.
fn scatter_mix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn unit(state: &mut u64) -> f32 {
    (scatter_mix64(state) >> 40) as f32 / ((1u64 << 24) - 1) as f32
}

/// Deterministic 0..1 from a world position (Immersja A2.1): an instance's OWN cosmetic
/// identity, stable across recompiles and independent of any rng stream — so giving a
/// mirrored twin its own yaw, or a planted row per-item variety, never reshuffles anything
/// else on the map. `salt` separates independent channels (yaw vs scale).
pub fn position_unit(x: f32, z: f32, salt: u64) -> f32 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64 ^ salt;
    for bits in [x.to_bits(), z.to_bits()] {
        hash ^= u64::from(bits);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    ((hash >> 40) & 0x00ff_ffff) as f32 / 16_777_216.0
}

/// A rectangular scatter region on the SOUTHERN half (z ≤ half axis); every accepted point is
/// emitted together with its northern mirror twin.
#[derive(Debug, Clone, Copy)]
pub struct ScatterRegion {
    pub x: (f32, f32),
    pub z: (f32, f32),
}

/// Scatter `pairs` mirrored instance pairs of `kind` inside `region`, rejecting points the
/// `exclude` rule refuses (bounded attempts, so a mostly-excluded region under-fills rather
/// than spinning). Mirroring: `z → axis*2 − z`, `yaw → −yaw`.
#[allow(clippy::too_many_arguments)]
pub fn scatter_mirrored(
    seed: u64,
    kind: SceneryKind,
    pairs: usize,
    region: ScatterRegion,
    axis_z: f32,
    heightmap: &HeightMap,
    exclude: &dyn Fn(f32, f32) -> bool,
    out: &mut Vec<SceneryInstance>,
) {
    let mut state = seed;
    let mut placed = 0;
    let mut attempts = 0;
    while placed < pairs && attempts < pairs * 12 {
        attempts += 1;
        let x = region.x.0 + unit(&mut state) * (region.x.1 - region.x.0);
        let z = region.z.0 + unit(&mut state) * (region.z.1 - region.z.0);
        let yaw = unit(&mut state) * std::f32::consts::TAU;
        let scale = 0.8 + unit(&mut state) * 0.5;
        if exclude(x, z) {
            continue;
        }
        let Some(ground) = heightmap.sample_height(x, z) else {
            continue;
        };
        let mirrored_z = axis_z * 2.0 - z;
        let Some(mirrored_ground) = heightmap.sample_height(x, mirrored_z) else {
            continue;
        };
        out.push(SceneryInstance { kind, position: [x, ground, z], yaw_rad: yaw, scale });
        // The twin is its own plant, not a reflection (Immersja A2.1): it keeps the pair's
        // SCALE (an Oak's trunk becomes a cover box scaled by the instance — fairness
        // demands identical twins) but grows its own yaw from its own position, so the
        // mirrored halves stop reading as one forest stamped twice.
        out.push(SceneryInstance {
            kind,
            position: [x, mirrored_ground, mirrored_z],
            yaw_rad: position_unit(x, mirrored_z, 0x0A17) * std::f32::consts::TAU,
            scale,
        });
        placed += 1;
    }
}

/// True when `(x, z)` lands inside any cover footprint inflated by `margin_m` — trees do not
/// grow through barns, parapets or wrecks.
pub fn inside_any_cover(cover: &[StaticCoverObject], x: f32, z: f32, margin_m: f32) -> bool {
    covers_containing(cover, x, z, margin_m).next().is_some()
}

/// Indices of every cover object a point falls inside — the NAMED answer to
/// [`inside_any_cover`], for callers that must report *what* blocked them rather than only
/// *that* something did. Both share this one containment rule, so a blocking test and the
/// explanation of that block can never disagree about where an object's footprint ends.
pub fn covers_containing(
    cover: &[StaticCoverObject],
    x: f32,
    z: f32,
    margin_m: f32,
) -> impl Iterator<Item = usize> + '_ {
    cover.iter().enumerate().filter_map(move |(index, object)| {
        let inside = (x - object.center[0]).abs() <= object.half_extents_m[0] + margin_m
            && (z - object.center[2]).abs() <= object.half_extents_m[2] + margin_m;
        inside.then_some(index)
    })
}
