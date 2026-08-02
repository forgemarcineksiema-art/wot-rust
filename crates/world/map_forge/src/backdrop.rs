//! The render-only backdrop skirt, for ANY blueprint map: the terrain program is a pure
//! function, so the fields, ridges and rivers simply continue past the boundary; an optional
//! [`HorizonSpec`] blends the apron outward into enclosing hills that close the view — except
//! along the river's exit gap. Physics never reads this: `HeightMap::sample_height` still
//! returns `None` outside the map, and the playable world ends exactly where it always did.
//!
//! Inside the map bounds this IS the terrain program, so the skirt meets the real heightmap
//! exactly at every shared grid point (seam-locked in the catalog tests).

use std::collections::BTreeMap;
use std::sync::{OnceLock, RwLock};

use terrain::{MapId, band_mask, lerp, smoothstep01};

use crate::blueprint::{HorizonSpec, MapBlueprint};
use crate::catalog::blueprint_for;
use crate::ops::EvalContext;

/// The blueprint behind a compiled map's `id`, if it is a shipped (catalog) map. Parsed
/// once and cached for the process lifetime — backdrop evaluation happens per vertex at
/// scene-bake time, never per frame, but reparsing the document per call would still be
/// wasteful.
pub fn cached_blueprint_by_id(id: &str) -> Option<&'static MapBlueprint> {
    let cache = CACHE.get_or_init(|| RwLock::new(BTreeMap::new()));
    if let Some(blueprint) = cache.read().expect("backdrop cache").get(id) {
        return Some(blueprint);
    }
    let parsed = MapId::SHIPPED
        .iter()
        .map(|id| blueprint_for(*id))
        .find(|blueprint| blueprint.meta.id == id)?;
    let blueprint: &'static MapBlueprint = Box::leak(Box::new(parsed));
    cache.write().expect("backdrop cache").insert(id.to_string(), blueprint);
    Some(blueprint)
}

/// The world height beyond the map's edge (and exactly the terrain program inside it).
pub fn backdrop_height(blueprint: &MapBlueprint, x: f32, z: f32) -> f32 {
    let interior = terrain_program_height(blueprint, x, z);
    let Some(horizon) = &blueprint.horizon else {
        return interior;
    };
    let [width, depth] = blueprint.grid.size_m;
    let outside_m = (-x).max(x - width).max(-z).max(z - depth).max(0.0);
    if outside_m <= 0.0 {
        return interior;
    }
    let closure = smoothstep01((outside_m - horizon.closure_start_m) / horizon.closure_span_m);
    // The river's exit corridor stays open (the gap follows the extended centerline).
    let river_gap = match &blueprint.river {
        Some(river) => {
            band_mask(x - river.center_x(z), horizon.river_gap_half_m, horizon.river_gap_falloff_m)
        }
        None => 0.0,
    };
    lerp(interior, enclosing_hills(horizon, x, z), closure * (1.0 - river_gap))
}

/// Evaluate the terrain program at any world point (the same walk the compiler samples).
fn terrain_program_height(blueprint: &MapBlueprint, x: f32, z: f32) -> f32 {
    let ctx = EvalContext { river: blueprint.river.as_ref(), axis_z: blueprint.grid.axis_z() };
    let mut h = blueprint.terrain.base.eval(x);
    for op in &blueprint.terrain.ops {
        h = op.apply(&ctx, x, z, h);
    }
    h
}

/// The hills that close the horizon: gently rolling, well above the valley floor, never so
/// tall they read as mountains a tank map forgot to mention.
fn enclosing_hills(horizon: &HorizonSpec, x: f32, z: f32) -> f32 {
    let [swell_amp, swell_x_freq, swell_z_freq, swell_x_phase] = horizon.swell;
    horizon.hills_base_m
        + swell_amp * (x * swell_x_freq + swell_x_phase).sin() * (z * swell_z_freq).cos()
        + horizon.x_roll[0] * (x * horizon.x_roll[1]).cos()
        + horizon.z_roll[0] * (z * horizon.z_roll[1]).sin()
}

static CACHE: OnceLock<RwLock<BTreeMap<String, &'static MapBlueprint>>> = OnceLock::new();
