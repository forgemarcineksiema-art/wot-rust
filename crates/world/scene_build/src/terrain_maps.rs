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
/// Smooth pooling suitability across more than one 5 m heightfield cell. This prevents the wet
/// sheen from exposing the simulation grid when a grazing camera catches the reflected sky.
const PUDDLE_BLUR_RADIUS_TEXELS: usize = 6;

fn pooling_smoothstep(edge0: f32, edge1: f32, value: f32) -> f32 {
    let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn blur_square_mask(mask: &[f32], size: usize, radius: usize) -> Vec<f32> {
    let mut horizontal = vec![0.0; mask.len()];
    let mut result = vec![0.0; mask.len()];
    let mut prefix = vec![0.0; size + 1];

    for z in 0..size {
        prefix[0] = 0.0;
        for x in 0..size {
            prefix[x + 1] = prefix[x] + mask[z * size + x];
        }
        for x in 0..size {
            let lo = x.saturating_sub(radius);
            let hi = (x + radius + 1).min(size);
            horizontal[z * size + x] = (prefix[hi] - prefix[lo]) / (hi - lo) as f32;
        }
    }

    for x in 0..size {
        prefix[0] = 0.0;
        for z in 0..size {
            prefix[z + 1] = prefix[z] + horizontal[z * size + x];
        }
        for z in 0..size {
            let lo = z.saturating_sub(radius);
            let hi = (z + radius + 1).min(size);
            result[z * size + x] = (prefix[hi] - prefix[lo]) / (hi - lo) as f32;
        }
    }
    result
}

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
    let mut puddle_propensity = Vec::with_capacity(size * size);
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
            // Pooling is broad terrain truth, not a per-fragment normal threshold. A wide
            // flatness ramp plus a weak local-depression preference is blurred below before
            // entering alpha, so neither the 5 m height grid nor its triangle diagonal survives.
            let flatness = pooling_smoothstep(0.94, 0.997, n[1]);
            let basin_radius_m = 8.0;
            let neighbour_mean = (height_at(wx - basin_radius_m, wz)
                + height_at(wx + basin_radius_m, wz)
                + height_at(wx, wz - basin_radius_m)
                + height_at(wx, wz + basin_radius_m))
                * 0.25;
            let basin = pooling_smoothstep(-0.12, 0.18, neighbour_mean - y);
            puddle_propensity.push(flatness * (0.82 + basin * 0.18));
            macro_normal.extend([
                ((n[0] * 0.5 + 0.5) * 255.0).round() as u8,
                ((n[1] * 0.5 + 0.5) * 255.0).round() as u8,
                ((n[2] * 0.5 + 0.5) * 255.0).round() as u8,
                0,
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

    let puddle_propensity = blur_square_mask(&puddle_propensity, size, PUDDLE_BLUR_RADIUS_TEXELS);
    for (texel, propensity) in macro_normal.chunks_exact_mut(4).zip(puddle_propensity) {
        texel[3] = (propensity * 255.0).round().clamp(0.0, 255.0) as u8;
    }

    TerrainGroundMaps { size: MAP_SIZE, splat, macro_normal, extent_m: [extent_x, extent_z] }
}

/// Each map's ground material set — the palette half of Terrain Material 2.0, read from the
/// map's BLUEPRINT (`materials` section; the art-direction ground window is a map-report
/// check). A blueprint without one wears the neutral steppe default.
pub fn terrain_material_set_for(map: MapId) -> TerrainMaterialSet {
    match &map_forge::cached_blueprint(map).materials {
        Some(spec) => material_set_from(spec),
        None => TerrainMaterialSet::default(),
    }
}

/// Bind the renderer-free blueprint palette to the renderer's material set. Public:
/// the map editor renders documents that are not (yet) any `MapId`.
pub fn material_set_from(spec: &map_forge::blueprint::GroundMaterialsSpec) -> TerrainMaterialSet {
    TerrainMaterialSet {
        layers: spec.layers.map(|layer| renderer_api::TerrainLayer {
            albedo: layer.albedo,
            detail: layer.detail,
            gloss: layer.gloss,
        }),
        macro_normal_strength: spec.macro_normal_strength,
        field_patch_strength: spec.field_patch_strength,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The migration lock: the blueprint palettes reproduce the hand-tuned sets exactly, and
    /// the maps keep their authored character — the worked steppe patches harder than the
    /// valley meadows, the valley's river-worn earth reads darker than the steppe's road dirt.
    #[test]
    fn blueprint_palettes_reproduce_the_hand_tuned_sets() {
        let steppe = terrain_material_set_for(MapId::ProkhorovkaHill252_2);
        let valley = terrain_material_set_for(MapId::BystraValley);

        assert_eq!(steppe.layers[0].albedo, [0.28, 0.33, 0.20]);
        assert_eq!(steppe.layers[3].albedo, [0.52, 0.51, 0.47]);
        assert_eq!(steppe.field_patch_strength, 1.0);

        assert_eq!(valley.layers[0].albedo, [0.26, 0.33, 0.21]);
        assert_eq!(valley.layers[2].albedo, [0.30, 0.26, 0.20]);
        assert_eq!(valley.field_patch_strength, 0.75);

        assert!(steppe.field_patch_strength > valley.field_patch_strength);
        assert!(valley.layers[2].albedo[0] < steppe.layers[2].albedo[0]);
        assert_eq!(steppe.macro_normal_strength, 0.65);
        assert_eq!(valley.macro_normal_strength, 0.65);
    }
}
