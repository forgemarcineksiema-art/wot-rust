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
    // Prokhorovka: what a road wears depends on what it is MADE OF (teren A2) — the
    // railway ballast is crushed stone and splats rock-dominant; a farm road is packed
    // earth and stays dirt-dominant. And every layer owns real ground somewhere.
    let steppe = map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2);
    let maps = bake_terrain_ground_maps(&steppe);
    let ballast = &steppe.roads[0];
    assert_eq!(ballast.surface, terrain::RoadSurface::Ballast, "roads[0] is the railbed");
    let mid = ballast.points[ballast.points.len() / 2];
    let w = splat_at(&maps, mid[0], mid[1]);
    assert!(
        w[3] as u32 >= 140,
        "a ballast core must wear stone-dominant: {w:?} at ({:.0},{:.0})",
        mid[0],
        mid[1]
    );
    let farm = steppe
        .roads
        .iter()
        .find(|road| road.surface == terrain::RoadSurface::Dirt)
        .expect("the steppe keeps its packed-earth lanes");
    let mid = farm.points[farm.points.len() / 2];
    let w = splat_at(&maps, mid[0], mid[1]);
    assert!(
        w[2] as u32 >= 140,
        "a farm-road core must stay dirt-dominant: {w:?} at ({:.0},{:.0})",
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
    // Blessed 2026-08-04 (teren A1): structural ground moisture entered the rule — the D8
    // drainage darkens the splat's wet-earth lane and glosses the pooling alpha. Deliberate,
    // locked in `terrain::ground` (margin caps flow) and `terrain::flow` (determinism).
    // Orliny's NORMALS hash survived untouched: its valley floors were already fully flat in
    // the pooling term, so the flow lane never exceeded them at u8 quantization.
    // Blessed again 2026-08-04 (teren A2): Ballast/Cobble route to the ROCK lane — only the
    // two maps with stone roads moved (Prokhorovka's railbed, Ostrogorsk's streets); Bystra
    // (roadless) and Orliny (dirt-only) staying bit-identical IS the lock on
    // `strongest_road_at`'s float-identity with the old fold.
    // Blessed again 2026-08-04 (teren B2): roads wear a baked CROWN in the macro normal —
    // only the three maps WITH roads moved their normals, and only their normals: the splat
    // stays bit-identical everywhere because the crown bends the visual lane, never the
    // weights physics reads. Roadless Bystra did not move at all.
    // Blessed again 2026-08-05 (teren C2): the Prokhorovka sculpt session — balka
    // tributaries, approach crests, the rail borrow ditch. Only Prokhorovka moved, and its
    // whole envelope held: all five contract suites, symmetry, playability BFS and the
    // hull-down census passed over the sculpted ground before this bless.
    // Blessed again 2026-08-25 (surface parity), ALL EIGHT HASHES: `sample_height` stands
    // on the render mesh's own triangle planes instead of a bilinear patch (the shared
    // per-cell diagonal is the flatter one, mirror-symmetric on fair maps), so
    // every texel sampled INSIDE a cell moved by up to the old twist residual (<= 0.48 m of
    // ground; the classification and its weights rule are untouched). The drive and the
    // picture keep reading one surface - now the same one the sim shoots and spots against.
    // Blessed again 2026-08-05 (teren C3): the Bystra sculpt session — first roads (the
    // gravel tracks and cobbled street splat as stone since A2), the second floodplain
    // terrace and swales (the moisture rule wets the wider lowland), the bridgehead bluffs
    // and the RoadProfile causeway (its crown rides the baked normal). Only Bystra moved.
    for (map, splat, normals) in [
        (MapId::ProkhorovkaHill252_2, 0x6562_9cd0_6d40_f32f_u64, 0xeca4_e313_ca58_d880_u64),
        (MapId::BystraValley, 0x624c_74b0_1cb1_c388, 0xe671_a539_e8e2_81b0),
        (MapId::OrlinyPereval, 0xcfe5_3851_bf87_55d4, 0x97bb_a288_deee_7e8c),
        (MapId::Ostrogorsk, 0x4493_96c3_9f3d_9792, 0x3523_a71c_7d66_a610),
    ] {
        let maps = bake_terrain_ground_maps(&map_forge::battlefield(map));
        assert_eq!(splitmix_hash(&maps.splat), splat, "{map:?} splat");
        assert_eq!(splitmix_hash(&maps.macro_normal), normals, "{map:?} macro normals");
    }
}
