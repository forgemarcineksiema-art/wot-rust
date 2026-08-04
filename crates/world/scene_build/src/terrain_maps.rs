//! Bake the Terrain Material 2.0 ground maps (`renderer_api::TerrainGroundMaps`) from map
//! data — pure CPU, deterministic, hashed by tests. The splat map turns the map's own truth
//! (height, slope, roads, water margins, the grass-patchwork noise) into four layer weights;
//! the macro normal map samples the heightfield finer than the 5 m render grid so raking light
//! reads every hummock the vertices cannot. Baked once per scene build, uploaded once.

use renderer_api::{TerrainGroundMaps, TerrainMaterialSet};
use terrain::{BattlefieldMap, MapId};

use super::battlefield::bake_lane_count;
use terrain::GroundClassifier;

/// Texture edge: 1024 texels over a 1000 m map is ~1 m ground truth per texel — the macro
/// scale the art direction's detail discipline wants (rule 5), cheap to hold resident (8 MB
/// for both maps together).
const MAP_SIZE: u32 = 1024;

/// Smooth pooling suitability across more than one 5 m heightfield cell. This prevents the wet
/// sheen from exposing the simulation grid when a grazing camera catches the reflected sky.
const PUDDLE_BLUR_RADIUS_TEXELS: usize = 6;

/// How much of the drainage wetness (the classifier's flow lane) reaches the pooling alpha:
/// a saturated drainage line glosses at half strength, so cieki catch the sky without
/// reading as standing water. Composed by MAX with the flatness/basin term, then blurred.
const FLOW_PUDDLE_GAIN: f32 = 0.5;

/// The road crown's rise at the centerline (teren B2): a parabolic camber baked as a
/// cross-road bend of the MACRO NORMAL — pure shading, ~10 cm that blocks nothing and
/// collides with nothing (the honest-steel line for sub-profile relief). The 5 m heightfield
/// cannot carry a form this narrow; the 1 m bake texel can. Entering through dx/dz BEFORE
/// normalization, the crown also reaches the pooling flatness — a crowned road sheds water,
/// which is what a crown is FOR.
const ROAD_CROWN_M: f32 = 0.10;

/// A road's XZ bounding box grown by its half-width + the bake's sampling step — the cheap
/// rejection in front of the polyline walk, computed once per bake instead of per texel.
struct RoadBound {
    min: [f32; 2],
    max: [f32; 2],
}

fn road_bounds(roads: &[terrain::Road], step: f32) -> Vec<RoadBound> {
    roads
        .iter()
        .map(|road| {
            let mut min = [f32::INFINITY; 2];
            let mut max = [f32::NEG_INFINITY; 2];
            for point in &road.points {
                min[0] = min[0].min(point[0]);
                min[1] = min[1].min(point[1]);
                max[0] = max[0].max(point[0]);
                max[1] = max[1].max(point[1]);
            }
            let reach = road.width_m * 0.5 + step * 2.0;
            RoadBound {
                min: [min[0] - reach, min[1] - reach],
                max: [max[0] + reach, max[1] + reach],
            }
        })
        .collect()
}

/// The crown height field: parabolic over each road's width, strongest road wins — the same
/// fold rule as the paint, so the camber and the surface never disagree about which road a
/// texel belongs to. Zero beyond the kerb (the edge discontinuity IS the kerb's read).
fn road_crown_m(roads: &[terrain::Road], bounds: &[RoadBound], x: f32, z: f32) -> f32 {
    roads
        .iter()
        .zip(bounds)
        .map(|(road, bound)| {
            if x < bound.min[0] || x > bound.max[0] || z < bound.min[1] || z > bound.max[1] {
                return 0.0;
            }
            let half = road.width_m * 0.5;
            let d = road.distance_to(x, z);
            if d >= half { 0.0 } else { ROAD_CROWN_M * (1.0 - (d / half) * (d / half)) }
        })
        .fold(0.0, f32::max)
}

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

/// The read-only inputs every ground-map texel needs. Split out so a row range can be baked
/// on its own thread: the bake reads the map and writes nothing shared, which is what makes
/// the row split a pure scheduling decision (see [`GroundBakeContext::bake_rows`]).
struct GroundBakeContext<'a> {
    battlefield: &'a BattlefieldMap,
    extent_x: f32,
    extent_z: f32,
    min_m: f32,
    /// The shared ground rule. The bake stopped classifying on its own the day physics needed the
    /// same answer: what the eye reads under a track and what the tracks feel are one rule now,
    /// living in `terrain` below both. The splat bytes are golden-locked precisely so this move
    /// can be proved inert (`client/tests/terrain_ground_maps.rs`).
    ground: GroundClassifier,
    /// Finite-difference step for the macro normal: half a texel, well under the 5 m grid.
    step: f32,
    /// Per-road XZ bounds (teren B2) — the cheap rejection in front of the crown's walks.
    road_bounds: Vec<RoadBound>,
}

impl<'a> GroundBakeContext<'a> {
    fn new(battlefield: &'a BattlefieldMap) -> Self {
        let heightmap = &battlefield.heightmap;
        let extent_x = (heightmap.width() - 1) as f32 * heightmap.cell_size_m();
        let extent_z = (heightmap.height() - 1) as f32 * heightmap.cell_size_m();
        let stats = heightmap.stats();
        let step = (extent_x / MAP_SIZE as f32) * 0.5;
        Self {
            battlefield,
            extent_x,
            extent_z,
            min_m: stats.min_m,
            ground: GroundClassifier::new(battlefield),
            step,
            road_bounds: road_bounds(&battlefield.roads, step),
        }
    }

    fn height_at(&self, x: f32, z: f32) -> f32 {
        self.battlefield
            .heightmap
            .sample_height(x.clamp(0.0, self.extent_x), z.clamp(0.0, self.extent_z))
            .unwrap_or(self.min_m)
    }

    /// Bake texel rows `[row_start, row_end)` into their own buffers. Every texel is a pure
    /// function of the map and its own coordinates — no accumulator crosses a texel — so
    /// concatenating the ranges in row order reproduces the whole-map bake bit for bit.
    fn bake_rows(&self, row_start: usize, row_end: usize) -> GroundBakeRows {
        let size = MAP_SIZE as usize;
        let rows = row_end - row_start;
        let mut splat = Vec::with_capacity(rows * size * 4);
        let mut macro_normal = Vec::with_capacity(rows * size * 4);
        let mut puddle_propensity = Vec::with_capacity(rows * size);
        let step = self.step;

        for tz in row_start..row_end {
            for tx in 0..size {
                // Texel centre in world metres.
                let wx = (tx as f32 + 0.5) / MAP_SIZE as f32 * self.extent_x;
                let wz = (tz as f32 + 0.5) / MAP_SIZE as f32 * self.extent_z;
                let y = self.height_at(wx, wz);

                // Macro normal from central differences at sub-grid step.
                let dx = self.height_at(wx + step, wz) - self.height_at(wx - step, wz);
                let dz = self.height_at(wx, wz + step) - self.height_at(wx, wz - step);
                // Teren B2: the crown's numerical gradient rides the same central
                // difference — the cross-road perpendicular emerges from the distance rule
                // itself, no segment geometry re-derived. It bends ONLY the packed visual
                // normal and the pooling flatness (a crowned road sheds water); the layer
                // WEIGHTS keep the base normal, so the splat stays bit-identical to what
                // physics reads through `weights_at` — the one-rule discipline holds. The
                // cheap gate keeps the four extra distance walks off road-free texels.
                let (mut view_dx, mut view_dz) = (dx, dz);
                let roads = &self.battlefield.roads;
                let bounds = &self.road_bounds;
                let near_road = roads.iter().zip(bounds).any(|(road, bound)| {
                    wx >= bound.min[0]
                        && wx <= bound.max[0]
                        && wz >= bound.min[1]
                        && wz <= bound.max[1]
                        && road.distance_to(wx, wz) < road.width_m * 0.5 + step
                });
                if near_road {
                    view_dx += road_crown_m(roads, bounds, wx + step, wz)
                        - road_crown_m(roads, bounds, wx - step, wz);
                    view_dz += road_crown_m(roads, bounds, wx, wz + step)
                        - road_crown_m(roads, bounds, wx, wz - step);
                }
                let inv_len =
                    1.0 / (dx * dx + (2.0 * step) * (2.0 * step) + dz * dz).sqrt().max(1.0e-6);
                let n = [-dx * inv_len, 2.0 * step * inv_len, -dz * inv_len];
                let view_inv = 1.0
                    / (view_dx * view_dx + (2.0 * step) * (2.0 * step) + view_dz * view_dz)
                        .sqrt()
                        .max(1.0e-6);
                let view_n = [-view_dx * view_inv, 2.0 * step * view_inv, -view_dz * view_inv];
                // Pooling is broad terrain truth, not a per-fragment normal threshold. A wide
                // flatness ramp plus a weak local-depression preference is blurred below before
                // entering alpha, so neither the 5 m height grid nor its triangle diagonal
                // survives. It reads the CROWNED normal: a cambered road sheds its water.
                let flatness = pooling_smoothstep(0.94, 0.997, view_n[1]);
                let basin_radius_m = 8.0;
                let neighbour_mean = (self.height_at(wx - basin_radius_m, wz)
                    + self.height_at(wx + basin_radius_m, wz)
                    + self.height_at(wx, wz - basin_radius_m)
                    + self.height_at(wx, wz + basin_radius_m))
                    * 0.25;
                let basin = pooling_smoothstep(-0.12, 0.18, neighbour_mean - y);
                // Pooling comes from the stronger of two truths: local flatness/basin, or
                // the drainage line the classifier's flow lane traces — the same MAX rule
                // the splat's moisture uses, so gloss and tone agree on where wet is.
                let pooling = (flatness * (0.82 + basin * 0.18))
                    .max(self.ground.flow_wetness_at(wx, wz) * FLOW_PUDDLE_GAIN);
                puddle_propensity.push(pooling);
                macro_normal.extend([
                    ((view_n[0] * 0.5 + 0.5) * 255.0).round() as u8,
                    ((view_n[1] * 0.5 + 0.5) * 255.0).round() as u8,
                    ((view_n[2] * 0.5 + 0.5) * 255.0).round() as u8,
                    0,
                ]);

                // Layer weights come from the shared ground rule (`terrain::GroundClassifier`),
                // which is the same call the drive reads for grip and rolling resistance. The
                // height and the macro normal are already in hand, so the bake hands them over
                // rather than paying for them twice.
                let [grass, straw, dirt, rock] = self.ground.weights_from(y, n[1], wx, wz);
                // Normalize into RGBA8; rock and dirt claim their share first, grass/straw fill.
                let total = (grass + straw + dirt + rock).max(1.0e-4);
                let quantize = |w: f32| ((w / total) * 255.0).round().clamp(0.0, 255.0) as u8;
                splat.extend([quantize(grass), quantize(straw), quantize(dirt), quantize(rock)]);
            }
        }
        GroundBakeRows { splat, macro_normal, puddle_propensity }
    }
}

/// One thread's slice of the bake, in row order.
struct GroundBakeRows {
    splat: Vec<u8>,
    macro_normal: Vec<u8>,
    puddle_propensity: Vec<f32>,
}

/// Row ranges to bake concurrently — one per worker lane (see [`bake_lane_count`]), never more
/// ranges than rows.
fn ground_bake_row_chunks(size: usize) -> Vec<(usize, usize)> {
    let rows_per_chunk = size.div_ceil(bake_lane_count(size));
    (0..size)
        .step_by(rows_per_chunk)
        .map(|start| (start, (start + rows_per_chunk).min(size)))
        .collect()
}

/// Bake the splat + macro-normal maps for a battlefield. UV spans the heightmap's ground
/// extent exactly: `uv = world.xz / extent`.
///
/// The per-texel loop is the single most expensive step of building a scene (measured 362 ms
/// of a 517 ms map swap, release, Ostrogorsk), and every texel is independent — so it is baked
/// as parallel row ranges and concatenated in order. The split is invisible in the output:
/// `chunking_the_ground_bake_does_not_move_a_single_texel` locks that, and the ground-map
/// goldens lock the content itself.
pub fn bake_terrain_ground_maps(battlefield: &BattlefieldMap) -> TerrainGroundMaps {
    let size = MAP_SIZE as usize;
    let context = GroundBakeContext::new(battlefield);
    let chunks = ground_bake_row_chunks(size);

    let baked: Vec<GroundBakeRows> = if chunks.len() < 2 {
        vec![context.bake_rows(0, size)]
    } else {
        let context = &context;
        std::thread::scope(|scope| {
            let handles: Vec<_> = chunks
                .iter()
                .map(|&(start, end)| scope.spawn(move || context.bake_rows(start, end)))
                .collect();
            handles.into_iter().map(|handle| handle.join().expect("ground map row bake")).collect()
        })
    };

    let mut splat = Vec::with_capacity(size * size * 4);
    let mut macro_normal = Vec::with_capacity(size * size * 4);
    let mut puddle_propensity = Vec::with_capacity(size * size);
    for rows in baked {
        splat.extend(rows.splat);
        macro_normal.extend(rows.macro_normal);
        puddle_propensity.extend(rows.puddle_propensity);
    }

    let extent_x = context.extent_x;
    let extent_z = context.extent_z;
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

    /// Cutting the bake into thread ranges is a SCHEDULING decision, never a content one. This
    /// is the lock that let the per-texel loop go parallel: bake a row span whole, bake it as
    /// two uneven halves, and demand the same bytes — an off-by-one at a chunk seam, or any
    /// state quietly carried between texels, shows up here as a diff.
    #[test]
    fn chunking_the_ground_bake_does_not_move_a_single_texel() {
        let battlefield = map_forge::battlefield(MapId::BystraValley);
        let context = GroundBakeContext::new(&battlefield);

        // A span in the middle of the map, split at a deliberately ugly row.
        let whole = context.bake_rows(100, 140);
        let head = context.bake_rows(100, 117);
        let tail = context.bake_rows(117, 140);

        let splat: Vec<u8> = head.splat.iter().chain(&tail.splat).copied().collect();
        let macro_normal: Vec<u8> =
            head.macro_normal.iter().chain(&tail.macro_normal).copied().collect();
        let puddle: Vec<f32> =
            head.puddle_propensity.iter().chain(&tail.puddle_propensity).copied().collect();

        assert_eq!(splat, whole.splat, "the splat map must not notice the seam");
        assert_eq!(macro_normal, whole.macro_normal, "the macro normal must not notice the seam");
        assert_eq!(puddle, whole.puddle_propensity, "pooling must not notice the seam");
    }

    /// The chunk plan tiles the map exactly once: no gap leaves a band of the world unbaked,
    /// no overlap makes the concatenation longer than the texture.
    #[test]
    fn the_ground_bake_chunk_plan_tiles_every_row_exactly_once() {
        let size = MAP_SIZE as usize;
        let chunks = ground_bake_row_chunks(size);
        assert!(!chunks.is_empty());
        assert_eq!(chunks[0].0, 0, "the plan starts at the first row");
        assert_eq!(chunks.last().expect("non-empty").1, size, "the plan ends at the last row");
        for window in chunks.windows(2) {
            assert_eq!(window[0].1, window[1].0, "chunks meet without gap or overlap");
        }
        assert!(chunks.iter().all(|&(start, end)| start < end), "no empty chunk is scheduled");
    }

    fn flat_battlefield(roads: Vec<terrain::Road>) -> BattlefieldMap {
        let heightmap = terrain::HeightMap::flat(65, 65, 5.0, 1.0).expect("flat map");
        BattlefieldMap {
            id: "flat".into(),
            name: "flat".into(),
            size_m: heightmap.extent_m(),
            historical_basis: String::new(),
            design_notes: vec![],
            heightmap,
            water: None,
            river: None,
            spawn_zones: vec![],
            capture_zones: vec![],
            strategic_points: vec![],
            features: vec![],
            static_cover: vec![],
            scenery: vec![],
            roads,
        }
    }

    /// Teren B2: the baked macro normal wears the road's crown — up at the apex, tilting
    /// OUTWARD across the carriageway, and bit-identical to a roadless bake past the kerb.
    #[test]
    fn a_road_wears_a_crown_in_the_baked_normal() {
        let road = terrain::Road {
            id: "lane".into(),
            surface: terrain::RoadSurface::Dirt,
            points: vec![[0.0, 160.0], [320.0, 160.0]],
            width_m: 8.0,
        };
        let crowned_map = flat_battlefield(vec![road]);
        let bare_map = flat_battlefield(vec![]);
        let crowned = GroundBakeContext::new(&crowned_map).bake_rows(500, 535);
        let bare = GroundBakeContext::new(&bare_map).bake_rows(500, 535);
        let normal_at = |rows: &GroundBakeRows, tx: usize, tz: usize| -> [f32; 3] {
            let index = ((tz - 500) * MAP_SIZE as usize + tx) * 4;
            std::array::from_fn(|axis| {
                f32::from(rows.macro_normal[index + axis]) / 255.0 * 2.0 - 1.0
            })
        };
        // Texel rows: wz = (tz + 0.5) / 1024 * 320 — the apex sits at wz ≈ 160 (tz 512),
        // the shoulder probe at wz ≈ 162 (tz 518), the beyond-kerb probe at wz ≈ 166.7.
        let apex = normal_at(&crowned, 512, 512);
        assert!(apex[1] > 0.99, "the crown's apex reads flat-up, got {apex:?}");
        let shoulder = normal_at(&crowned, 512, 518);
        assert!(
            shoulder[2] > 0.01,
            "the shoulder must tilt outward, away from the centerline: {shoulder:?}"
        );
        assert_eq!(
            normal_at(&crowned, 512, 533),
            normal_at(&bare, 512, 533),
            "past the kerb the bake is bit-identical to a roadless map"
        );
    }

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

        // Ostrogorsk's stone lane is GRANITE (teren A2): the cobbled boulevard splats as
        // rock, so the layer's albedo is the street's — cool set stone, never steppe chalk.
        let city = terrain_material_set_for(MapId::Ostrogorsk);
        assert_eq!(city.layers[3].albedo, [0.36, 0.37, 0.4]);
        assert!(
            city.layers[3].albedo[2] > city.layers[3].albedo[0],
            "granite leans cool: b must beat r"
        );
    }
}
