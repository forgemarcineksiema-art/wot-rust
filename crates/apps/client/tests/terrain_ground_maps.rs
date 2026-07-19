//! Locks for the Terrain Material 2.0 ground-map bake (`client::bake_terrain_ground_maps`):
//! deterministic bytes, well-formed buffers, normalized layer weights, and coverage that
//! matches the map's own truth — roads wear to dirt, water margins to worn earth, steep quarry
//! faces break to rock, and the steppe splits between lush grass and dry straw. Pure CPU.

use client::{bake_terrain_ground_maps, terrain_material_set_for};
use terrain::{MapId, bystra_valley, prokhorovka_hill_252_2};

fn splitmix_hash(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

#[test]
fn the_bake_is_deterministic_and_well_formed() {
    let battlefield = prokhorovka_hill_252_2();
    let a = bake_terrain_ground_maps(&battlefield);
    let b = bake_terrain_ground_maps(&battlefield);
    assert!(a.is_well_formed());
    assert_eq!(splitmix_hash(&a.splat), splitmix_hash(&b.splat), "splat bake must be pure");
    assert_eq!(
        splitmix_hash(&a.macro_normal),
        splitmix_hash(&b.macro_normal),
        "macro-normal bake must be pure"
    );
    assert_eq!(a.extent_m, [1000.0, 1000.0], "the steppe spans its full kilometre");
}

#[test]
fn every_texel_weighs_its_layers_to_one() {
    let maps = bake_terrain_ground_maps(&prokhorovka_hill_252_2());
    for texel in maps.splat.chunks_exact(4) {
        let sum: u32 = texel.iter().map(|&w| w as u32).sum();
        assert!(
            (250..=258).contains(&sum),
            "layer weights must stay normalized within quantization: {texel:?} -> {sum}"
        );
    }
}

/// Sample the splat at a world point (nearest texel).
fn splat_at(maps: &renderer_api::TerrainGroundMaps, wx: f32, wz: f32) -> [u8; 4] {
    let tx = ((wx / maps.extent_m[0]) * maps.size as f32) as usize;
    let tz = ((wz / maps.extent_m[1]) * maps.size as f32) as usize;
    let i =
        (tz.min(maps.size as usize - 1) * maps.size as usize + tx.min(maps.size as usize - 1)) * 4;
    [maps.splat[i], maps.splat[i + 1], maps.splat[i + 2], maps.splat[i + 3]]
}

#[test]
fn the_splat_reads_the_maps_own_truth() {
    // Prokhorovka: a point on a road polyline wears dirt-dominant, and every layer owns real
    // ground somewhere on the map (grass, straw, road dirt, chalk break on the massif).
    let steppe = prokhorovka_hill_252_2();
    let maps = bake_terrain_ground_maps(&steppe);
    let road = &steppe.roads[0];
    let mid = road.points[road.points.len() / 2];
    let w = splat_at(&maps, mid[0], mid[1]);
    assert!(
        w[2] as u32 >= 140,
        "a road core must wear dirt-dominant: {w:?} at ({:.0},{:.0})",
        mid[0],
        mid[1]
    );
    let mut layer_seen = [false; 4];
    for texel in maps.splat.chunks_exact(4) {
        for (i, &v) in texel.iter().enumerate() {
            if v >= 96 {
                layer_seen[i] = true;
            }
        }
    }
    assert_eq!(layer_seen, [true; 4], "every layer must own real ground somewhere");

    // Bystra: the river margins read worn wet earth (the dirt layer claims them).
    let bystra = bystra_valley();
    let bystra_maps = bake_terrain_ground_maps(&bystra);
    let dirt_heavy = bystra_maps.splat.chunks_exact(4).filter(|t| t[2] >= 128).count();
    assert!(
        dirt_heavy > 1000,
        "the river corridor must wear its margins to earth: {dirt_heavy} texels"
    );

    // Prokhorovka is a dry steppe: grass + straw dominate almost everywhere.
    let steppe_maps = bake_terrain_ground_maps(&prokhorovka_hill_252_2());
    let grassy =
        steppe_maps.splat.chunks_exact(4).filter(|t| (t[0] as u32 + t[1] as u32) >= 128).count();
    let total = (steppe_maps.size * steppe_maps.size) as usize;
    assert!(
        grassy as f32 / total as f32 > 0.7,
        "the steppe must stay a grassland: {:.1}% grassy",
        grassy as f32 / total as f32 * 100.0
    );
}

#[test]
fn macro_normals_decode_to_unit_up_facing_vectors() {
    let maps = bake_terrain_ground_maps(&prokhorovka_hill_252_2());
    for texel in maps.macro_normal.chunks_exact(4).step_by(1097) {
        let n = [
            texel[0] as f32 / 255.0 * 2.0 - 1.0,
            texel[1] as f32 / 255.0 * 2.0 - 1.0,
            texel[2] as f32 / 255.0 * 2.0 - 1.0,
        ];
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        assert!((0.9..=1.1).contains(&len), "packed normal must stay unit-ish: {n:?}");
        assert!(n[1] > 0.2, "ground never overhangs on a heightfield: {n:?}");
    }
}

#[test]
fn puddle_propensity_is_smooth_varied_and_never_a_binary_grid_gate() {
    let maps = bake_terrain_ground_maps(&bystra_valley());
    let size = maps.size as usize;
    let alpha: Vec<u8> = maps.macro_normal.chunks_exact(4).map(|texel| texel[3]).collect();
    let unique: std::collections::BTreeSet<u8> = alpha.iter().copied().collect();
    let mut max_neighbour_jump = 0u8;
    for z in 0..size - 1 {
        for x in 0..size - 1 {
            let here = alpha[z * size + x];
            max_neighbour_jump = max_neighbour_jump.max(here.abs_diff(alpha[z * size + x + 1]));
            max_neighbour_jump = max_neighbour_jump.max(here.abs_diff(alpha[(z + 1) * size + x]));
        }
    }

    assert!(unique.len() > 128, "pooling mask must retain a broad smooth range");
    assert!(alpha.iter().any(|&value| value < 64), "steep ground must reject pooling");
    assert!(alpha.iter().any(|&value| value > 192), "flat ground must admit pooling");
    assert!(
        max_neighbour_jump <= 24,
        "blurred pooling must not reveal heightfield cell edges: jump {max_neighbour_jump}"
    );
}

#[test]
fn each_map_owns_a_policy_conformant_material_set() {
    // The selector is total over MapId and both palettes pass the bible's envelope (locked in
    // renderer_api); here we lock the mapping itself.
    assert_eq!(
        terrain_material_set_for(MapId::ProkhorovkaHill252_2),
        renderer_api::TerrainMaterialSet::prokhorovka()
    );
    assert_eq!(
        terrain_material_set_for(MapId::BystraValley),
        renderer_api::TerrainMaterialSet::bystra()
    );
}
