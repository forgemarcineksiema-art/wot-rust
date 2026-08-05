//! Near-field grass (Materia Świata 1b): a deterministic, world-anchored population of blade
//! tufts cached around the camera. A six-metre population margin surrounds the visible ring,
//! so crossing a cache boundary only streams already-invisible tufts; the shader owns the
//! continuous camera-distance fade. Density and colour come from the baked splat at each tuft,
//! so narrow roads, rock and the riverbed remain bare instead of inheriting a cell-centre label.

use glam::{Mat4, Vec3};
use renderer_api::{
    MaterialHandle, MeshAsset, MeshHandle, RenderObject, SceneVertex, TerrainGroundMaps,
    TerrainMaterialSet,
};
use terrain::{HeightMap, WaterBody};

/// The scene-registry handle the tuft mesh lives under — inside the shadowless-dressing band
/// (see `renderer_api::SHADOWLESS_DRESSING_MESH_BASE`): grass draws in the color pass only,
/// never into the shadow cascades or the SSAO prepass.
pub const GRASS_MESH_HANDLE: MeshHandle = MeshHandle(0xFFFF_0001);
// Compile-time lock: grass must skip the shadow cascades and the SSAO prepass.
const _: () = assert!(GRASS_MESH_HANDLE.0 >= renderer_api::SHADOWLESS_DRESSING_MESH_BASE);

/// End of the shader-visible blade ring. CPU population extends past this by
/// [`GRASS_CACHE_MARGIN_M`], so cache rebuilds never stream visible tufts.
pub const GRASS_RADIUS_M: f32 = 48.0;
/// Extra invisible population kept around the visible ring. This is larger than one normal
/// four-metre cache step, leaving headroom for a render frame that overshoots the threshold.
pub const GRASS_CACHE_MARGIN_M: f32 = 6.0;
pub const GRASS_CACHE_RADIUS_M: f32 = GRASS_RADIUS_M + GRASS_CACHE_MARGIN_M;
/// Scatter cell edge; tufts are conjured per cell from the cell's own hash. Shared with the
/// far-tuft bake (`grass_cards`): both costumes read the SAME cells.
pub(crate) const CELL_M: f32 = 8.0;
/// Fixed world population ceiling per 8 m cell. Vegetation acceptance can only remove from
/// this deterministic candidate sequence; camera movement never changes a cell's population.
pub(crate) const CELL_TUFT_CANDIDATES: u32 = 28;
/// Full-vegetation population budget for the 54 m cache disc. This is a verification guard,
/// not a runtime crop: hard truncation would reintroduce a moving wall at the cache boundary.
pub const MAX_GRASS_INSTANCES: usize = 4_800;
/// Ground with less vegetation weight than this grows nothing (roads, rock, riverbed).
const MIN_VEG_WEIGHT: f32 = 0.35;
/// Standing water drowns the tufts.
const MAX_WATER_DEPTH_M: f32 = 0.05;
/// A crater's kill zone: nothing grows in the bowl or on the fresh spoil. One factor for
/// BOTH costumes — a burst that mows the near ring mows the far meadow identically.
pub(crate) const CRATER_KILL_FACTOR: f32 = 1.45;
/// The tallest blade the kernel emits, mesh-local (locked to the mesh by test). The far
/// costume's peak height is exactly this times the candidate's scale — height continuity
/// across the hand-off is this single number.
pub(crate) const TUFT_MESH_TALLEST_M: f32 = 0.34;

/// The honesty cap (Jedna Trawa D1): no grass, at any instance scale, stands taller than
/// this. There is no camouflage mechanic, so grass that hid a tank would lie about gameplay.
pub const GRASS_HEIGHT_CAP_M: f32 = 0.6;
/// Instance-scale band the conjurer draws from (size = MIN + hash * SPAN). Named so the
/// height-cap lock can multiply the tallest mesh blade by the largest scale the field uses.
pub const TUFT_SCALE_MIN: f32 = 1.0;
pub const TUFT_SCALE_SPAN: f32 = 0.6;

/// One tuft (blade kernel 2.0, Jedna Trawa P1): ten blades arcing outward from a common
/// root, each a two-segment curve — root station, mid station, and a POINTED tip. The
/// serrated tuft silhouette is nothing but the per-blade height spread meeting sharp tips;
/// the arc is visible as the mid station leaning part-way and the tip reaching full out.
/// Both faces are wound (the scene pipeline culls back faces; a blade must read from
/// anywhere). Vertices carry a white-to-dusk gradient with `tint_weight` 1.0 — the INSTANCE
/// tint is the actual grass colour, so one mesh serves every plot tone on the map.
pub fn grass_tuft_mesh() -> MeshAsset {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let blades = 10;
    for blade in 0..blades {
        let angle =
            blade as f32 / blades as f32 * std::f32::consts::TAU + ((blade % 3) as f32) * 0.35;
        let (sin, cos) = angle.sin_cos();
        let dir = Vec3::new(cos, 0.0, sin);
        // Strong per-blade height spread: 0.14–0.34 mesh-local — the serration IS this.
        let height = 0.14 + 0.20 * ((blade * 7 + 3) % 6) as f32 / 5.0;
        // Horizontal reach at the tip; the mid station takes only 35 % of it, so the blade
        // profile is a convex outward curve, not a straight lean.
        let arc = 0.06 + 0.10 * ((blade * 5 + 1) % 4) as f32 / 3.0;
        let root = dir * (0.05 + 0.05 * ((blade % 4) as f32 / 3.0));
        let across = Vec3::new(-sin, 0.0, cos);
        let half_w = 0.016 + 0.006 * ((blade % 3) as f32 / 2.0);
        let mid = root + dir * (arc * 0.35) + Vec3::Y * (height * 0.58);
        let tip = root + dir * arc + Vec3::Y * height;
        // Base sits in shade, the tip catches the sky — a cheap self-shadow gradient. The
        // normals stand mostly UP so a blade takes the same sun the ground under it does —
        // side-facing normals read as a dark alien succulent, not grass.
        let base_tone = [0.72, 0.72, 0.72];
        let mid_tone = [0.9, 0.9, 0.9];
        let tip_tone = [1.05, 1.05, 1.05];
        let normal = Vec3::new(cos * 0.3, 1.0, sin * 0.3).normalize().to_array();
        let base = vertices.len() as u32;
        // Roots stay planted (sway 0); the wind lane grows QUADRATICALLY with the station's
        // relative height (0.58² ≈ 0.34), so the shader bends the blade as an arc pinned at
        // the root instead of hinging a rigid stick.
        let tip_sway = 0.35 + height * 0.8;
        let mid_sway = tip_sway * 0.34;
        for (position, tone, sway, taper) in [
            (root, base_tone, 0.0, 1.0),
            (mid, mid_tone, mid_sway, 0.55),
            (tip, tip_tone, tip_sway, 0.0),
        ] {
            // The tip's taper is 0: both "sides" collapse into one point — pushed once.
            if taper <= 0.0 {
                vertices.push(SceneVertex {
                    position: position.to_array(),
                    normal,
                    color: tone,
                    tint_weight: 1.0,
                    gloss: 0.05,
                    surface: renderer_api::surface_role::GRASS_BLADE,
                    sway,
                    uv: [0.0, 0.0],
                    bounce: [0.0; 3],
                });
            } else {
                for side in [-1.0f32, 1.0] {
                    vertices.push(SceneVertex {
                        position: (position + across * (half_w * taper * side)).to_array(),
                        normal,
                        color: tone,
                        tint_weight: 1.0,
                        gloss: 0.05,
                        surface: renderer_api::surface_role::GRASS_BLADE,
                        sway,
                        uv: [0.0, 0.0],
                        bounce: [0.0; 3],
                    });
                }
            }
        }
        // Vertex order per blade: r-, r+, m-, m+, tip. Front and back faces: the pipeline
        // culls, the blade must not vanish from behind.
        let (r0, r1, m0, m1, t) = (base, base + 1, base + 2, base + 3, base + 4);
        indices.extend_from_slice(&[r0, r1, m1, r0, m1, m0, m0, m1, t]);
        indices.extend_from_slice(&[m1, r1, r0, m0, m1, r0, m1, m0, t]);
    }
    MeshAsset::new(vertices, indices)
}

/// The vegetation weight (grass + straw splat channels, 0..1) standing at a world position.
pub(crate) fn vegetation_weight(maps: &TerrainGroundMaps, x: f32, z: f32) -> f32 {
    let size = maps.size as usize;
    let tx = ((x / maps.extent_m[0]) * maps.size as f32).clamp(0.0, maps.size as f32 - 1.0);
    let tz = ((z / maps.extent_m[1]) * maps.size as f32).clamp(0.0, maps.size as f32 - 1.0);
    let index = (tz as usize * size + tx as usize) * 4;
    let total: u32 = maps.splat[index..index + 4].iter().map(|&w| u32::from(w)).sum();
    if total == 0 {
        return 0.0;
    }
    (u32::from(maps.splat[index]) + u32::from(maps.splat[index + 1])) as f32 / total as f32
}

/// How hard a candidate is pulled toward its nearest clump centre (D7). A convex lerp
/// between two in-cell points, so a pulled candidate NEVER leaves its cell — the per-cell
/// determinism and phase-sweep contracts hold by construction.
pub(crate) const CLUMP_PULL: f32 = 0.55;
/// Baldness threshold: where the low-frequency meadow noise dips below this, nothing grows
/// — real fields hold bare patches, not a uniform carpet. Redistribution, not addition:
/// budgets and candidate counts stay exactly what they were.
pub(crate) const BALD_CUT: f32 = 0.24;

/// 2–3 deterministic clump centres per cell, in cell-local metres. Always draws the same
/// number of lanes from the seed stream, so the candidate stream after it never shifts.
pub(crate) fn clump_centres(seed: &mut u64) -> ([(f32, f32); 3], usize) {
    let count = if game_core::math::next_hash_unit(seed) < 0.5 { 2 } else { 3 };
    let mut centres = [(0.0, 0.0); 3];
    for centre in &mut centres {
        *centre = (
            game_core::math::next_hash_unit(seed) * CELL_M,
            game_core::math::next_hash_unit(seed) * CELL_M,
        );
    }
    (centres, count)
}

/// Pull a cell-local candidate toward its nearest centre.
pub(crate) fn pull_toward_clump(
    x: f32,
    z: f32,
    centres: &[(f32, f32); 3],
    count: usize,
) -> (f32, f32) {
    let mut nearest = centres[0];
    let mut best = f32::INFINITY;
    for &(cx, cz) in &centres[..count] {
        let d = (x - cx).hypot(z - cz);
        if d < best {
            best = d;
            nearest = (cx, cz);
        }
    }
    (x + (nearest.0 - x) * CLUMP_PULL, z + (nearest.1 - z) * CLUMP_PULL)
}

/// The meadow's baldness field: low-frequency world-anchored noise. Sample it at the
/// FOLDED z (`z.min(extent_z - z)`) — the whole population mirrors its south-half
/// decisions north, so the field's bare patches agree across the axis by construction.
pub(crate) fn meadow_baldness(x: f32, z_folded: f32) -> f32 {
    terrain::value_noise(x / 31.0, z_folded / 31.0)
}

/// One candidate drawn from a cell's stream: a would-be tuft with every per-tuft lane the
/// population owns. Whether it STANDS is decided later, per emitted position, by
/// [`tuft_ground`] — the stream itself never depends on the eye or the map state.
pub(crate) struct TuftCandidate {
    pub x: f32,
    pub z: f32,
    pub yaw: f32,
    pub size: f32,
    pub tone: f32,
    pub vegetation_lane: f32,
}

/// THE candidate stream (Jedna Trawa P3): one deterministic per-cell sequence that BOTH
/// costumes read — the near ring conjures full tufts from it, the far bake reads the same
/// cells in the same order and can therefore only stand a card where a tuft stands. Lane
/// order is frozen by the unification and anti-streaming locks: cell_dry, clump centres,
/// then per candidate raw position, yaw, size, tone, vegetation lane.
pub(crate) struct CellStream {
    seed: u64,
    origin_x: f32,
    origin_z: f32,
    pub cell_dry: f32,
    centres: [(f32, f32); 3],
    centre_count: usize,
}

impl CellStream {
    pub(crate) fn new(cx: i32, cz: i32) -> Self {
        let mut seed = (cx as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ (cz as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
            ^ 0x6265_7472_6177_6121;
        // The cell's own dryness lane: tufts lean grass or straw per plot, echoing the
        // shader's field quilt without resampling it.
        let cell_dry = game_core::math::next_hash_unit(&mut seed);
        // D7: grass grows in CLUMPS around 2–3 hash centres, not uniformly at random.
        let (centres, centre_count) = clump_centres(&mut seed);
        Self {
            seed,
            origin_x: cx as f32 * CELL_M,
            origin_z: cz as f32 * CELL_M,
            cell_dry,
            centres,
            centre_count,
        }
    }

    pub(crate) fn next_candidate(&mut self) -> TuftCandidate {
        let raw_x = game_core::math::next_hash_unit(&mut self.seed) * CELL_M;
        let raw_z = game_core::math::next_hash_unit(&mut self.seed) * CELL_M;
        let yaw = game_core::math::next_hash_unit(&mut self.seed) * std::f32::consts::TAU;
        let size =
            TUFT_SCALE_MIN + game_core::math::next_hash_unit(&mut self.seed) * TUFT_SCALE_SPAN;
        let tone = game_core::math::next_hash_unit(&mut self.seed);
        let vegetation_lane = game_core::math::next_hash_unit(&mut self.seed);
        // The pull is a convex combination of two in-cell points — the candidate stays in
        // its cell, so every per-cell contract survives unchanged.
        let (local_x, local_z) = pull_toward_clump(raw_x, raw_z, &self.centres, self.centre_count);
        TuftCandidate {
            x: self.origin_x + local_x,
            z: self.origin_z + local_z,
            yaw,
            size,
            tone,
            vegetation_lane,
        }
    }
}

/// The ground both costumes stand on: one bundle of the map facts acceptance needs, so the
/// ONE rule below is called identically by the near ring and the far bake.
pub(crate) struct MeadowGround<'a> {
    pub maps: &'a TerrainGroundMaps,
    pub heightmap: &'a HeightMap,
    pub water: Option<WaterBody>,
    pub cover: &'a [terrain::StaticCoverObject],
}

impl MeadowGround<'_> {
    /// The ONE acceptance rule: does a tuft stand at this exact position? Vegetation is
    /// sampled at the candidate (with its stochastic lane, so partial splat weights thin
    /// the field instead of drawing hard cell borders), craters kill, cover floors exclude,
    /// water drowns. Both costumes call this — a far card cannot stand where the near ring
    /// would refuse.
    pub(crate) fn tuft_ground(
        &self,
        craters: &[(f32, f32, f32)],
        x: f32,
        z: f32,
        vegetation_lane: f32,
    ) -> Option<f32> {
        let veg = vegetation_weight(self.maps, x, z);
        if veg < MIN_VEG_WEIGHT || vegetation_lane >= veg {
            return None;
        }
        if craters.iter().any(|&(kx, kz, kill)| (x - kx).hypot(z - kz) < kill) {
            return None; // burned and buried where the shell landed
        }
        if terrain::inside_any_cover(self.cover, x, z, 0.0) {
            return None; // grass does not grow through a tenement floor
        }
        let ground = self.heightmap.sample_height(x, z)?;
        if self.water.is_some_and(|w| w.depth_over(ground) > MAX_WATER_DEPTH_M) {
            return None;
        }
        Some(ground)
    }
}

/// Build the stable grass population cached around `eye`. Every cell owns exactly one candidate
/// sequence; local splat acceptance, terrain, water and craters may remove candidates, but the
/// eye never changes their rank, size or transform. The shader performs the visible 34–48 m
/// fade every frame, while this CPU population extends to [`GRASS_CACHE_RADIUS_M`].
pub fn grass_frame_objects(
    heightmap: &HeightMap,
    water: Option<WaterBody>,
    cover: &[terrain::StaticCoverObject],
    maps: &TerrainGroundMaps,
    materials: &TerrainMaterialSet,
    eye: Vec3,
) -> Vec<RenderObject> {
    let mut objects = Vec::with_capacity(MAX_GRASS_INSTANCES);
    let extent_z = heightmap.extent_m()[1];
    let mirror_z = extent_z * 0.5;
    let south_rows = (mirror_z / CELL_M).ceil() as i32;
    let min_cx = ((eye.x - GRASS_CACHE_RADIUS_M) / CELL_M).floor() as i32;
    let max_cx = ((eye.x + GRASS_CACHE_RADIUS_M) / CELL_M).floor() as i32;
    // The fold (Jedna Trawa P3): the population is generated for the SOUTH half and emitted
    // with its exact mirrored twin — the rule the card meadow always had, now owned by the
    // one stream both costumes read. A south source row is visited when its own band or its
    // twin's band can reach the cache disc.
    let direct_min = ((eye.z - GRASS_CACHE_RADIUS_M) / CELL_M).floor() as i32;
    let direct_max = ((eye.z + GRASS_CACHE_RADIUS_M) / CELL_M).floor() as i32;
    let twin_min = ((extent_z - (eye.z + GRASS_CACHE_RADIUS_M)) / CELL_M).floor() as i32;
    let twin_max = ((extent_z - (eye.z - GRASS_CACHE_RADIUS_M)) / CELL_M).floor() as i32;
    // A high-explosive burst burns and buries the grass it lands on: the replicated crater
    // ledger (already folded into the heightmap) is a kill list — nothing grows inside a
    // bowl or on its fresh spoil rim.
    let craters: Vec<(f32, f32, f32)> = heightmap
        .crater_records()
        .iter()
        .map(|crater| (crater.x_m(), crater.z_m(), crater.radius_m() * CRATER_KILL_FACTOR))
        .collect();
    // Craters whose kill zone can touch a cell band — usually none, so the per-tuft test
    // costs nothing on virgin ground. The twin band gets its own list at the mirrored z.
    let nearby = |center_x: f32, center_z: f32| -> Vec<(f32, f32, f32)> {
        let cell_reach = CELL_M * 0.75;
        craters
            .iter()
            .copied()
            .filter(|&(x, z, kill)| {
                let dx = (x - center_x).abs() - cell_reach;
                let dz = (z - center_z).abs() - cell_reach;
                dx.max(0.0).hypot(dz.max(0.0)) <= kill
            })
            .collect()
    };
    let meadow = MeadowGround { maps, heightmap, water, cover };
    for cz in 0..south_rows {
        let in_direct = (direct_min..=direct_max).contains(&cz);
        let in_twin = (twin_min..=twin_max).contains(&cz);
        if !in_direct && !in_twin {
            continue;
        }
        for cx in min_cx..=max_cx {
            let mut stream = CellStream::new(cx, cz);
            let center_x = (cx as f32 + 0.5) * CELL_M;
            let center_z = (cz as f32 + 0.5) * CELL_M;
            let craters_direct = nearby(center_x, center_z);
            let craters_twin = nearby(center_x, extent_z - center_z);
            for _ in 0..CELL_TUFT_CANDIDATES {
                let candidate = stream.next_candidate();
                // The mid row seeds only its own half, so the fold never doubles a tuft.
                if candidate.z >= mirror_z {
                    continue;
                }
                // Bare patches: redistribution, not addition — the candidate budget stays
                // 28, the meadow just refuses the low-noise ground. The source z IS the
                // folded coordinate, so both twins inherit one decision.
                if meadow_baldness(candidate.x, candidate.z) < BALD_CUT {
                    continue;
                }
                for (px, pz, pyaw, kill_list) in [
                    (candidate.x, candidate.z, candidate.yaw, &craters_direct),
                    (
                        candidate.x,
                        extent_z - candidate.z,
                        std::f32::consts::TAU - candidate.yaw,
                        &craters_twin,
                    ),
                ] {
                    let flat = Vec3::new(px - eye.x, 0.0, pz - eye.z).length();
                    if flat > GRASS_CACHE_RADIUS_M {
                        continue;
                    }
                    let Some(ground) =
                        meadow.tuft_ground(kill_list, px, pz, candidate.vegetation_lane)
                    else {
                        continue;
                    };
                    // A touch lighter than the soil it stands on: blades catch more sky.
                    let Some(albedo) = crate::grass_cards::card_albedo(
                        maps,
                        materials,
                        px,
                        pz,
                        stream.cell_dry * 0.5 + candidate.tone * 0.5,
                    ) else {
                        continue;
                    };
                    let transform = Mat4::from_translation(Vec3::new(px, ground, pz))
                        * Mat4::from_rotation_y(pyaw)
                        * Mat4::from_scale(Vec3::splat(candidate.size));
                    objects.push(RenderObject {
                        tank_id: None,
                        mesh: GRASS_MESH_HANDLE,
                        material: MaterialHandle(0),
                        transform: transform.to_cols_array_2d(),
                        tint: albedo,
                    });
                }
            }
        }
    }
    objects
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full_veg_maps(extent: f32) -> TerrainGroundMaps {
        // 2x2 texels, all weight on the grass channel.
        let mut splat = Vec::new();
        for _ in 0..4 {
            splat.extend_from_slice(&[255, 0, 0, 0]);
        }
        TerrainGroundMaps {
            size: 2,
            splat,
            macro_normal: vec![128; 2 * 2 * 4],
            extent_m: [extent, extent],
        }
    }

    fn bare_dirt_maps(extent: f32) -> TerrainGroundMaps {
        let mut splat = Vec::new();
        for _ in 0..4 {
            splat.extend_from_slice(&[0, 0, 255, 0]);
        }
        TerrainGroundMaps {
            size: 2,
            splat,
            macro_normal: vec![128; 2 * 2 * 4],
            extent_m: [extent, extent],
        }
    }

    fn maps_with_dirt_strip(extent: f32, strip_min_x: f32, strip_max_x: f32) -> TerrainGroundMaps {
        const SIZE: usize = 256;
        let mut splat = Vec::with_capacity(SIZE * SIZE * 4);
        for _tz in 0..SIZE {
            for tx in 0..SIZE {
                let texel_center_x = (tx as f32 + 0.5) * extent / SIZE as f32;
                if (strip_min_x..strip_max_x).contains(&texel_center_x) {
                    splat.extend_from_slice(&[0, 0, 255, 0]);
                } else {
                    splat.extend_from_slice(&[255, 0, 0, 0]);
                }
            }
        }
        TerrainGroundMaps {
            size: SIZE as u32,
            splat,
            macro_normal: vec![128; SIZE * SIZE * 4],
            extent_m: [extent, extent],
        }
    }

    fn tuft_key(object: &RenderObject) -> (u32, u32) {
        (object.transform[3][0].to_bits(), object.transform[3][2].to_bits())
    }

    fn tuft_flat_distance(object: &RenderObject, eye: Vec3) -> f32 {
        (object.transform[3][0] - eye.x).hypot(object.transform[3][2] - eye.z)
    }

    fn flat_ground() -> HeightMap {
        HeightMap::flat(65, 65, 4.0, 1.0).expect("flat map")
    }

    fn mean_nearest_neighbour(points: &[(f32, f32)]) -> f32 {
        let mut total = 0.0;
        for (i, &(x, z)) in points.iter().enumerate() {
            let mut best = f32::INFINITY;
            for (j, &(ox, oz)) in points.iter().enumerate() {
                if i != j {
                    best = best.min((x - ox).hypot(z - oz));
                }
            }
            total += best;
        }
        total / points.len() as f32
    }

    #[test]
    fn tufts_clump_instead_of_scattering_uniformly() {
        // D7's closing lock, in the Clark–Evans direction: clumping pulls the MEAN
        // nearest-neighbour distance well under the uniform-Poisson expectation
        // (0.5 / sqrt(density)). Variance would be the wrong statistic — clustered points
        // can move it either way.
        let ground = flat_ground();
        let eye = Vec3::new(128.0, 3.0, 128.0);
        let materials = TerrainMaterialSet::default();
        let grown = grass_frame_objects(&ground, None, &[], &full_veg_maps(256.0), &materials, eye);
        let tufts: Vec<(f32, f32)> = grown
            .iter()
            .map(|object| (object.transform[3][0], object.transform[3][2]))
            .filter(|(x, z)| (x - eye.x).hypot(z - eye.z) < 40.0)
            .collect();
        assert!(tufts.len() > 400, "enough tufts for the statistic: {}", tufts.len());
        let density = tufts.len() as f32 / (std::f32::consts::PI * 40.0 * 40.0);
        let uniform_expectation = 0.5 / density.sqrt();
        let clumped = mean_nearest_neighbour(&tufts);
        assert!(
            clumped < 0.85 * uniform_expectation,
            "the field must clump (mean NN {clumped:.3} vs uniform {uniform_expectation:.3})"
        );
    }

    #[test]
    fn the_meadow_holds_bald_patches_and_stays_lush_around_them() {
        // Where the low-frequency noise dips, NOTHING grows — a whole cell stands bare
        // while the field around it keeps its budgeted density. The test lets the field's
        // own rule NAME a bald cell first (all five probes under the cut), then demands
        // the meadow honored it. If baldness ever silently disappears, the finder fails
        // loudly — that is the lock.
        let ground = flat_ground();
        let extent_z = ground.extent_m()[1];
        let bald = (2..30)
            .flat_map(|cz| (2..30).map(move |cx| (cx, cz)))
            .find(|&(cx, cz): &(i32, i32)| {
                let x0 = cx as f32 * CELL_M;
                let z0 = cz as f32 * CELL_M;
                [(0.0, 0.0), (CELL_M, 0.0), (0.0, CELL_M), (CELL_M, CELL_M), (4.0, 4.0)].iter().all(
                    |&(dx, dz)| {
                        let z = z0 + dz;
                        meadow_baldness(x0 + dx, z.min(extent_z - z)) < BALD_CUT
                    },
                )
            })
            .expect("a 256 m field holds at least one fully bald cell");
        let eye = Vec3::new((bald.0 as f32 + 0.5) * CELL_M, 3.0, (bald.1 as f32 + 0.5) * CELL_M);
        let materials = TerrainMaterialSet::default();
        let grown = grass_frame_objects(&ground, None, &[], &full_veg_maps(256.0), &materials, eye);
        let inside_bald = grown.iter().any(|object| {
            (object.transform[3][0] / CELL_M).floor() as i32 == bald.0
                && (object.transform[3][2] / CELL_M).floor() as i32 == bald.1
        });
        assert!(!inside_bald, "nothing grows in the bald cell at {bald:?}");
        // The eye is parked ON the hole, so this window reads balder than any real camera
        // ever will — the every-phase density floor lives in the cache-sweep test. Here the
        // lock is only that a hole is a HOLE in a field, not a dead map.
        assert!(grown.len() > 2000, "the field around the holes stays alive: {}", grown.len());
    }

    #[test]
    fn a_cover_footprint_grows_no_grass() {
        // Grass never grew UNDER an authored building on purpose — it grew THROUGH the
        // floor because nothing told it the footprint existed (part of D19). Both twins of
        // the exclusion are the near ring's; the card meadow carries its own.
        let ground = flat_ground();
        let eye = Vec3::new(128.0, 3.0, 128.0);
        let block = terrain::StaticCoverObject {
            id: "tenement".into(),
            name: "Tenement".into(),
            kind: terrain::StaticCoverKind::CityBuilding,
            center: [128.0, 1.0, 128.0],
            half_extents_m: [6.0, 3.0, 5.0],
        };
        let materials = TerrainMaterialSet::default();
        let grown = grass_frame_objects(
            &ground,
            None,
            std::slice::from_ref(&block),
            &full_veg_maps(256.0),
            &materials,
            eye,
        );
        assert!(!grown.is_empty(), "the meadow around the building still grows");
        for tuft in &grown {
            let (x, z) = (tuft.transform[3][0], tuft.transform[3][2]);
            assert!(
                (x - 128.0).abs() > 6.0 || (z - 128.0).abs() > 5.0,
                "a tuft rooted inside the tenement footprint at ({x}, {z})"
            );
        }
    }

    #[test]
    fn grass_grows_on_vegetation_and_refuses_roads_water_and_the_far_field() {
        let ground = flat_ground();
        let materials = crate::terrain_maps::terrain_material_set_for(terrain::MapId::BystraValley);
        let eye = Vec3::new(128.0, 3.0, 128.0);

        let grown = grass_frame_objects(&ground, None, &[], &full_veg_maps(256.0), &materials, eye);
        assert!(
            grown.len() > 400 && grown.len() <= MAX_GRASS_INSTANCES,
            "a vegetated ring stands dense but budgeted, got {}",
            grown.len()
        );
        for tuft in &grown {
            let position = Vec3::new(tuft.transform[3][0], 0.0, tuft.transform[3][2]);
            let flat = (position - Vec3::new(eye.x, 0.0, eye.z)).length();
            assert!(
                flat <= GRASS_CACHE_RADIUS_M + 1.0e-3,
                "no tuft outside the cache population, got {flat}"
            );
            assert!(
                (tuft.transform[3][1] - 1.0).abs() < 1.0e-3,
                "every tuft roots on the sampled ground"
            );
        }

        let bare = grass_frame_objects(&ground, None, &[], &bare_dirt_maps(256.0), &materials, eye);
        assert!(bare.is_empty(), "a dirt road grows nothing, got {}", bare.len());

        let flood = Some(WaterBody { surface_level_m: 2.0 });
        let drowned =
            grass_frame_objects(&ground, flood, &[], &full_veg_maps(256.0), &materials, eye);
        assert!(drowned.is_empty(), "standing water drowns the tufts, got {}", drowned.len());
    }

    #[test]
    fn local_dirt_strip_stays_bare_inside_grassy_cells() {
        // The strip straddles the x=128 cell boundary but misses both adjacent 8 m cell
        // centres (124 and 132). A cell-centre vegetation gate therefore cannot pass this
        // test by accident: only per-tuft sampling can mow the narrow road correctly.
        const STRIP_MIN_X: f32 = 125.0;
        const STRIP_MAX_X: f32 = 131.0;
        let ground = flat_ground();
        let materials = crate::terrain_maps::terrain_material_set_for(terrain::MapId::BystraValley);
        let maps = maps_with_dirt_strip(256.0, STRIP_MIN_X, STRIP_MAX_X);
        let eye = Vec3::new(128.0, 3.0, 128.0);
        assert!(vegetation_weight(&maps, 124.0, eye.z) > 0.99);
        assert!(vegetation_weight(&maps, 132.0, eye.z) > 0.99);
        let grown = grass_frame_objects(&ground, None, &[], &maps, &materials, eye);

        let mut left = 0;
        let mut right = 0;
        for tuft in &grown {
            let x = tuft.transform[3][0];
            let z = tuft.transform[3][2];
            assert!(
                !(STRIP_MIN_X..STRIP_MAX_X).contains(&x),
                "per-tuft splat acceptance keeps the dirt strip bare, got ({x}, {z})"
            );
            if (112.0..STRIP_MIN_X).contains(&x) && (z - eye.z).abs() < 20.0 {
                left += 1;
            }
            if (STRIP_MAX_X..144.0).contains(&x) && (z - eye.z).abs() < 20.0 {
                right += 1;
            }
        }
        assert!(left > 30 && right > 30, "grass brackets the local dirt strip: {left}/{right}");
    }

    /// A shell hole is bare: no tuft stands inside a replicated crater's bowl or on its
    /// fresh spoil — the burst burned and buried them (Fizyczny Świat tie-in).
    #[test]
    fn grass_refuses_to_grow_in_a_fresh_crater() {
        let mut ground = flat_ground();
        let materials =
            crate::terrain_maps::terrain_material_set_for(terrain::MapId::ProkhorovkaHill252_2);
        let maps = full_veg_maps(260.0);
        let eye = Vec3::new(100.0, 8.0, 100.0);

        let crater = terrain::CraterRecord::from_world(
            103.0,
            100.0,
            2.4,
            0.8,
            terrain::CRATER_KIND_HIGH_EXPLOSIVE,
        );
        ground.set_craters(&[crater]);
        let after = grass_frame_objects(&ground, None, &[], &maps, &materials, eye);

        let kill = crater.radius_m() * 1.45;
        let mut ring_neighbours = 0;
        for tuft in &after {
            let x = tuft.transform[3][0];
            let z = tuft.transform[3][2];
            let dist = (x - crater.x_m()).hypot(z - crater.z_m());
            assert!(dist >= kill - 1.0e-3, "nothing grows in the bowl: tuft at ({x}, {z})");
            if dist < kill + 3.0 {
                ring_neighbours += 1;
            }
        }
        // The field DID sample this ground: live grass stands right outside the kill zone,
        // so the empty bowl is the burst's doing, not a hole in the scatter.
        assert!(ring_neighbours > 5, "the field surrounds the bowl: {ring_neighbours}");
    }

    /// Tone-lock (P3): a blade's tint IS the ground albedo under it (the shared calculator
    /// the mid-field cards use) — never a contrasting straw lifted off a green map.
    #[test]
    fn blades_wear_the_same_ground_tone_as_the_cards() {
        let ground = flat_ground();
        let materials =
            crate::terrain_maps::terrain_material_set_for(terrain::MapId::ProkhorovkaHill252_2);
        let maps = full_veg_maps(260.0);
        let eye = Vec3::new(128.0, 3.0, 128.0);
        let grown = grass_frame_objects(&ground, None, &[], &maps, &materials, eye);
        for tuft in grown.iter().step_by(97) {
            let x = tuft.transform[3][0];
            let z = tuft.transform[3][2];
            let reference = crate::grass_cards::card_albedo(&maps, &materials, x, z, 0.5)
                .expect("vegetated ground");
            for lane in 0..3 {
                assert!(
                    (tuft.tint[lane] - reference[lane]).abs() < 0.12,
                    "blade tint stays in the ground's family: {:?} vs {:?}",
                    tuft.tint,
                    reference
                );
            }
        }
    }

    /// Blade tips opted into the wind lane, roots stayed planted, and the lane grows
    /// with height ALONG each blade — the shader bends an arc pinned at the root, not a
    /// rigid stick on a hinge (kernel 2.0).
    #[test]
    fn the_wind_lane_grows_along_each_blade_and_roots_stay_planted() {
        let mesh = grass_tuft_mesh();
        assert_eq!(mesh.vertices().len() % 5, 0, "a blade is root pair + mid pair + tip");
        for blade in mesh.vertices().chunks(5) {
            let (roots, mids, tip) = (&blade[0..2], &blade[2..4], &blade[4]);
            for root in roots {
                assert!(root.position[1] <= 0.001, "roots sit on the ground");
                assert_eq!(root.sway, 0.0, "a root stays planted");
            }
            for mid in mids {
                assert!(
                    mid.sway > 0.0 && mid.sway < tip.sway,
                    "the mid station rides less wind than the tip: {} vs {}",
                    mid.sway,
                    tip.sway
                );
            }
            assert!(tip.sway > 0.3, "a tip rides the wind: sway {}", tip.sway);
        }
    }

    /// Kernel 2.0's silhouette locks (Jedna Trawa P1): every blade ends in a single POINTED
    /// tip, the per-blade height spread is wide enough to serrate the tuft's outline, the
    /// blade profile arcs outward (mid station part-way, tip full reach), and the tallest
    /// blade at the largest conjured scale stays under the honesty cap — grass never hides
    /// a tank (D1).
    #[test]
    fn blades_are_pointed_arced_serrated_and_capped() {
        let mesh = grass_tuft_mesh();
        let mut tallest: f32 = 0.0;
        let mut shortest = f32::INFINITY;
        for blade in mesh.vertices().chunks(5) {
            let tip = &blade[4];
            let apex = blade.iter().map(|v| v.position[1]).fold(0.0f32, f32::max);
            assert!(
                (tip.position[1] - apex).abs() < 1.0e-6,
                "the single tip vertex IS the blade's apex"
            );
            let radial = |v: &SceneVertex| v.position[0].hypot(v.position[2]);
            let root_out = (radial(&blade[0]) + radial(&blade[1])) * 0.5;
            let mid_out = (radial(&blade[2]) + radial(&blade[3])) * 0.5;
            assert!(
                root_out < mid_out && mid_out < radial(tip),
                "a blade arcs outward: root {root_out:.3} < mid {mid_out:.3} < tip {:.3}",
                radial(tip)
            );
            tallest = tallest.max(tip.position[1]);
            shortest = shortest.min(tip.position[1]);
        }
        assert!(
            tallest / shortest > 1.8,
            "the height spread serrates the silhouette: {shortest:.3}..{tallest:.3}"
        );
        assert!(
            (tallest - TUFT_MESH_TALLEST_M).abs() < 1.0e-6,
            "TUFT_MESH_TALLEST_M is the mesh's real apex — the far costume's height \
             continuity hangs on this single number: {tallest}"
        );
        assert!(
            tallest * (TUFT_SCALE_MIN + TUFT_SCALE_SPAN) <= GRASS_HEIGHT_CAP_M + 1.0e-6,
            "the tallest blade at the largest scale respects the honesty cap: {}",
            tallest * (TUFT_SCALE_MIN + TUFT_SCALE_SPAN)
        );
    }

    /// THE anti-streaming contract: a normal cache rebuild may change only the invisible
    /// six-metre population margin. Every tuft that either eye can show remains in both caches
    /// with a bit-identical natural transform; the shader alone changes its distance fade.
    #[test]
    fn rebuild_and_cell_crossing_keep_the_visible_population_stable() {
        let ground = flat_ground();
        let materials =
            crate::terrain_maps::terrain_material_set_for(terrain::MapId::ProkhorovkaHill252_2);
        let maps = full_veg_maps(256.0);
        let eye_a = Vec3::new(127.75, 3.0, 128.0);
        let eye_b = eye_a + Vec3::new(4.25, 0.0, 0.0);
        let a = grass_frame_objects(&ground, None, &[], &maps, &materials, eye_a);
        let b = grass_frame_objects(&ground, None, &[], &maps, &materials, eye_b);
        let a_by_key: std::collections::HashMap<_, _> =
            a.iter().map(|tuft| (tuft_key(tuft), tuft)).collect();
        let b_by_key: std::collections::HashMap<_, _> =
            b.iter().map(|tuft| (tuft_key(tuft), tuft)).collect();

        let mut shared = 0usize;
        for (key, tuft_a) in &a_by_key {
            if let Some(tuft_b) = b_by_key.get(key) {
                shared += 1;
                assert_eq!(
                    *tuft_a, *tuft_b,
                    "shared world tuft keeps its natural transform and tone across a rebuild"
                );
            } else {
                assert!(
                    tuft_flat_distance(tuft_a, eye_a) > GRASS_RADIUS_M
                        && tuft_flat_distance(tuft_a, eye_b) > GRASS_RADIUS_M,
                    "a removed cache-margin tuft must be shader-invisible to both eyes"
                );
            }
        }
        for (key, tuft_b) in &b_by_key {
            if !a_by_key.contains_key(key) {
                assert!(
                    tuft_flat_distance(tuft_b, eye_a) > GRASS_RADIUS_M
                        && tuft_flat_distance(tuft_b, eye_b) > GRASS_RADIUS_M,
                    "a newly streamed cache-margin tuft must be shader-invisible to both eyes"
                );
            }
        }
        assert!(shared > 3_000, "the caches overlap massively across one rebuild: {shared}");
    }

    /// The fixed 28-candidate population is the budget. Sweep every half-metre sub-cell phase
    /// on the lushest possible ground: no phase may need runtime truncation, and every cache
    /// retains candidates throughout the invisible margin outside the 48 m shader ring.
    #[test]
    fn full_vegetation_cache_sweep_fits_without_hard_truncation() {
        let ground = flat_ground();
        let materials =
            crate::terrain_maps::terrain_material_set_for(terrain::MapId::ProkhorovkaHill252_2);
        let maps = full_veg_maps(256.0);
        let mut peak = 0usize;
        let mut floor = usize::MAX;
        for z_phase in 0..16 {
            for x_phase in 0..16 {
                let eye = Vec3::new(96.0 + x_phase as f32 * 0.5, 3.0, 96.0 + z_phase as f32 * 0.5);
                let grown = grass_frame_objects(&ground, None, &[], &maps, &materials, eye);
                peak = peak.max(grown.len());
                floor = floor.min(grown.len());
                assert!(
                    grown.len() < MAX_GRASS_INSTANCES,
                    "fixed population must fit without touching the runtime guard: {}",
                    grown.len()
                );
                assert!(
                    grown.iter().any(|tuft| {
                        let d = tuft_flat_distance(tuft, eye);
                        d > GRASS_RADIUS_M + 4.0 && d <= GRASS_CACHE_RADIUS_M
                    }),
                    "the cache must populate its invisible outer margin at phase {x_phase}/{z_phase}"
                );
            }
        }
        // Renegotiated for D7 (teren A4): visible bald patches ARE the feature, and a floor
        // of 3 800 was arithmetically incompatible with ~10 % baldness on a 28-candidate
        // budget. The field must still read dense — just not carpet-uniform.
        assert!(floor > 3_300, "lush cache stays visually dense at every phase: {floor}");
        assert!(peak < MAX_GRASS_INSTANCES, "lush cache keeps explicit headroom: {peak}");
    }

    /// The fold (Jedna Trawa P3): the ring is now mirror-fair like the far meadow always
    /// was — a tuft south of the axis has its exact twin north of it, because BOTH grow
    /// from the one south-half candidate stream.
    #[test]
    fn the_ring_is_mirror_fair_across_the_axis() {
        let ground = flat_ground();
        let extent_z = ground.extent_m()[1];
        let materials = TerrainMaterialSet::default();
        let maps = full_veg_maps(extent_z);
        // The eye sits ON the axis, so both twins of every nearby candidate are in range.
        let eye = Vec3::new(128.0, 3.0, extent_z * 0.5);
        let grown = grass_frame_objects(&ground, None, &[], &maps, &materials, eye);
        let mut probed = 0;
        for tuft in grown.iter().step_by(37) {
            let (x, z) = (tuft.transform[3][0], tuft.transform[3][2]);
            if (z - extent_z * 0.5).abs() < 0.5 || tuft_flat_distance(tuft, eye) > 40.0 {
                continue; // the twin of a far-edge tuft may fall outside the cache
            }
            let twin = grown.iter().any(|other| {
                (other.transform[3][0] - x).abs() < 1.0e-3
                    && (other.transform[3][2] - (extent_z - z)).abs() < 1.0e-3
            });
            assert!(twin, "tuft at ({x}, {z}) has no mirror twin");
            probed += 1;
        }
        assert!(probed > 30, "the probe saw a real sample: {probed}");
    }

    #[test]
    fn the_ring_is_deterministic_and_rides_the_eye() {
        let ground = flat_ground();
        let materials =
            crate::terrain_maps::terrain_material_set_for(terrain::MapId::ProkhorovkaHill252_2);
        let maps = full_veg_maps(256.0);
        let eye = Vec3::new(100.0, 3.0, 100.0);
        let a = grass_frame_objects(&ground, None, &[], &maps, &materials, eye);
        let b = grass_frame_objects(&ground, None, &[], &maps, &materials, eye);
        assert_eq!(a.len(), b.len(), "the same eye grows the same field");
        assert_eq!(a[0].transform, b[0].transform);

        // A shared cell keeps its tufts when the eye moves one cell over (world-anchored).
        let moved = grass_frame_objects(
            &ground,
            None,
            &[],
            &maps,
            &materials,
            eye + Vec3::new(CELL_M, 0.0, 0.0),
        );
        let anchor = a.iter().map(|o| o.transform[3][0]).fold(0.0f32, f32::max);
        assert!(
            moved.iter().any(|o| (o.transform[3][0] - anchor).abs() < CELL_M * 4.0),
            "tufts are world-anchored, not eye-glued"
        );
    }

    #[test]
    fn the_tuft_mesh_is_cheap_double_sided_and_fully_tintable() {
        let mesh = grass_tuft_mesh();
        // Kernel 2.0 budget: 10 blades × 18 indices (2-segment blade, both faces). Raised
        // from 150 with the P0 measurement in hand (ring 0.83 ms GPU at 144 indices/tuft);
        // the delta is re-measured in the program's STATUS ledger.
        assert!(
            mesh.indices().len() <= 180,
            "a tuft stays cheap: {} indices",
            mesh.indices().len()
        );
        assert_eq!(mesh.indices().len() % 3, 0, "whole triangles only");
        for vertex in mesh.vertices() {
            assert_eq!(vertex.tint_weight, 1.0, "the instance tint IS the grass colour");
            assert_eq!(
                vertex.surface,
                renderer_api::surface_role::GRASS_BLADE,
                "the shader must recognize every near blade for its camera-distance fade"
            );
            assert!(vertex.position[1] >= 0.0 && vertex.position[1] <= 0.35);
        }
    }
}
