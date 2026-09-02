//! Authored flora data (route 2, the owner's call of 2026-09-02: "trees as data, authored
//! offline in Blender, baked into our format, runtime unchanged"), for EVERY species, in
//! VARIANTS (the owner's second call the same evening: "leaves denser, branches better made,
//! the same technology for the other species, a natural generator of sizes — small to big,
//! thin to thick, low to tall, dense or not").
//!
//! Per species (`assets/flora/<species>/`):
//! - the leaf-CLUSTER pages: four sprites of a twig with a few hundred individual leaves
//!   (needle fascicles for the pine), rendered orthographically by Cycles under a uniform white
//!   world (`scripts/flora/bake_clusters.py`), so the colour page stores ALBEDO × local
//!   occlusion — the convention the procedural SDF slots stored, and the engine's FOLIAGE path
//!   lights it live. The normal page is the camera-space normal, `n * 0.5 + 0.5` (the atlas'
//!   tangent-space "dome" convention: right, up, toward the viewer). One 2048×512 row per
//!   species, pasted under the procedural page by `leaf_atlas`.
//! - four skeleton VARIANTS (young / mature / old / sparse), two rungs each, grown by Sapling
//!   Tree Gen (`scripts/flora/bake_tree.py`): trunk, limbs and twigs as wood, every Sapling
//!   leaf quad as a cross pair of cluster cards.
//! - a CC0 bark tile (Poly Haven; the licence file sits beside it), albedo + tangent normals,
//!   one layer of the renderer's bark array.
//!
//! Everything ships INSIDE the binary (`include_bytes!`): an asset missing at runtime would
//! draw a tree with no leaves, and a picture that depends on a working directory is not a
//! deterministic picture. Every asset's identity is a golden hash — a re-bake is a deliberate
//! diff, never drift. The procedural-only rule of map-forge policy #10 is amended by this
//! module, not silently broken.

use std::io::Cursor;
use std::sync::OnceLock;

use glam::Vec3;
use vehicle_geometry::{GeometryMesh, GeometryVertex, SmoothingGroup};

use super::leaves::LeafCard;
use super::{BakedTree, TreeLod, TreeSpecies};
use crate::WorldMaterial;

/// A species' cluster block: `CLUSTER_GRID_W` × `CLUSTER_GRID_H` square sprites in one row.
pub const CLUSTER_SPRITE_PX: u32 = 512;
pub const CLUSTER_GRID_W: u32 = 4;
pub const CLUSTER_GRID_H: u32 = 1;
pub const CLUSTER_PAGE_W: u32 = CLUSTER_GRID_W * CLUSTER_SPRITE_PX;
pub const CLUSTER_PAGE_H: u32 = CLUSTER_GRID_H * CLUSTER_SPRITE_PX;
/// Sprites per species block.
pub const CLUSTER_SPRITES: u32 = CLUSTER_GRID_W * CLUSTER_GRID_H;
/// Skeleton variants per species, in the exporter's order: young, mature, old, sparse.
pub const VARIANTS: u32 = 4;
/// The variant the ladder's representative individual and the impostor stand on.
pub const REFERENCE_VARIANT: u32 = 1;
/// A bark layer's size: 1 m × 2 m at 1024 × 2048 (square tiles are stacked twice).
pub const BARK_W: u32 = 1024;
pub const BARK_H: u32 = 2048;

/// Both authored pages of one species' clusters, tightly packed RGBA8, row 0 at the TOP.
/// `color` is albedo·occlusion with the cutout alpha; `normal` is the dome page with alpha
/// 255 everywhere and the flat texel (128, 128, 255) wherever the colour page is cut.
#[derive(Debug, Clone)]
pub struct ClusterPages {
    pub width: u32,
    pub height: u32,
    pub color: Vec<u8>,
    pub normal: Vec<u8>,
}

impl ClusterPages {
    /// The asset's identity: FNV over both pages, the same hash the leaf atlas uses.
    pub fn deterministic_hash(&self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        for byte in self.color.iter().chain(self.normal.iter()) {
            crate::fnv(&mut hash, u64::from(*byte));
        }
        hash
    }

    /// The colour page's alpha coverage of one sprite (≥ 128 counts as leaf), 0..=1.
    pub fn sprite_coverage(&self, sprite: u32) -> f32 {
        let (x0, y0) = sprite_origin(sprite);
        let mut covered = 0u32;
        for y in y0..y0 + CLUSTER_SPRITE_PX {
            for x in x0..x0 + CLUSTER_SPRITE_PX {
                let alpha = self.color[((y * self.width + x) * 4 + 3) as usize];
                covered += u32::from(alpha >= 128);
            }
        }
        covered as f32 / (CLUSTER_SPRITE_PX * CLUSTER_SPRITE_PX) as f32
    }
}

/// Where sprite `sprite` (row-major) starts inside a cluster page, texels.
pub fn sprite_origin(sprite: u32) -> (u32, u32) {
    ((sprite % CLUSTER_GRID_W) * CLUSTER_SPRITE_PX, (sprite / CLUSTER_GRID_W) * CLUSTER_SPRITE_PX)
}

/// One decoded bark page: tightly packed RGBA8, row 0 at the top, `BARK_W` × `BARK_H`.
#[derive(Debug, Clone)]
pub struct BarkPage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// The raw files of one species, straight from the binary.
struct SpeciesAssets {
    dir: &'static str,
    bark_dir: &'static str,
    cluster_color: &'static [u8],
    cluster_normal: &'static [u8],
    /// `[variant][rung]`: rung 0 = Close (near), 1 = Mid.
    trees: [[&'static [u8]; 2]; VARIANTS as usize],
    bark_albedo: &'static [u8],
    bark_normal: &'static [u8],
}

macro_rules! flora_file {
    ($dir:literal, $file:literal) => {
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../assets/flora/", $dir, $file))
    };
}

macro_rules! flora {
    ($dir:literal, $bark:literal) => {
        SpeciesAssets {
            dir: $dir,
            bark_dir: $bark,
            cluster_color: flora_file!($dir, "/clusters_color.png"),
            cluster_normal: flora_file!($dir, "/clusters_normal.png"),
            trees: [
                [flora_file!($dir, "/v0/tree_near.bin"), flora_file!($dir, "/v0/tree_mid.bin")],
                [flora_file!($dir, "/v1/tree_near.bin"), flora_file!($dir, "/v1/tree_mid.bin")],
                [flora_file!($dir, "/v2/tree_near.bin"), flora_file!($dir, "/v2/tree_mid.bin")],
                [flora_file!($dir, "/v3/tree_near.bin"), flora_file!($dir, "/v3/tree_mid.bin")],
            ],
            bark_albedo: include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../assets/flora/bark/",
                $bark,
                "/diff_1k.png"
            )),
            bark_normal: include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../assets/flora/bark/",
                $bark,
                "/nor_gl_1k.png"
            )),
        }
    };
}

static OAK: SpeciesAssets = flora!("oak", "jolcham_oak_bark_01");
static POPLAR: SpeciesAssets = flora!("poplar", "bark_brown_02");
static WILLOW: SpeciesAssets = flora!("willow", "bark_willow_02");
static FRUIT: SpeciesAssets = flora!("fruit", "sakura_bark");
static BUSH: SpeciesAssets = flora!("bush", "tree_bark_03");
static PINE: SpeciesAssets = flora!("pine", "pine_bark");

fn assets(species: TreeSpecies) -> &'static SpeciesAssets {
    match species {
        TreeSpecies::Oak => &OAK,
        TreeSpecies::Poplar => &POPLAR,
        TreeSpecies::Willow => &WILLOW,
        TreeSpecies::FruitTree => &FRUIT,
        TreeSpecies::Bush => &BUSH,
        TreeSpecies::Pine => &PINE,
    }
}

/// The species' position in `TreeSpecies::ALL`: its cluster row and its bark layer.
pub fn species_index(species: TreeSpecies) -> u32 {
    TreeSpecies::ALL.iter().position(|&s| s == species).expect("every species is in ALL") as u32
}

/// The asset directory names (species, bark), for reports and tests.
pub fn asset_dirs(species: TreeSpecies) -> (&'static str, &'static str) {
    let a = assets(species);
    (a.dir, a.bark_dir)
}

/// The golden hashes per species: (clusters, the eight tree files, the bark pair). A re-bake
/// changes the picture: bless deliberately and say what changed about the LEAVES, the SHAPE
/// or the BARK.
pub const SPECIES_GOLDENS: [(TreeSpecies, u64, u64, u64); 6] = [
    (TreeSpecies::Oak, 0x9589_9f13_6788_6aa6, 0xc331_0ccc_446a_23b5, 0x42fd_61ea_222f_dd31),
    (TreeSpecies::Poplar, 0xd426_f283_936a_4735, 0x73d9_a96c_aa6c_f683, 0x9114_1fd6_45f4_f3b1),
    (TreeSpecies::Willow, 0x3cba_2693_d88b_7b33, 0x5806_ed76_1308_6f4f, 0x69d3_ab60_3d49_f2a1),
    (TreeSpecies::FruitTree, 0xeae0_1018_e1d2_b111, 0xe786_8cc2_b1a9_2398, 0x14d1_95fa_d5c6_ec4d),
    (TreeSpecies::Bush, 0xf651_19f7_6c42_fc81, 0x2b95_10f3_dccb_0f40, 0x16db_0350_dcc4_d74d),
    (TreeSpecies::Pine, 0xac91_6545_a0ad_d895, 0xcd2b_ce5e_6424_da87, 0x0f83_122c_b8de_75f1),
];

fn fnv_bytes(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        crate::fnv(hash, u64::from(*byte));
    }
}

/// FNV over a species' eight tree files.
pub fn trees_hash(species: TreeSpecies) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for rungs in &assets(species).trees {
        for bytes in rungs {
            fnv_bytes(&mut hash, bytes);
        }
    }
    hash
}

/// FNV over a species' decoded bark pair.
pub fn bark_hash(species: TreeSpecies) -> u64 {
    let (albedo, normal) = bark_pages(species);
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    fnv_bytes(&mut hash, &albedo.rgba);
    fnv_bytes(&mut hash, &normal.rgba);
    hash
}

/// The authored cluster pages of a species (every species has them). Decoded once per
/// process.
pub fn clusters(species: TreeSpecies) -> Option<&'static ClusterPages> {
    static PAGES: [OnceLock<ClusterPages>; 6] = [const { OnceLock::new() }; 6];
    let a = assets(species);
    Some(
        PAGES[species_index(species) as usize]
            .get_or_init(|| decode_cluster_pages(a.cluster_color, a.cluster_normal)),
    )
}

/// The bark pair (albedo, normals) of a species, `BARK_W` × `BARK_H`, decoded once.
pub fn bark_pages(species: TreeSpecies) -> &'static (BarkPage, BarkPage) {
    static PAGES: [OnceLock<(BarkPage, BarkPage)>; 6] = [const { OnceLock::new() }; 6];
    let a = assets(species);
    PAGES[species_index(species) as usize]
        .get_or_init(|| (decode_bark(a.bark_albedo), decode_bark(a.bark_normal)))
}

/// Decode one PNG to tightly packed RGBA8 (8-bit, alpha expanded).
fn decode_rgba(bytes: &[u8]) -> (u32, u32, Vec<u8>) {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().expect("authored flora png: header");
    let mut buf = vec![0u8; reader.output_buffer_size().expect("authored flora png: size")];
    let info = reader.next_frame(&mut buf).expect("authored flora png: frame");
    buf.truncate(info.buffer_size());
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => buf.chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
        other => panic!("authored flora png: unsupported colour type {other:?}"),
    };
    (info.width, info.height, rgba)
}

fn decode_cluster_pages(color_png: &[u8], normal_png: &[u8]) -> ClusterPages {
    let (width, height, color) = decode_rgba(color_png);
    let (normal_w, normal_h, raw_normal) = decode_rgba(normal_png);
    assert_eq!((width, height), (CLUSTER_PAGE_W, CLUSTER_PAGE_H), "cluster colour page size");
    assert_eq!((normal_w, normal_h), (width, height), "cluster normal page size");
    // The normal render's background is black; the atlas wants the flat dome texel there so
    // the box-filtered mips never pull a rim normal toward (−1, −1, −1). Alpha is opaque:
    // the normal page carries no cutout of its own.
    let normal: Vec<u8> = color
        .chunks_exact(4)
        .zip(raw_normal.chunks_exact(4))
        .flat_map(|(c, n)| if c[3] < 8 { [128, 128, 255, 255] } else { [n[0], n[1], n[2], 255] })
        .collect();
    ClusterPages { width, height, color, normal }
}

/// A bark tile as one `BARK_W` × `BARK_H` layer: a 1 × 2 source as is, a square source
/// stacked twice (the tile is 1 m wide and 2 m tall on the trunk either way).
fn decode_bark(bytes: &[u8]) -> BarkPage {
    let (width, height, rgba) = decode_rgba(bytes);
    assert_eq!(width, BARK_W, "bark tile width");
    let rgba = if height == BARK_H {
        rgba
    } else {
        assert_eq!(height * 2, BARK_H, "bark tile is 1:2 or square");
        let mut stacked = rgba.clone();
        stacked.extend_from_slice(&rgba);
        stacked
    };
    BarkPage { width: BARK_W, height: BARK_H, rgba }
}

/// The shade lane of an authored deck: rim cards at 1.0, core cards down to this — the same
/// one-mass law the procedural dealer applied (`leaves::CORE_SHADE`).
const CORE_SHADE: f32 = 0.68;

/// Which variant and mirror a seed names. The ladder passes `variant_seed(v)`; the statics
/// bake and the tree line pass position bits, so a shelterbelt is a population, not one
/// tree stamped. A mixed hash, because raw position bits share their low bits.
pub fn variant_of_seed(seed: u64) -> (u32, bool) {
    // A splitmix64 finaliser: small seeds (the ladder's, the tests') and position bits alike
    // spread over every variant and both mirrors.
    let mut x = seed ^ 0x9e37_79b9_7f4a_7c15;
    x ^= x >> 30;
    x = x.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^= x >> 31;
    (((x >> 8) % u64::from(VARIANTS)) as u32, (x >> 20) & 1 == 1)
}

/// The seed that names variant `variant`, unmirrored — found by search, because the seed's
/// hash decides. Cheap (a handful of FNV rounds) and deterministic.
pub fn variant_seed(variant: u32) -> u64 {
    seed_for(variant, false)
}

/// The seed that names (`variant`, `mirrored`), found by search.
pub fn seed_for(variant: u32, mirrored: bool) -> u64 {
    (0..4096u64)
        .find(|&seed| variant_of_seed(seed) == (variant % VARIANTS, mirrored))
        .expect("every (variant, mirror) has a seed under 4096")
}

/// The authored tree of a species at a rung: the variant and mirror the seed names.
pub fn tree(species: TreeSpecies, seed: u64, lod: TreeLod) -> Option<BakedTree> {
    let (variant, mirrored) = variant_of_seed(seed);
    let mut tree = tree_variant(species, variant, lod);
    if mirrored {
        mirror_authored_tree_across_x(&mut tree);
    }
    Some(tree)
}

/// One named variant, unmirrored.
pub fn tree_variant(species: TreeSpecies, variant: u32, lod: TreeLod) -> BakedTree {
    let rung = match lod {
        TreeLod::Close => 0,
        TreeLod::Mid => 1,
    };
    parse_tree(species, assets(species).trees[(variant % VARIANTS) as usize][rung])
}

fn mirror_authored_tree_across_x(tree: &mut BakedTree) {
    let flip = |v: Vec3| Vec3::new(-v.x, v.y, v.z);
    let mut vertices: Vec<GeometryVertex> = tree.trunk.vertices().to_vec();
    for vertex in &mut vertices {
        vertex.position = flip(vertex.position);
        vertex.normal = flip(vertex.normal);
    }
    let mut indices: Vec<u32> = tree.trunk.indices().to_vec();
    for triangle in indices.chunks_exact_mut(3) {
        triangle.swap(1, 2);
    }
    tree.trunk = GeometryMesh::new(vertices, indices);
    for card in &mut tree.leaves {
        card.center = flip(card.center);
        card.half_right = flip(card.half_right);
        card.half_up = flip(card.half_up);
        card.normal = flip(card.normal);
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn u32(&mut self) -> u32 {
        let v = u32::from_le_bytes(self.bytes[self.at..self.at + 4].try_into().expect("u32"));
        self.at += 4;
        v
    }

    fn f32(&mut self) -> f32 {
        f32::from_bits(self.u32())
    }

    fn u8(&mut self) -> u8 {
        let v = self.bytes[self.at];
        self.at += 1;
        v
    }

    fn vec3(&mut self) -> Vec3 {
        Vec3::new(self.f32(), self.f32(), self.f32())
    }
}

/// `WOTTREE1`: u32 nverts, nverts × (pos, normal), u32 nidx, nidx × u32, u32 ncards,
/// ncards × (center, half_right, half_up, normal, u8 sprite). Engine space, Y up, grounded.
fn parse_tree(species: TreeSpecies, bytes: &[u8]) -> BakedTree {
    assert_eq!(&bytes[..8], b"WOTTREE1", "authored tree: magic");
    let mut r = Reader { bytes, at: 8 };
    let nverts = r.u32() as usize;
    let mut vertices = Vec::with_capacity(nverts);
    for _ in 0..nverts {
        let position = r.vec3();
        let normal = r.vec3();
        vertices.push(GeometryVertex::new(
            position,
            normal.normalize_or_zero(),
            WorldMaterial::Bark.carrier(),
            SmoothingGroup(1),
        ));
    }
    let nidx = r.u32() as usize;
    let indices: Vec<u32> = (0..nidx).map(|_| r.u32()).collect();
    assert!(indices.iter().all(|&i| (i as usize) < nverts), "authored tree: index range");
    let ncards = r.u32() as usize;
    let mut raw: Vec<(Vec3, Vec3, Vec3, Vec3, u8)> = Vec::with_capacity(ncards);
    for _ in 0..ncards {
        let center = r.vec3();
        let half_right = r.vec3();
        let half_up = r.vec3();
        let normal = r.vec3();
        let sprite = r.u8();
        raw.push((center, half_right, half_up, normal, sprite));
    }
    assert_eq!(r.at, bytes.len(), "authored tree: trailing bytes");
    let centroid = raw.iter().map(|c| c.0).sum::<Vec3>() / (ncards.max(1) as f32);
    let reach = raw.iter().map(|c| c.0.distance(centroid)).fold(0.01_f32, f32::max);
    let slot_base = super::leaf_atlas::cluster_slot_base(species);
    let leaves = raw
        .into_iter()
        .map(|(center, half_right, half_up, normal, sprite)| LeafCard {
            center,
            half_right,
            half_up,
            normal: normal.normalize_or_zero(),
            slot: slot_base + sprite % (CLUSTER_SPRITES as u8),
            shade: CORE_SHADE
                + (1.0 - CORE_SHADE) * (center.distance(centroid) / reach).clamp(0.0, 1.0),
        })
        .collect();
    BakedTree {
        species,
        trunk: GeometryMesh::new(vertices, indices),
        // Authored crowns are their own mass; no occlusion hull.
        canopy: GeometryMesh::new(Vec::new(), Vec::new()),
        leaves,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every species: the three asset hashes on their goldens.
    #[test]
    fn every_species_is_on_its_goldens() {
        for (species, clusters_golden, trees_golden, bark_golden) in SPECIES_GOLDENS {
            let pages = clusters(species).expect("clusters");
            assert_eq!(
                pages.deterministic_hash(),
                clusters_golden,
                "{species:?} clusters changed — bless (0x{:016x})",
                pages.deterministic_hash()
            );
            assert_eq!(
                trees_hash(species),
                trees_golden,
                "{species:?} trees changed — bless (0x{:016x})",
                trees_hash(species)
            );
            assert_eq!(
                bark_hash(species),
                bark_golden,
                "{species:?} bark changed — bless (0x{:016x})",
                bark_hash(species)
            );
        }
    }

    /// Every sprite of every species is a real but sparse cluster, rooted at its bottom
    /// centre (the card's stem), the normal page flat outside the leaves and bent inside.
    #[test]
    fn every_species_cluster_block_is_well_formed() {
        for species in TreeSpecies::ALL {
            let pages = clusters(species).expect("clusters");
            assert_eq!((pages.width, pages.height), (CLUSTER_PAGE_W, CLUSTER_PAGE_H));
            for sprite in 0..CLUSTER_SPRITES {
                let coverage = pages.sprite_coverage(sprite);
                assert!(
                    (0.06..=0.75).contains(&coverage),
                    "{species:?} sprite {sprite}: coverage {coverage:.3}"
                );
                let (x0, y0) = sprite_origin(sprite);
                let mut rooted = false;
                for y in (y0 + CLUSTER_SPRITE_PX - 40)..(y0 + CLUSTER_SPRITE_PX) {
                    for x in (x0 + CLUSTER_SPRITE_PX / 2 - 48)..(x0 + CLUSTER_SPRITE_PX / 2 + 48) {
                        rooted |= pages.color[((y * pages.width + x) * 4 + 3) as usize] >= 128;
                    }
                }
                assert!(rooted, "{species:?} sprite {sprite} is not rooted at its bottom centre");
            }
            let mut inside = 0u32;
            let mut bent = 0u32;
            for (c, n) in pages.color.chunks_exact(4).zip(pages.normal.chunks_exact(4)) {
                if c[3] < 8 {
                    assert_eq!(&n[..3], &[128, 128, 255], "{species:?}: cut texel must be flat");
                } else if c[3] >= 128 {
                    inside += 1;
                    bent += u32::from(
                        (i32::from(n[0]) - 128).abs() + (i32::from(n[1]) - 128).abs() > 12,
                    );
                }
            }
            assert!(inside > 0);
            assert!(bent * 100 >= inside * 15, "{species:?}: real normals: {bent}/{inside}");
        }
    }

    /// Every variant of every species: valid wood inside the budget, a cross-pair deck on
    /// the species' cluster block, the rungs agreeing in height, grounded; the variants
    /// really differ in size (young under mature under old) and the mirror keeps its winding.
    #[test]
    fn every_variant_of_every_species_is_a_valid_tree() {
        for species in TreeSpecies::ALL {
            let mut tips = Vec::new();
            for variant in 0..VARIANTS {
                let near = tree_variant(species, variant, TreeLod::Close);
                let mid = tree_variant(species, variant, TreeLod::Mid);
                assert!(
                    near.trunk.triangle_count() <= 12_000,
                    "{species:?} v{variant}: near wood {}",
                    near.trunk.triangle_count()
                );
                assert!(mid.trunk.triangle_count() <= near.trunk.triangle_count());
                assert!(
                    (60..=640).contains(&near.leaves.len()),
                    "{species:?} v{variant}: near deck {}",
                    near.leaves.len()
                );
                assert!(mid.leaves.len() <= near.leaves.len());
                assert!(near.leaves.len().is_multiple_of(2), "cross pairs come in twos");
                assert!(
                    (near.tip() - mid.tip()).abs() < 0.05,
                    "{species:?} v{variant}: rung tips {} vs {}",
                    near.tip(),
                    mid.tip()
                );
                let low =
                    near.trunk.vertices().iter().map(|v| v.position.y).fold(f32::MAX, f32::min);
                assert!(low.abs() < 0.25, "{species:?} v{variant}: grounded at y = 0: {low}");
                let slot_base = super::super::leaf_atlas::cluster_slot_base(species);
                for card in &near.leaves {
                    assert!((slot_base..slot_base + CLUSTER_SPRITES as u8).contains(&card.slot));
                    assert!((0.68..=1.0).contains(&card.shade));
                }
                for pair in near.leaves.chunks_exact(2) {
                    assert_eq!(pair[0].center, pair[1].center);
                    assert!(pair[0].normal.dot(pair[1].normal).abs() < 0.05, "perpendicular cross");
                }
                assert!(
                    near.trunk.validate_quality(vehicle_geometry::OPEN_OR_CLOSED_MESH).is_ok(),
                    "{species:?} v{variant}: the authored wood is a valid mesh"
                );
                tips.push(near.tip());
            }
            assert!(
                tips[0] < tips[1] && tips[1] < tips[2],
                "{species:?}: young < mature < old: {tips:?}"
            );
            let plain =
                tree(species, variant_seed(REFERENCE_VARIANT), TreeLod::Close).expect("tree");
            assert_eq!(
                plain.leaves.len(),
                tree_variant(species, REFERENCE_VARIANT, TreeLod::Close).leaves.len()
            );
            let mut mirrored_seed = 0;
            while variant_of_seed(mirrored_seed) != (REFERENCE_VARIANT, true) {
                mirrored_seed += 1;
            }
            let mirrored = tree(species, mirrored_seed, TreeLod::Close).expect("tree");
            assert!((mirrored.tip() - plain.tip()).abs() < 1.0e-3);
            assert_ne!(mirrored.leaves[0].center, plain.leaves[0].center);
            assert!(
                mirrored.trunk.validate_quality(vehicle_geometry::OPEN_OR_CLOSED_MESH).is_ok(),
                "{species:?}: the mirrored wood keeps its winding"
            );
        }
    }

    /// The seed → variant map spreads position seeds over every variant and both mirrors.
    #[test]
    fn position_seeds_spread_over_the_variants() {
        let mut seen = std::collections::BTreeSet::new();
        for x in 0..40 {
            let position = [10.0 + x as f32 * 7.3, 0.0, 4.0 + x as f32 * 3.1];
            let seed = position[0].to_bits() as u64 ^ ((position[2].to_bits() as u64) << 32);
            seen.insert(variant_of_seed(seed));
        }
        assert!(seen.len() >= 7, "variants × mirrors reached: {seen:?}");
        for variant in 0..VARIANTS {
            assert_eq!(variant_of_seed(variant_seed(variant)), (variant, false));
        }
    }

    /// Every bark layer: a 1 × 2 tile, the normal page a real normal map, the albedo a bark.
    #[test]
    fn every_bark_layer_is_a_bark() {
        for species in TreeSpecies::ALL {
            let (albedo, normal) = bark_pages(species);
            assert_eq!((albedo.width, albedo.height), (BARK_W, BARK_H));
            assert_eq!((normal.width, normal.height), (BARK_W, BARK_H));
            let mean = |page: &BarkPage, channel: usize| {
                page.rgba.chunks_exact(4).map(|t| u32::from(t[channel])).sum::<u32>() as f32
                    / (page.width * page.height) as f32
            };
            assert!(mean(normal, 2) > 190.0, "{species:?}: normals point out: {}", mean(normal, 2));
            let (r, g, b) = (mean(albedo, 0), mean(albedo, 1), mean(albedo, 2));
            assert!(
                r < 190.0 && r >= b,
                "{species:?}: bark is a brown/grey, not a white or a blue: ({r}, {g}, {b})"
            );
        }
    }
}
