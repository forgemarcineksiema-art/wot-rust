//! Near-field grass (Materia Świata 1b): the one thing that tells the eye how big the world
//! is. A deterministic scatter of blade tufts rides the camera in an ~26 m ring through the
//! scene pipeline's per-frame instanced path (`frame_draws`) — the far field pays nothing,
//! nothing is stored, and every client conjures the same tufts from the same cell hashes.
//! Density and colour come from the baked splat (grass stands where the vegetation layers
//! do — dirt roads, rock and the riverbed stay bare), and the ring's edge shrinks tufts to
//! zero so the boundary never pops.

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

/// The field the grass lives in, and where its outer edge fade begins. Inside
/// [`NEAR_FULL_M`] the cover is full; past it the density falls with the square of the
/// distance — constant SCREEN density, so the field reads unbroken to the horizon of the
/// ring while the cost grows only logarithmically with the radius.
pub const GRASS_RADIUS_M: f32 = 60.0;
const FADE_START_M: f32 = 44.0;
/// Full-cover radius: every tuft the splat allows stands inside this.
const NEAR_FULL_M: f32 = 22.0;
/// Each tuft grows from nothing over this much approach past its own reveal distance, so a
/// tuft is born far away and sub-pixel — never as a pop in the midfield.
const REVEAL_BAND_M: f32 = 7.0;
/// Scatter cell edge; tufts are conjured per cell from the cell's own hash.
const CELL_M: f32 = 8.0;
/// Tufts a fully-vegetated cell grows; the splat's vegetation weight scales it down.
const CELL_TUFT_MAX: u32 = 110;
/// The field's worst case (full-cover disc + the decaying band) inside the 6.5k scene
/// instance buffer, with vehicle dressing headroom to spare.
pub const MAX_GRASS_INSTANCES: usize = 4_800;
/// Ground with less vegetation weight than this grows nothing (roads, rock, riverbed).
const MIN_VEG_WEIGHT: f32 = 0.35;
/// Standing water drowns the tufts.
const MAX_WATER_DEPTH_M: f32 = 0.05;

/// One tuft: five blades leaning outward from a common root, each a bent two-triangle card,
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
                surface: 0.0,
                sway,
            });
        }
        // Front and back faces: the pipeline culls, the blade must not vanish from behind.
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        indices.extend_from_slice(&[base + 2, base + 1, base, base + 3, base + 2, base]);
    }
    MeshAsset::new(vertices, indices)
}

/// Density multiplier of the grass field at planar distance `d` from the eye: a gentle boost
/// right at the feet, full cover to [`NEAR_FULL_M`], then the inverse-square falloff that
/// keeps the density constant on SCREEN out to the field's edge.
fn density_multiplier(d: f32) -> f32 {
    if d <= NEAR_FULL_M {
        let near_t = ((d - 6.0) / (NEAR_FULL_M - 6.0)).clamp(0.0, 1.0);
        2.2 - 1.2 * near_t * near_t * (3.0 - 2.0 * near_t)
    } else {
        (NEAR_FULL_M / d).powi(2)
    }
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

/// Conjure this frame's grass ring around the eye. Deterministic per cell — no storage, no
/// churn: the same eye position always grows the same field.
pub fn grass_frame_objects(
    heightmap: &HeightMap,
    water: Option<WaterBody>,
    maps: &TerrainGroundMaps,
    materials: &TerrainMaterialSet,
    eye: Vec3,
) -> Vec<RenderObject> {
    let mut objects = Vec::new();
    let min_cx = ((eye.x - GRASS_RADIUS_M) / CELL_M).floor() as i32;
    let max_cx = ((eye.x + GRASS_RADIUS_M) / CELL_M).floor() as i32;
    let min_cz = ((eye.z - GRASS_RADIUS_M) / CELL_M).floor() as i32;
    let max_cz = ((eye.z + GRASS_RADIUS_M) / CELL_M).floor() as i32;
    let grass_albedo = Vec3::from_array(materials.layers[0].albedo);
    let straw_albedo = Vec3::from_array(materials.layers[1].albedo);
    // Nearest cells first: when the ring's worst case outruns the budget, the rim thins out
    // under its own fade — never the ground at the player's feet.
    let mut cells: Vec<(i32, i32, f32)> = Vec::new();
    for cz in min_cz..=max_cz {
        for cx in min_cx..=max_cx {
            let center_x = (cx as f32 + 0.5) * CELL_M;
            let center_z = (cz as f32 + 0.5) * CELL_M;
            let dist = Vec3::new(center_x - eye.x, 0.0, center_z - eye.z).length();
            cells.push((cx, cz, dist));
        }
    }
    cells.sort_by(|a, b| a.2.total_cmp(&b.2));
    // A high-explosive burst burns and buries the grass it lands on: the replicated crater
    // ledger (already folded into the heightmap) is a kill list — nothing grows inside a
    // bowl or on its fresh spoil rim.
    let craters: Vec<(f32, f32, f32)> = heightmap
        .crater_records()
        .iter()
        .map(|crater| (crater.x_m(), crater.z_m(), crater.radius_m() * 1.45))
        .collect();
    'cells: for (cx, cz, cell_dist) in cells {
        {
            let mut seed = (cx as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
                ^ (cz as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
                ^ 0x6265_7472_6177_6121;
            // The cell's own dryness lane: tufts lean grass or straw per plot, echoing the
            // shader's field quilt without resampling it.
            let cell_dry = game_core::math::next_hash_unit(&mut seed);
            let origin = Vec3::new(cx as f32 * CELL_M, 0.0, cz as f32 * CELL_M);
            let veg = vegetation_weight(maps, origin.x + CELL_M * 0.5, origin.z + CELL_M * 0.5);
            if veg < MIN_VEG_WEIGHT {
                continue;
            }
            // Spend the budget where the eye is: full cover to NEAR_FULL_M (with a boost at
            // the feet), then a 1/d^2 falloff — constant density on SCREEN, so the far field
            // reads as unbroken grass while costing a fraction of the near field. Each tuft's
            // index in the cell's deterministic sequence IS its reveal rank: the target count
            // rises as the eye approaches, always admitting the same tufts in the same order,
            // and each newborn scales in over its own reveal band far from the eye.
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
            let cell_near = (cell_dist - CELL_M).max(1.0);
            let cell_ceiling = veg * CELL_TUFT_MAX as f32 * density_multiplier(cell_near) + 3.0;
            let count = (cell_ceiling as u32).min((CELL_TUFT_MAX as f32 * 2.2) as u32);
            for index in 0..count {
                let x = origin.x + game_core::math::next_hash_unit(&mut seed) * CELL_M;
                let z = origin.z + game_core::math::next_hash_unit(&mut seed) * CELL_M;
                let yaw = game_core::math::next_hash_unit(&mut seed) * std::f32::consts::TAU;
                let size = 1.0 + game_core::math::next_hash_unit(&mut seed) * 0.6;
                let tone = game_core::math::next_hash_unit(&mut seed);
                let flat = Vec3::new(x - eye.x, 0.0, z - eye.z).length();
                if flat > GRASS_RADIUS_M {
                    continue;
                }
                // This tuft's personal reveal: the local target count must have passed its
                // index, and it grows in smoothly as the margin opens — born sub-pixel.
                let target = veg * CELL_TUFT_MAX as f32 * density_multiplier(flat);
                let reveal = ((target - index as f32)
                    * (REVEAL_BAND_M / NEAR_FULL_M.max(1.0)).max(0.35))
                .clamp(0.0, 1.0);
                if reveal <= 0.0 {
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
                // The field's outer edge shrinks the tuft to nothing — no boundary pop.
                let fade =
                    1.0 - ((flat - FADE_START_M) / (GRASS_RADIUS_M - FADE_START_M)).clamp(0.0, 1.0);
                let scale = size * fade * reveal;
                if scale < 0.05 {
                    continue;
                }
                // A touch lighter than the soil it stands on: blades catch more sky.
                let albedo = grass_albedo.lerp(straw_albedo, (cell_dry * 0.8 + tone * 0.2) * 0.7)
                    * (1.0 + tone * 0.3);
                let transform = Mat4::from_translation(Vec3::new(x, ground, z))
                    * Mat4::from_rotation_y(yaw)
                    * Mat4::from_scale(Vec3::new(scale, scale, scale));
                objects.push(RenderObject {
                    tank_id: None,
                    mesh: GRASS_MESH_HANDLE,
                    material: MaterialHandle(0),
                    transform: transform.to_cols_array_2d(),
                    tint: albedo.to_array(),
                });
                if objects.len() >= MAX_GRASS_INSTANCES {
                    break 'cells;
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

    fn flat_ground() -> HeightMap {
        HeightMap::flat(65, 65, 4.0, 1.0).expect("flat map")
    }

    #[test]
    fn grass_grows_on_vegetation_and_refuses_roads_water_and_the_far_field() {
        let ground = flat_ground();
        let materials = TerrainMaterialSet::bystra();
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
            assert!(flat <= GRASS_RADIUS_M + 1.0e-3, "no tuft outside the ring, got {flat}");
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

    /// A shell hole is bare: no tuft stands inside a replicated crater's bowl or on its
    /// fresh spoil — the burst burned and buried them (Fizyczny Świat tie-in).
    #[test]
    fn grass_refuses_to_grow_in_a_fresh_crater() {
        let mut ground = flat_ground();
        let materials = TerrainMaterialSet::prokhorovka();
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

    #[test]
    fn the_ring_is_deterministic_and_rides_the_eye() {
        let ground = flat_ground();
        let materials = TerrainMaterialSet::prokhorovka();
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
            assert!(vertex.position[1] >= 0.0 && vertex.position[1] <= 0.4);
        }
    }
}
