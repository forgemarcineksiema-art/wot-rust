//! Authored flora data (route 2, the owner's call of 2026-09-02: "trees as data, authored
//! offline in Blender, baked into our format, runtime unchanged").
//!
//! The first authored piece is the oak's leaf-CLUSTER pages: eight sprites of a twig with a
//! few dozen individual leaves, rendered orthographically by Cycles under a uniform white
//! world (`scripts/flora/bake_oak_clusters.py`), so the colour page stores ALBEDO × local
//! occlusion — the same convention the procedural SDF slots stored, and the engine's FOLIAGE
//! path lights it live. The normal page is the camera-space normal, `n * 0.5 + 0.5`, which
//! is the atlas' tangent-space "dome" convention (right, up, toward the viewer).
//!
//! The pages ship INSIDE the binary (`include_bytes!` from `assets/flora/<species>/`) — an
//! asset that is missing at runtime would draw a tree with no leaves, and a picture that
//! depends on a working directory is not a deterministic picture. Their identity is a golden
//! hash: a re-bake is a deliberate diff, never drift. No third-party asset is involved (the
//! manifest next to the pages says so); the procedural-only rule of map-forge policy #10 is
//! amended by this module, not silently broken.

use std::io::Cursor;
use std::sync::OnceLock;

use glam::Vec3;
use vehicle_geometry::{GeometryMesh, GeometryVertex, SmoothingGroup};

use super::leaves::LeafCard;
use super::{BakedTree, TreeLod, TreeSpecies};
use crate::WorldMaterial;

/// The cluster block's page: a `CLUSTER_GRID_W` × `CLUSTER_GRID_H` grid of square sprites.
pub const CLUSTER_SPRITE_PX: u32 = 512;
pub const CLUSTER_GRID_W: u32 = 4;
pub const CLUSTER_GRID_H: u32 = 2;
pub const CLUSTER_PAGE_W: u32 = CLUSTER_GRID_W * CLUSTER_SPRITE_PX;
pub const CLUSTER_PAGE_H: u32 = CLUSTER_GRID_H * CLUSTER_SPRITE_PX;
/// Sprites per species block.
pub const CLUSTER_SPRITES: u32 = CLUSTER_GRID_W * CLUSTER_GRID_H;

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

/// The golden hash of the oak's cluster pages as baked on 2026-09-02 (seed 1, 96 samples,
/// Blender 5.2.0 LTS, Cycles on the MX330). A re-bake changes the picture: bless deliberately
/// and say what changed about the LEAVES.
pub const OAK_CLUSTERS_GOLDEN: u64 = 0xde88_4b37_8202_081d;

static OAK_COLOR_PNG: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../assets/flora/oak/clusters_color.png"
));
static OAK_NORMAL_PNG: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../assets/flora/oak/clusters_normal.png"
));

/// The authored cluster pages of a species, if it has them. Decoded once per process.
pub fn clusters(species: TreeSpecies) -> Option<&'static ClusterPages> {
    match species {
        TreeSpecies::Oak => Some(oak_clusters()),
        TreeSpecies::Poplar
        | TreeSpecies::Willow
        | TreeSpecies::FruitTree
        | TreeSpecies::Bush
        | TreeSpecies::Pine => None,
    }
}

fn oak_clusters() -> &'static ClusterPages {
    static PAGES: OnceLock<ClusterPages> = OnceLock::new();
    PAGES.get_or_init(|| decode_pages(OAK_COLOR_PNG, OAK_NORMAL_PNG))
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

fn decode_pages(color_png: &[u8], normal_png: &[u8]) -> ClusterPages {
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

// ---------------------------------------------------------------------------------------
// The BARK pair: a CC0 photographic tile (Poly Haven `jolcham_oak_bark_01`, 1 m × 2 m,
// licence in `assets/flora/bark/jolcham_oak_bark_01/LICENSE.md`), albedo and OpenGL-convention
// tangent normals, re-encoded to 8-bit PNG by `scripts/flora/convert_bark.py`.

static BARK_ALBEDO_PNG: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../assets/flora/bark/jolcham_oak_bark_01/diff_1k.png"
));
static BARK_NORMAL_PNG: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../assets/flora/bark/jolcham_oak_bark_01/nor_gl_1k.png"
));

/// One decoded bark page: tightly packed RGBA8, row 0 at the top.
#[derive(Debug, Clone)]
pub struct BarkPage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// The golden hash of the bark pair as embedded on 2026-09-02.
pub const BARK_GOLDEN: u64 = 0x42fd_61ea_222f_dd31;

/// The bark pair (albedo, normals), decoded once per process.
pub fn bark_pages() -> &'static (BarkPage, BarkPage) {
    static PAGES: OnceLock<(BarkPage, BarkPage)> = OnceLock::new();
    PAGES.get_or_init(|| {
        let (w, h, rgba) = decode_rgba(BARK_ALBEDO_PNG);
        let (nw, nh, nrgba) = decode_rgba(BARK_NORMAL_PNG);
        assert_eq!((w, h), (nw, nh), "bark albedo and normal pages agree in size");
        (BarkPage { width: w, height: h, rgba }, BarkPage { width: nw, height: nh, rgba: nrgba })
    })
}

/// FNV over both bark pages.
pub fn bark_hash() -> u64 {
    let (albedo, normal) = bark_pages();
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in albedo.rgba.iter().chain(normal.rgba.iter()) {
        crate::fnv(&mut hash, u64::from(*byte));
    }
    hash
}

// ---------------------------------------------------------------------------------------
// The authored TREE: skeleton wood and cluster-card anchors grown by Sapling Tree Gen in
// Blender (`scripts/flora/bake_oak_tree.py`), two rungs from one seed.

static OAK_TREE_NEAR: &[u8] =
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../assets/flora/oak/tree_near.bin"));
static OAK_TREE_MID: &[u8] =
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../assets/flora/oak/tree_mid.bin"));

/// The golden hashes of the oak's two rung files (FNV over the bytes). A re-export changes the
/// tree: bless deliberately and say what changed about the SHAPE.
pub const OAK_TREE_GOLDENS: [(TreeLod, u64); 2] =
    [(TreeLod::Close, 0xb1ec_a836_1182_26fa), (TreeLod::Mid, 0x3cde_941e_4fa3_ec4e)];

/// The shade lane of an authored deck: rim cards at 1.0, core cards down to this — the same
/// one-mass law the procedural dealer applies (`leaves::CORE_SHADE`).
const CORE_SHADE: f32 = 0.68;

/// The authored tree of a species at a rung, if the species has one. ONE individual was
/// grown; the seed buys the only variety a file can give cheaply — odd seeds mirror it
/// across X (winding flipped with it), so a shelterbelt of authored oaks is two trees under
/// their own yaws and scales, not one tree stamped. The ladder ships the unmirrored one.
pub fn tree(species: TreeSpecies, seed: u64, lod: TreeLod) -> Option<BakedTree> {
    let bytes = tree_bytes(species, lod)?;
    let mut tree = parse_tree(species, bytes);
    if seed & 1 == 1 {
        mirror_authored_tree_across_x(&mut tree);
    }
    Some(tree)
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

fn tree_bytes(species: TreeSpecies, lod: TreeLod) -> Option<&'static [u8]> {
    match (species, lod) {
        (TreeSpecies::Oak, TreeLod::Close) => Some(OAK_TREE_NEAR),
        (TreeSpecies::Oak, TreeLod::Mid) => Some(OAK_TREE_MID),
        _ => None,
    }
}

/// FNV over a rung file's bytes.
pub fn tree_file_hash(species: TreeSpecies, lod: TreeLod) -> Option<u64> {
    let bytes = tree_bytes(species, lod)?;
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes {
        crate::fnv(&mut hash, u64::from(*byte));
    }
    Some(hash)
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
    // The shade lane from the deck's own geometry: rim 1.0, core CORE_SHADE.
    let centroid = raw.iter().map(|c| c.0).sum::<Vec3>() / (ncards.max(1) as f32);
    let reach = raw.iter().map(|c| c.0.distance(centroid)).fold(0.01_f32, f32::max);
    let leaves = raw
        .into_iter()
        .map(|(center, half_right, half_up, normal, sprite)| LeafCard {
            center,
            half_right,
            half_up,
            normal: normal.normalize_or_zero(),
            slot: super::leaf_atlas::CLUSTER_SLOT_BASE + sprite % (CLUSTER_SPRITES as u8),
            shade: CORE_SHADE
                + (1.0 - CORE_SHADE) * (center.distance(centroid) / reach).clamp(0.0, 1.0),
        })
        .collect();
    BakedTree {
        species,
        trunk: GeometryMesh::new(vertices, indices),
        // Tall trees keep an empty occlusion hull — their crowns honestly show sky.
        canopy: GeometryMesh::new(Vec::new(), Vec::new()),
        leaves,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The authored oak: both rung files on their goldens, the wood under the ladder's budget,
    /// the cards a cross-pair deck on the cluster block, the tree mature and grounded.
    #[test]
    fn the_authored_oak_is_on_its_goldens_and_inside_its_budget() {
        for (lod, golden) in OAK_TREE_GOLDENS {
            let hash = tree_file_hash(TreeSpecies::Oak, lod).expect("oak rung file");
            assert_eq!(hash, golden, "oak {lod:?}: the tree file changed — bless (0x{hash:016x})");
        }
        let near = tree(TreeSpecies::Oak, 0, TreeLod::Close).expect("authored oak");
        let mid = tree(TreeSpecies::Oak, 0, TreeLod::Mid).expect("authored oak");
        // Measured 3,128 at the dense export (limbs at eight sides + 57 twigs at four).
        assert!(near.trunk.triangle_count() <= 3_500, "near wood {}", near.trunk.triangle_count());
        assert!(mid.trunk.triangle_count() <= 700, "mid wood {}", mid.trunk.triangle_count());
        assert!(mid.trunk.triangle_count() < near.trunk.triangle_count());
        assert!((240..=520).contains(&near.leaves.len()), "near deck {}", near.leaves.len());
        assert!(mid.leaves.len() < near.leaves.len() && mid.leaves.len() >= 120);
        assert!(near.leaves.len().is_multiple_of(2), "cross pairs come in twos");
        assert!(near.tip() > 15.0, "the oak stays mature: {}", near.tip());
        assert!((near.tip() - mid.tip()).abs() < 1.0, "the rungs agree in height");
        let low = near.trunk.vertices().iter().map(|v| v.position.y).fold(f32::MAX, f32::min);
        assert!(low.abs() < 0.2, "grounded at y = 0: {low}");
        for card in &near.leaves {
            assert!(card.slot >= super::super::leaf_atlas::CLUSTER_SLOT_BASE);
            assert!((0.68..=1.0).contains(&card.shade));
            assert!(card.half_right.length() > 0.5 && card.half_up.length() > 0.5);
            // A cross pair: the two quads through one centre stand perpendicular.
        }
        for pair in near.leaves.chunks_exact(2) {
            assert_eq!(pair[0].center, pair[1].center);
            assert!(pair[0].normal.dot(pair[1].normal).abs() < 0.05, "perpendicular cross");
        }
        assert!(
            near.trunk.validate_quality(vehicle_geometry::OPEN_OR_CLOSED_MESH).is_ok(),
            "the authored wood is a valid mesh"
        );
        // Seed 1 is the mirror: same tip, same counts, a different individual, still valid.
        let mirrored = tree(TreeSpecies::Oak, 1, TreeLod::Close).expect("authored oak");
        assert_eq!(mirrored.leaves.len(), near.leaves.len());
        assert!((mirrored.tip() - near.tip()).abs() < 1.0e-3);
        assert_ne!(mirrored.leaves[0].center, near.leaves[0].center);
        assert!(
            mirrored.trunk.validate_quality(vehicle_geometry::OPEN_OR_CLOSED_MESH).is_ok(),
            "the mirrored wood keeps its winding"
        );
    }

    /// The bark pair: on its golden, a 1:2 tile (1 m × 2 m), the normal page a real normal
    /// map (mostly pointing out of the tile) and the albedo a bark, not a white.
    #[test]
    fn the_bark_pair_is_on_its_golden_and_is_a_bark() {
        let (albedo, normal) = bark_pages();
        assert_eq!(albedo.height, albedo.width * 2, "a 1 m × 2 m tile");
        assert_eq!(
            bark_hash(),
            BARK_GOLDEN,
            "the bark pair changed — bless (0x{:016x})",
            bark_hash()
        );
        let mean = |page: &BarkPage, channel: usize| {
            page.rgba.chunks_exact(4).map(|t| u32::from(t[channel])).sum::<u32>() as f32
                / (page.width * page.height) as f32
        };
        assert!(mean(normal, 2) > 200.0, "normals point out of the tile: {}", mean(normal, 2));
        assert!(
            (100.0..=132.0).contains(&mean(normal, 0))
                && (100.0..=132.0).contains(&mean(normal, 1))
        );
        let (r, g, b) = (mean(albedo, 0), mean(albedo, 1), mean(albedo, 2));
        assert!(r < 160.0 && r > b, "bark is a brown, not a white or a blue: ({r}, {g}, {b})");
    }

    /// The asset's identity: the shipped oak pages are the ones blessed, byte for byte.
    #[test]
    fn the_oak_cluster_pages_are_on_their_golden() {
        let pages = oak_clusters();
        assert_eq!((pages.width, pages.height), (CLUSTER_PAGE_W, CLUSTER_PAGE_H));
        assert_eq!(
            pages.deterministic_hash(),
            OAK_CLUSTERS_GOLDEN,
            "the oak's cluster pages changed — bless deliberately (0x{:016x})",
            pages.deterministic_hash()
        );
    }

    /// Every sprite is a real but sparse cluster: leaves cover a band of its slot, never a
    /// pancake and never a bare twig — the band the SDF slots were held to, widened for
    /// authored clusters that carry their own depth.
    #[test]
    fn every_oak_sprite_is_a_real_but_sparse_cluster() {
        let pages = oak_clusters();
        for sprite in 0..CLUSTER_SPRITES {
            let coverage = pages.sprite_coverage(sprite);
            assert!(
                (0.12..=0.70).contains(&coverage),
                "oak sprite {sprite}: coverage {coverage:.3}"
            );
        }
    }

    /// The card's stem hangs at −half_up (the bottom of the slot): every sprite has its twig
    /// rooted in the bottom band, centred, so a card never floats its cluster off its twig.
    #[test]
    fn every_oak_sprite_is_rooted_at_the_bottom_centre() {
        let pages = oak_clusters();
        for sprite in 0..CLUSTER_SPRITES {
            let (x0, y0) = sprite_origin(sprite);
            let mut rooted = false;
            for y in (y0 + CLUSTER_SPRITE_PX - 40)..(y0 + CLUSTER_SPRITE_PX) {
                for x in (x0 + CLUSTER_SPRITE_PX / 2 - 48)..(x0 + CLUSTER_SPRITE_PX / 2 + 48) {
                    rooted |= pages.color[((y * pages.width + x) * 4 + 3) as usize] >= 128;
                }
            }
            assert!(rooted, "oak sprite {sprite} is not rooted at its bottom centre");
        }
    }

    /// The normal page is flat wherever the colour page is cut, and a real normal where it
    /// is not — the dome convention the SDF slots follow, so the mips stay sane.
    #[test]
    fn the_normal_page_is_flat_outside_the_leaves_and_bent_inside() {
        let pages = oak_clusters();
        let mut inside = 0u32;
        let mut bent = 0u32;
        for (c, n) in pages.color.chunks_exact(4).zip(pages.normal.chunks_exact(4)) {
            if c[3] < 8 {
                assert_eq!(&n[..3], &[128, 128, 255], "cut texel must be flat");
            } else if c[3] >= 128 {
                inside += 1;
                bent +=
                    u32::from((i32::from(n[0]) - 128).abs() + (i32::from(n[1]) - 128).abs() > 12);
            }
            assert_eq!(n[3], 255);
        }
        assert!(inside > 0);
        assert!(bent * 100 >= inside * 20, "the leaves carry real normals: {bent}/{inside}");
    }
}
