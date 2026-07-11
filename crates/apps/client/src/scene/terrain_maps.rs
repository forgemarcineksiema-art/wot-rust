//! Bake the Terrain Material 2.0 ground maps (`renderer_api::TerrainGroundMaps`) from map
//! data — pure CPU, deterministic, hashed by tests. The splat map turns the map's own truth
//! (height, slope, roads, water margins, the grass-patchwork noise) into four layer weights;
//! the macro normal map samples the heightfield finer than the 5 m render grid so raking light
//! reads every hummock the vertices cannot. Baked once per scene build, uploaded once.

use renderer_api::{TerrainGroundMaps, TerrainMaterialSet};
use terrain::{BattlefieldMap, MapId};

use super::battlefield::{grass_patchwork_noise, road_blend_at};

/// Texture edge: 1024 texels over a 1000 m map is ~1 m ground truth per texel — the macro
/// scale the art direction's detail discipline wants (rule 5), cheap to hold resident (8 MB
/// for both maps together).
const MAP_SIZE: u32 = 1024;

/// How steep ground starts breaking to rock (1 - normal.y at the sampled point).
const ROCK_STEEP_START: f32 = 0.18;
/// Crest fraction of the map's height span where chalk/rock begins to break through.
const ROCK_CREST_START: f32 = 0.72;
/// Ground within this height above the waterline reads as worn wet earth (mud lives in the
/// dirt layer, darkened by the existing water tint and wetness at render time).
const WATER_MARGIN_M: f32 = 0.45;

/// Bake the splat + macro-normal maps for a battlefield. UV spans the heightmap's ground
/// extent exactly: `uv = world.xz / extent`.
pub fn bake_terrain_ground_maps(battlefield: &BattlefieldMap) -> TerrainGroundMaps {
    let heightmap = &battlefield.heightmap;
    let extent_x = (heightmap.width() - 1) as f32 * heightmap.cell_size_m();
    let extent_z = (heightmap.height() - 1) as f32 * heightmap.cell_size_m();
    let stats = heightmap.stats();
    let span = (stats.max_m - stats.min_m).max(1.0);

    let size = MAP_SIZE as usize;
    let mut splat = Vec::with_capacity(size * size * 4);
    let mut macro_normal = Vec::with_capacity(size * size * 4);
    // Finite-difference step for the macro normal: half a texel, well under the 5 m grid.
    let step = (extent_x / MAP_SIZE as f32) * 0.5;

    let height_at = |x: f32, z: f32| -> f32 {
        heightmap
            .sample_height(x.clamp(0.0, extent_x), z.clamp(0.0, extent_z))
            .unwrap_or(stats.min_m)
    };

    for tz in 0..size {
        for tx in 0..size {
            // Texel centre in world metres.
            let wx = (tx as f32 + 0.5) / MAP_SIZE as f32 * extent_x;
            let wz = (tz as f32 + 0.5) / MAP_SIZE as f32 * extent_z;
            let y = height_at(wx, wz);

            // Macro normal from central differences at sub-grid step.
            let dx = height_at(wx + step, wz) - height_at(wx - step, wz);
            let dz = height_at(wx, wz + step) - height_at(wx, wz - step);
            let inv_len =
                1.0 / (dx * dx + (2.0 * step) * (2.0 * step) + dz * dz).sqrt().max(1.0e-6);
            let n = [-dx * inv_len, 2.0 * step * inv_len, -dz * inv_len];
            macro_normal.extend([
                ((n[0] * 0.5 + 0.5) * 255.0).round() as u8,
                ((n[1] * 0.5 + 0.5) * 255.0).round() as u8,
                ((n[2] * 0.5 + 0.5) * 255.0).round() as u8,
                255,
            ]);

            // Layer weights. Rock breaks through on steep faces and high crests; roads wear
            // the ground to dirt; water margins read as worn wet earth; what remains splits
            // between lush grass and dry straw by the same patchwork noise the old vertex
            // palette drifted with (the map keeps its character, per-pixel now).
            let steep = (1.0 - n[1]).clamp(0.0, 1.0);
            let crest =
                ((y - stats.min_m) / span - ROCK_CREST_START).max(0.0) / (1.0 - ROCK_CREST_START);
            let rock =
                ((steep - ROCK_STEEP_START).max(0.0) * 3.2 + crest * crest * 0.9).clamp(0.0, 1.0);

            let road = road_blend_at(&battlefield.roads, wx, wz);
            let margin = battlefield
                .water
                .map(|water| {
                    let above = y - water.surface_level_m;
                    (1.0 - above.abs() / WATER_MARGIN_M).clamp(0.0, 1.0)
                })
                .unwrap_or(0.0);
            let dirt = (road + margin * 0.85).clamp(0.0, 1.0);

            let remaining = (1.0 - rock).max(0.0) * (1.0 - dirt).max(0.0);
            let patch = grass_patchwork_noise(wx, wz);
            let straw_share = ((patch - 0.5) * 2.4).clamp(0.0, 1.0);
            let grass = remaining * (1.0 - straw_share);
            let straw = remaining * straw_share;
            // Normalize into RGBA8; rock and dirt claim their share first, grass/straw fill.
            let total = (grass + straw + dirt + rock).max(1.0e-4);
            let quantize = |w: f32| ((w / total) * 255.0).round().clamp(0.0, 255.0) as u8;
            splat.extend([quantize(grass), quantize(straw), quantize(dirt), quantize(rock)]);
        }
    }

    TerrainGroundMaps { size: MAP_SIZE, splat, macro_normal, extent_m: [extent_x, extent_z] }
}

/// Each map's ground material set — the palette half of Terrain Material 2.0, envelope-locked
/// in `renderer_api` against the art-direction ground swatches.
pub fn terrain_material_set_for(map: MapId) -> TerrainMaterialSet {
    match map {
        MapId::ProkhorovkaHill252_2 => TerrainMaterialSet::prokhorovka(),
        MapId::BystraValley => TerrainMaterialSet::bystra(),
    }
}
