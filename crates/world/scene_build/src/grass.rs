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
/// Scatter cell edge; tufts are conjured per cell from the cell's own hash.
const CELL_M: f32 = 8.0;
/// Fixed world population ceiling per 8 m cell. Vegetation acceptance can only remove from
/// this deterministic candidate sequence; camera movement never changes a cell's population.
const CELL_TUFT_CANDIDATES: u32 = 28;
/// Full-vegetation population budget for the 54 m cache disc. This is a verification guard,
/// not a runtime crop: hard truncation would reintroduce a moving wall at the cache boundary.
pub const MAX_GRASS_INSTANCES: usize = 4_800;
/// Ground with less vegetation weight than this grows nothing (roads, rock, riverbed).
const MIN_VEG_WEIGHT: f32 = 0.35;
/// Standing water drowns the tufts.
const MAX_WATER_DEPTH_M: f32 = 0.05;

/// One tuft: twelve blades leaning outward from a common root, each a bent two-triangle card,
/// both faces wound (the scene pipeline culls back faces; a blade must read from anywhere).
/// Vertices carry a white-to-dusk gradient with `tint_weight` 1.0 — the INSTANCE tint is the
/// actual grass colour, so one mesh serves every plot tone on the map.
pub fn grass_tuft_mesh() -> MeshAsset {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let blades = 12;
    for blade in 0..blades {
        let angle =
            blade as f32 / blades as f32 * std::f32::consts::TAU + ((blade % 3) as f32) * 0.35;
        let (sin, cos) = angle.sin_cos();
        let lean = Vec3::new(cos, 0.0, sin) * 0.09;
        let root = Vec3::new(cos, 0.0, sin) * (0.06 + 0.06 * ((blade % 4) as f32 / 3.0));
        let across = Vec3::new(-sin, 0.0, cos) * 0.02;
        let height = 0.16 + 0.12 * ((blade * 7 + 3) % 5) as f32 / 4.0;
        let tip = root + lean + Vec3::Y * height;
        // Base sits in shade, the tip catches the sky — a cheap self-shadow gradient. The
        // normals stand mostly UP so a blade takes the same sun the ground under it does —
        // side-facing normals read as a dark alien succulent, not grass.
        let base_tone = [0.72, 0.72, 0.72];
        let tip_tone = [1.05, 1.05, 1.05];
        let normal = Vec3::new(cos * 0.3, 1.0, sin * 0.3).normalize().to_array();
        let base = vertices.len() as u32;
        // Roots stay planted (sway 0); the tips ride the field's wind, taller blades harder.
        let tip_sway = 0.35 + height * 0.8;
        for (position, tone, sway) in [
            (root - across, base_tone, 0.0),
            (root + across, base_tone, 0.0),
            (tip + across * 0.3, tip_tone, tip_sway),
            (tip - across * 0.3, tip_tone, tip_sway),
        ] {
            vertices.push(SceneVertex {
                position: position.to_array(),
                normal,
                color: tone,
                tint_weight: 1.0,
                gloss: 0.05,
                surface: renderer_api::surface_role::GRASS_BLADE,
                sway,
                uv: [0.0, 0.0],
            });
        }
        // Front and back faces: the pipeline culls, the blade must not vanish from behind.
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        indices.extend_from_slice(&[base + 2, base + 1, base, base + 3, base + 2, base]);
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

/// Build the stable grass population cached around `eye`. Every cell owns exactly one candidate
/// sequence; local splat acceptance, terrain, water and craters may remove candidates, but the
/// eye never changes their rank, size or transform. The shader performs the visible 34–48 m
/// fade every frame, while this CPU population extends to [`GRASS_CACHE_RADIUS_M`].
pub fn grass_frame_objects(
    heightmap: &HeightMap,
    water: Option<WaterBody>,
    maps: &TerrainGroundMaps,
    materials: &TerrainMaterialSet,
    eye: Vec3,
) -> Vec<RenderObject> {
    let mut objects = Vec::with_capacity(MAX_GRASS_INSTANCES);
    let min_cx = ((eye.x - GRASS_CACHE_RADIUS_M) / CELL_M).floor() as i32;
    let max_cx = ((eye.x + GRASS_CACHE_RADIUS_M) / CELL_M).floor() as i32;
    let min_cz = ((eye.z - GRASS_CACHE_RADIUS_M) / CELL_M).floor() as i32;
    let max_cz = ((eye.z + GRASS_CACHE_RADIUS_M) / CELL_M).floor() as i32;
    // A high-explosive burst burns and buries the grass it lands on: the replicated crater
    // ledger (already folded into the heightmap) is a kill list — nothing grows inside a
    // bowl or on its fresh spoil rim.
    let craters: Vec<(f32, f32, f32)> = heightmap
        .crater_records()
        .iter()
        .map(|crater| (crater.x_m(), crater.z_m(), crater.radius_m() * 1.45))
        .collect();
    for cz in min_cz..=max_cz {
        for cx in min_cx..=max_cx {
            let mut seed = (cx as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
                ^ (cz as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
                ^ 0x6265_7472_6177_6121;
            // The cell's own dryness lane: tufts lean grass or straw per plot, echoing the
            // shader's field quilt without resampling it.
            let cell_dry = game_core::math::next_hash_unit(&mut seed);
            let origin = Vec3::new(cx as f32 * CELL_M, 0.0, cz as f32 * CELL_M);
            // Craters whose kill zone can touch this cell — usually none, so the per-tuft
            // test costs nothing on virgin ground.
            let center_x = (cx as f32 + 0.5) * CELL_M;
            let center_z = (cz as f32 + 0.5) * CELL_M;
            let cell_reach = CELL_M * 0.75;
            let nearby_craters: Vec<(f32, f32, f32)> = craters
                .iter()
                .copied()
                .filter(|&(x, z, kill)| {
                    let dx = (x - center_x).abs() - cell_reach;
                    let dz = (z - center_z).abs() - cell_reach;
                    dx.max(0.0).hypot(dz.max(0.0)) <= kill
                })
                .collect();
            for _ in 0..CELL_TUFT_CANDIDATES {
                let x = origin.x + game_core::math::next_hash_unit(&mut seed) * CELL_M;
                let z = origin.z + game_core::math::next_hash_unit(&mut seed) * CELL_M;
                let yaw = game_core::math::next_hash_unit(&mut seed) * std::f32::consts::TAU;
                let size = 1.0 + game_core::math::next_hash_unit(&mut seed) * 0.6;
                let tone = game_core::math::next_hash_unit(&mut seed);
                let vegetation_lane = game_core::math::next_hash_unit(&mut seed);
                let flat = Vec3::new(x - eye.x, 0.0, z - eye.z).length();
                if flat > GRASS_CACHE_RADIUS_M {
                    continue;
                }
                // Vegetation is sampled at the candidate, not at the 8 m cell centre. The
                // extra hash lane makes partial splat weights a stable acceptance probability:
                // a road can cut through a cell without inheriting grass from either side.
                let veg = vegetation_weight(maps, x, z);
                if veg < MIN_VEG_WEIGHT || vegetation_lane >= veg {
                    continue;
                }
                if nearby_craters.iter().any(|&(kx, kz, kill)| (x - kx).hypot(z - kz) < kill) {
                    continue; // burned and buried where the shell landed
                }
                let Some(ground) = heightmap.sample_height(x, z) else {
                    continue;
                };
                if water.is_some_and(|w| w.depth_over(ground) > MAX_WATER_DEPTH_M) {
                    continue;
                }
                // A touch lighter than the soil it stands on: blades catch more sky.
                let Some(albedo) = crate::grass_cards::card_albedo(
                    maps,
                    materials,
                    x,
                    z,
                    cell_dry * 0.5 + tone * 0.5,
                ) else {
                    continue;
                };
                let albedo = Vec3::from_array(albedo);
                let transform = Mat4::from_translation(Vec3::new(x, ground, z))
                    * Mat4::from_rotation_y(yaw)
                    * Mat4::from_scale(Vec3::splat(size));
                objects.push(RenderObject {
                    tank_id: None,
                    mesh: GRASS_MESH_HANDLE,
                    material: MaterialHandle(0),
                    transform: transform.to_cols_array_2d(),
                    tint: albedo.to_array(),
                });
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

    #[test]
    fn grass_grows_on_vegetation_and_refuses_roads_water_and_the_far_field() {
        let ground = flat_ground();
        let materials = crate::terrain_maps::terrain_material_set_for(terrain::MapId::BystraValley);
        let eye = Vec3::new(128.0, 3.0, 128.0);

        let grown = grass_frame_objects(&ground, None, &full_veg_maps(256.0), &materials, eye);
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

        let bare = grass_frame_objects(&ground, None, &bare_dirt_maps(256.0), &materials, eye);
        assert!(bare.is_empty(), "a dirt road grows nothing, got {}", bare.len());

        let flood = Some(WaterBody { surface_level_m: 2.0 });
        let drowned = grass_frame_objects(&ground, flood, &full_veg_maps(256.0), &materials, eye);
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
        let grown = grass_frame_objects(&ground, None, &maps, &materials, eye);

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
        let after = grass_frame_objects(&ground, None, &maps, &materials, eye);

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
        let grown = grass_frame_objects(&ground, None, &maps, &materials, eye);
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

    /// Blade tips opted into the wind lane, roots stayed planted — the shader sways only
    /// what the mesh offered.
    #[test]
    fn blade_tips_ride_the_wind_and_roots_stay_planted() {
        let mesh = grass_tuft_mesh();
        let (mut tips, mut roots) = (0, 0);
        for vertex in mesh.vertices() {
            if vertex.position[1] > 0.05 {
                assert!(vertex.sway > 0.3, "a tip rides the wind: sway {}", vertex.sway);
                tips += 1;
            } else {
                assert_eq!(vertex.sway, 0.0, "a root stays planted");
                roots += 1;
            }
        }
        assert!(tips > 0 && roots > 0);
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
        let a = grass_frame_objects(&ground, None, &maps, &materials, eye_a);
        let b = grass_frame_objects(&ground, None, &maps, &materials, eye_b);
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
                let grown = grass_frame_objects(&ground, None, &maps, &materials, eye);
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
        assert!(floor > 3_800, "lush cache stays visually dense at every phase: {floor}");
        assert!(peak < MAX_GRASS_INSTANCES, "lush cache keeps explicit headroom: {peak}");
    }

    #[test]
    fn the_ring_is_deterministic_and_rides_the_eye() {
        let ground = flat_ground();
        let materials =
            crate::terrain_maps::terrain_material_set_for(terrain::MapId::ProkhorovkaHill252_2);
        let maps = full_veg_maps(256.0);
        let eye = Vec3::new(100.0, 3.0, 100.0);
        let a = grass_frame_objects(&ground, None, &maps, &materials, eye);
        let b = grass_frame_objects(&ground, None, &maps, &materials, eye);
        assert_eq!(a.len(), b.len(), "the same eye grows the same field");
        assert_eq!(a[0].transform, b[0].transform);

        // A shared cell keeps its tufts when the eye moves one cell over (world-anchored).
        let moved = grass_frame_objects(
            &ground,
            None,
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
        assert!(
            mesh.indices().len() <= 150,
            "a tuft stays cheap: {} indices",
            mesh.indices().len()
        );
        assert_eq!(mesh.indices().len() % 6, 0, "blades are two-triangle cards, both faces");
        for vertex in mesh.vertices() {
            assert_eq!(vertex.tint_weight, 1.0, "the instance tint IS the grass colour");
            assert_eq!(
                vertex.surface,
                renderer_api::surface_role::GRASS_BLADE,
                "the shader must recognize every near blade for its camera-distance fade"
            );
            assert!(vertex.position[1] >= 0.0 && vertex.position[1] <= 0.4);
        }
    }
}
