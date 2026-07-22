//! The mid-field meadow (Żywy Step P2): grass-clump CARDS baked statically for the whole map
//! and drawn through the renderer's dressing slot (color pass only, chunk-culled, distance
//! cut). Where the near ring gives the eye individual blades, the cards give it an UNBROKEN
//! field out to ~330 m — each card two crossed trapezoids in the exact albedo of the ground
//! it stands on, so the band boundaries dissolve into tone instead of reading as edges.
//! Deterministic (map + cell hash) and mirror-fair by construction: the scatter is generated
//! for the south half and emitted with its exact mirrored twin.

use glam::Vec3;
use renderer_api::{SceneVertex, TerrainGroundMaps, TerrainMaterialSet, surface_role};
use terrain::BattlefieldMap;

use crate::grass::vegetation_weight;

/// Scatter cell edge (matches the near ring's rhythm).
const CELL_M: f32 = 8.0;
/// Cards a fully-vegetated cell grows: 64 m² / ~14 m² per card.
const CELL_CARDS: f32 = 4.5;
/// Ground with less vegetation than this grows nothing (roads, rock, riverbed).
const MIN_VEG_WEIGHT: f32 = 0.35;
/// Standing water drowns the cards.
const MAX_WATER_DEPTH_M: f32 = 0.05;
/// A crater's kill zone (matches the near ring): nothing grows in the bowl or on the spoil.
const CRATER_KILL_FACTOR: f32 = 1.45;
/// The sway lane doubles as height-over-root for the vertex-stage collapse: sway = h * this.
const SWAY_PER_HEIGHT: f32 = 0.3;

/// Bake the whole map's card meadow. Pure function of (map, splat, materials) — every client
/// bakes the identical field; a crater re-bake goes through here too.
pub fn grass_card_dressing_mesh(
    battlefield: &BattlefieldMap,
    maps: &TerrainGroundMaps,
    materials: &TerrainMaterialSet,
) -> (Vec<SceneVertex>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let heightmap = &battlefield.heightmap;
    let [extent_x, extent_z] = heightmap.extent_m();
    let mirror_z = extent_z * 0.5;
    let craters: Vec<(f32, f32, f32)> = heightmap
        .crater_records()
        .iter()
        .map(|crater| (crater.x_m(), crater.z_m(), crater.radius_m() * CRATER_KILL_FACTOR))
        .collect();

    let cols = (extent_x / CELL_M).floor() as i32;
    let south_rows = (mirror_z / CELL_M).ceil() as i32;
    for cz in 0..south_rows {
        for cx in 0..cols {
            let mut seed = (cx as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
                ^ (cz as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
                ^ 0x6361_7264_7321_0001;
            let origin_x = cx as f32 * CELL_M;
            let origin_z = cz as f32 * CELL_M;
            let veg = vegetation_weight(maps, origin_x + CELL_M * 0.5, origin_z + CELL_M * 0.5);
            if veg < MIN_VEG_WEIGHT {
                continue;
            }
            let count = (veg * CELL_CARDS + game_core::math::next_hash_unit(&mut seed)) as u32;
            for _ in 0..count {
                let x = origin_x + game_core::math::next_hash_unit(&mut seed) * CELL_M;
                let z = origin_z + game_core::math::next_hash_unit(&mut seed) * CELL_M;
                let yaw = game_core::math::next_hash_unit(&mut seed) * std::f32::consts::TAU;
                let height = 0.55 + game_core::math::next_hash_unit(&mut seed) * 0.3;
                let tone = game_core::math::next_hash_unit(&mut seed);
                // The mid row seeds only its own half, so the fold never doubles a card.
                if z >= mirror_z {
                    continue;
                }
                let Some(albedo) = card_albedo(maps, materials, x, z, tone) else {
                    continue;
                };
                for (px, pz, pyaw) in [(x, z, yaw), (x, extent_z - z, std::f32::consts::TAU - yaw)]
                {
                    let Some(ground) = heightmap.sample_height(px, pz) else {
                        continue;
                    };
                    if battlefield.water.is_some_and(|w| w.depth_over(ground) > MAX_WATER_DEPTH_M) {
                        continue;
                    }
                    if craters.iter().any(|&(kx, kz, kill)| (px - kx).hypot(pz - kz) < kill) {
                        continue;
                    }
                    push_card(
                        &mut vertices,
                        &mut indices,
                        Vec3::new(px, ground, pz),
                        pyaw,
                        height,
                        albedo,
                    );
                }
            }
        }
    }
    (vertices, indices)
}

/// The exact tone of the ground under a card: splat-weighted layer albedo with a small sky
/// lift — blades catch more light than the soil they stand on, but stay the SAME color
/// family, so the card field dissolves into the ground instead of contrasting with it.
pub(crate) fn card_albedo(
    maps: &TerrainGroundMaps,
    materials: &TerrainMaterialSet,
    x: f32,
    z: f32,
    tone: f32,
) -> Option<[f32; 3]> {
    let size = maps.size as usize;
    let tx = ((x / maps.extent_m[0]) * maps.size as f32).clamp(0.0, maps.size as f32 - 1.0);
    let tz = ((z / maps.extent_m[1]) * maps.size as f32).clamp(0.0, maps.size as f32 - 1.0);
    let index = (tz as usize * size + tx as usize) * 4;
    let weights = &maps.splat[index..index + 4];
    let total: u32 = weights.iter().map(|&w| u32::from(w)).sum();
    if total == 0 {
        return None;
    }
    let mut albedo = Vec3::ZERO;
    for (layer, &weight) in materials.layers.iter().zip(weights) {
        albedo += Vec3::from_array(layer.albedo) * (f32::from(weight) / total as f32);
    }
    Some((albedo * (1.02 + tone * 0.08)).to_array())
}

/// One clump card: two crossed trapezoids (narrowed tops), both faces wound — 8 vertices,
/// 8 triangles. The sway lane carries height-over-root for the collapse AND the wind.
fn push_card(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    root: Vec3,
    yaw: f32,
    height: f32,
    albedo: [f32; 3],
) {
    let base_tone = [albedo[0] * 0.72, albedo[1] * 0.72, albedo[2] * 0.72];
    let tip_tone = [albedo[0] * 1.12, albedo[1] * 1.12, albedo[2] * 1.12];
    let (sin, cos) = yaw.sin_cos();
    for second in [false, true] {
        let (dir_x, dir_z) = if second { (-sin, cos) } else { (cos, sin) };
        let half_bottom = 0.35;
        let half_top = 0.17;
        let base = vertices.len() as u32;
        for (offset, y, tone, sway) in [
            (-half_bottom, 0.0, base_tone, 0.0),
            (half_bottom, 0.0, base_tone, 0.0),
            (half_top, height, tip_tone, height * SWAY_PER_HEIGHT),
            (-half_top, height, tip_tone, height * SWAY_PER_HEIGHT),
        ] {
            vertices.push(SceneVertex {
                position: [root.x + dir_x * offset, root.y + y, root.z + dir_z * offset],
                normal: [0.0, 1.0, 0.0],
                color: tone,
                tint_weight: 0.0,
                gloss: 0.05,
                surface: surface_role::GRASS_CARD,
                sway,
                uv: [0.0, 0.0],
            });
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        indices.extend_from_slice(&[base + 2, base + 1, base, base + 3, base + 2, base]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain_maps::{bake_terrain_ground_maps, terrain_material_set_for};

    fn baked() -> (Vec<SceneVertex>, Vec<u32>) {
        let map = map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2);
        let maps = bake_terrain_ground_maps(&map);
        let materials = terrain_material_set_for(terrain::MapId::ProkhorovkaHill252_2);
        grass_card_dressing_mesh(&map, &maps, &materials)
    }

    #[test]
    fn the_card_meadow_is_deterministic_mirror_fair_and_built_of_cheap_cards() {
        let (vertices, indices) = baked();
        let (twin_v, twin_i) = baked();
        assert!(vertices == twin_v && indices == twin_i, "every client bakes the same field");
        // 8 vertices / 8 triangles per card, tens of thousands of cards on the vegetated map.
        assert_eq!(vertices.len() % 8, 0);
        assert_eq!(indices.len(), vertices.len() * 3);
        let cards = vertices.len() / 8;
        assert!(
            (20_000..90_000).contains(&cards),
            "the whole-map meadow is tens of thousands of cards: {cards}"
        );
        // Mirror-fair: a card's root at z has a twin at extent-z with the same x.
        let map = map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2);
        let extent_z = map.heightmap.extent_m()[1];
        for probe in (0..cards).step_by(cards / 37) {
            let root = vertices[probe * 8].position;
            let twin_exists = vertices.chunks(8).any(|card| {
                let p = card[0].position;
                (p[0] - root[0]).abs() < 0.01 && (p[2] - (extent_z - root[2])).abs() < 0.01
            });
            assert!(twin_exists, "card at ({}, {}) has no mirror twin", root[0], root[2]);
        }
    }

    #[test]
    fn cards_wear_the_ground_tone_and_the_sway_lane_encodes_height() {
        let (vertices, _) = baked();
        for card in vertices.chunks(8).step_by(211) {
            for vertex in card {
                assert!(
                    (vertex.surface - surface_role::GRASS_CARD).abs() < 1.0e-3,
                    "cards dispatch as GRASS_CARD"
                );
                let (r, g, b) = (vertex.color[0], vertex.color[1], vertex.color[2]);
                let max = r.max(g).max(b);
                let saturation = if max <= 0.0 { 0.0 } else { (max - r.min(g).min(b)) / max };
                assert!(saturation <= 0.45, "rule 2: ground stays muted, got {saturation}");
                if vertex.position[1] - card[0].position[1] > 0.1 {
                    assert!(vertex.sway > 0.1, "tips carry their height in the sway lane");
                } else {
                    assert_eq!(vertex.sway, 0.0, "roots stay planted");
                }
            }
        }
    }

    #[test]
    fn a_fresh_crater_mows_its_patch_out_of_the_card_meadow() {
        let mut map = map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2);
        let maps = bake_terrain_ground_maps(&map);
        let materials = terrain_material_set_for(terrain::MapId::ProkhorovkaHill252_2);
        let crater = terrain::CraterRecord::from_world(
            500.0,
            480.0,
            3.0,
            1.0,
            terrain::CRATER_KIND_HIGH_EXPLOSIVE,
        );
        map.heightmap.set_craters(&[crater]);
        let (vertices, _) = grass_card_dressing_mesh(&map, &maps, &materials);
        let kill = crater.radius_m() * CRATER_KILL_FACTOR;
        for card in vertices.chunks(8) {
            // The ROOT is the midpoint of the base edge (vertex 0 and 1 are offset by the
            // card's half-width); the kill zone is measured from where the clump grows.
            let root_x = (card[0].position[0] + card[1].position[0]) * 0.5;
            let root_z = (card[0].position[2] + card[1].position[2]) * 0.5;
            assert!(
                (root_x - crater.x_m()).hypot(root_z - crater.z_m()) >= kill - 1.0e-3,
                "no card grows in the burst: ({root_x}, {root_z})"
            );
        }
    }
}
