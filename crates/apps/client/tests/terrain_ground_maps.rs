//! Locks for the Terrain Material 2.0 ground-map bake (`client::bake_terrain_ground_maps`):
//! deterministic bytes, well-formed buffers, normalized layer weights, and coverage that
//! matches the map's own truth — roads wear to dirt, water margins to worn earth, steep quarry
//! faces break to rock, and the steppe splits between lush grass and dry straw. Pure CPU.

use client::{bake_terrain_ground_maps, terrain_material_set_for};
use terrain::MapId;

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
    let battlefield = map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2);
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
    let maps =
        bake_terrain_ground_maps(&map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2));
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
    let steppe = map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2);
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
    let bystra = map_forge::battlefield(terrain::MapId::BystraValley);
    let bystra_maps = bake_terrain_ground_maps(&bystra);
    let dirt_heavy = bystra_maps.splat.chunks_exact(4).filter(|t| t[2] >= 128).count();
    assert!(
        dirt_heavy > 1000,
        "the river corridor must wear its margins to earth: {dirt_heavy} texels"
    );

    // Prokhorovka is a dry steppe: grass + straw dominate almost everywhere.
    let steppe_maps =
        bake_terrain_ground_maps(&map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2));
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
    let maps =
        bake_terrain_ground_maps(&map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2));
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
    let maps = bake_terrain_ground_maps(&map_forge::battlefield(terrain::MapId::BystraValley));
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

    // STEEP GROUND REJECTS POOLING — asserted where the signal lives, not after the blur.
    //
    // This used to read `alpha.any(|v| v < 64)`, and it passed by 664 texels out of 1 048 576:
    // 0.06 %, on a knife edge. Measured across blur radii, the darkest texel on Bystra sits within
    // one radius step of the 64 threshold in either direction — r5 gives thousands of texels under
    // it, r7 gives twenty-four, r8 gives none — so the assertion was a coin toss about where one
    // extreme landed rather than a statement about the terrain. Densifying the map to 2.5 m nudged
    // that minimum from 44 to 64 and the "contract" flipped, which is how a marginal test reads as
    // a broken map.
    //
    // The flatness signal is the macro normal's Y, unblurred, in the same texture. `pooling_smooth‐
    // step(0.94, 0.997, n.y)` is what the bake feeds the mask, so n.y below 0.94 IS "steep enough
    // to reject pooling" — and there are thousands of those texels, on any grid the map might use.
    let steep = maps
        .macro_normal
        .chunks_exact(4)
        .filter(|texel| f32::from(texel[1]) / 255.0 * 2.0 - 1.0 < 0.94)
        .count();
    assert!(
        steep > 1_000,
        "steep ground must reject pooling — only {steep} texels are past the flatness edge, so \
         the mask has nothing to refuse"
    );
    assert!(alpha.iter().any(|&value| value > 192), "flat ground must admit pooling");
    assert!(
        max_neighbour_jump <= 24,
        "blurred pooling must not reveal heightfield cell edges: jump {max_neighbour_jump}"
    );
}

#[test]
fn each_map_owns_a_policy_conformant_material_set() {
    // The selector is total over MapId; each map's palette now comes from its BLUEPRINT
    // (`materials` section, envelope-enforced by the map report), and the two maps keep
    // distinct authored characters instead of sharing the fallback.
    let steppe = terrain_material_set_for(MapId::ProkhorovkaHill252_2);
    let valley = terrain_material_set_for(MapId::BystraValley);
    assert_ne!(steppe, valley, "each map authors its own ground palette");
    assert_ne!(
        valley,
        renderer_api::TerrainMaterialSet::default(),
        "an authored palette is not the fallback"
    );
}

/// The splat bake is a GOLDEN, not merely a pure function of itself.
///
/// The classification behind it — rock from steepness and crest, dirt from roads and water
/// margins, the remainder split by the patchwork noise — is about to stop living only in the
/// render bake and start being read by physics too, so that what you SEE under the track and what
/// you FEEL under it are one rule rather than two that can drift. The whole value of that move is
/// that it changes nothing about the picture, and "nothing" needs a number to be checkable.
///
/// If this hash moves, either the classification genuinely changed (bless it deliberately, in the
/// same diff, with the reason) or a refactor that promised to be inert was not.
#[test]
fn the_splat_bake_is_texel_identical_to_its_golden() {
    for (map, splat, normals) in [
        (MapId::ProkhorovkaHill252_2, 0x5124_4556_6116_dafa_u64, 0x005a_3eac_ae02_f08a_u64),
        (MapId::BystraValley, 0x4e34_1f19_bb83_6d56, 0x83a4_b469_4777_9f54),
        (MapId::OrlinyPereval, 0x9699_5682_1414_f5bd, 0x4576_7f22_e480_fec5),
        (MapId::Ostrogorsk, 0x4505_e21f_f799_e6ab, 0x28f1_5c8d_6b79_48eb),
    ] {
        let maps = bake_terrain_ground_maps(&map_forge::battlefield(map));
        assert_eq!(splitmix_hash(&maps.splat), splat, "{map:?} splat");
        assert_eq!(splitmix_hash(&maps.macro_normal), normals, "{map:?} macro normals");
    }
}
