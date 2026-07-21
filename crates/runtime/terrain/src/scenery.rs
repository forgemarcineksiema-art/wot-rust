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
        out.push(SceneryInstance {
            kind,
            position: [x, mirrored_ground, mirrored_z],
            yaw_rad: -yaw,
            scale,
        });
        placed += 1;
    }
}

/// True when `(x, z)` lands inside any cover footprint inflated by `margin_m` — trees do not
/// grow through barns, parapets or wrecks.
pub fn inside_any_cover(cover: &[StaticCoverObject], x: f32, z: f32, margin_m: f32) -> bool {
    cover.iter().any(|object| {
        (x - object.center[0]).abs() <= object.half_extents_m[0] + margin_m
            && (z - object.center[2]).abs() <= object.half_extents_m[2] + margin_m
    })
}
