//! The procedural leaf atlas (Drzewa 3.0 PR5): every mask a leaf card will ever cut is BAKED
//! here on the CPU, deterministically, from 2-D SDF composition — no imported texture may
//! exist under map-forge policy #10, and none is needed. One page of color·alpha and one page
//! of tangent-space dome normals, 2048×1024: a 4×4 grid of 128 px leaf slots in the top-left
//! square, and EVERY species' impostor sprite pair in the rest (splatted by the paint side —
//! PR10 grew the battlefield oak's pair, Inny Poziom F1 the other five species', so the
//! backdrop ring past the red line can stand on the same sprites the ladder's far rung uses).
//!
//! Slot 0 is LOAD-BEARING WHITE: every procedural vertex in the world carries uv (0,0), and
//! the moment this atlas replaces the renderer's 1×1 white no-op texel, texel (0,0) must stay
//! opaque white or every untextured surface in the game tints and discards. The lock below is
//! the whole world's no-op contract.
//!
//! Alpha discipline: interiors solid, a ~1.5 px antialiased rim, nothing else — the histogram
//! stays bimodal so the shaders' 0.5 cutout cuts a shape, not a fog. The coverage-preserving
//! mip build (PR2) rides on exactly this.

use glam::Vec2;

use super::TreeSpecies;
use crate::shape::Rng;

/// Atlas page width, texels. The leaf-slot grid fills the top-left [`LEAF_GRID_PX`] square;
/// the impostor sprites fill the rest of the first row and the whole second row — one
/// binding serves both.
pub const ATLAS_WIDTH: u32 = 4_096;
/// Atlas page height, texels. The top 1024 rows are the procedural page (leaf grid and the
/// impostor rows); below them the AUTHORED cluster blocks (route 2, 2026-09-02): one
/// 2048×512 row of four cluster sprites PER SPECIES, in `TreeSpecies::ALL` order, pasted
/// from `authored::clusters`.
pub const ATLAS_HEIGHT: u32 = 4_096;
/// Where the authored cluster blocks start (rows), and their slot geometry.
pub const CLUSTER_BLOCK_Y: u32 = 2_048;
pub const CLUSTER_SLOT_PX: u32 = super::authored::CLUSTER_SPRITE_PX;
/// The first cluster slot number: slots below it index the procedural leaf grid. A species'
/// slots are `cluster_slot_base(species) + sprite`.
pub const CLUSTER_SLOT_BASE: u8 = (ATLAS_GRID * ATLAS_GRID) as u8;
// Two species' cluster rows share one 4096 px row of the page.
const _: () = assert!(
    CLUSTER_BLOCK_Y + TreeSpecies::ALL.len().div_ceil(2) as u32 * super::authored::CLUSTER_PAGE_H
        <= ATLAS_HEIGHT
        && super::authored::CLUSTER_PAGE_W * 2 <= ATLAS_WIDTH
);
// The cluster block sits under the impostor rows, never inside them.
const _: () = assert!(IMPOSTOR_SPRITE_H * IMPOSTOR_ROWS <= CLUSTER_BLOCK_Y);
/// The leaf-slot grid's edge, texels (the top-left square).
pub const LEAF_GRID_PX: u32 = 512;
/// Slots per grid edge.
pub const ATLAS_GRID: u32 = 4;
/// One slot's edge, texels.
pub const SLOT_SIZE: u32 = LEAF_GRID_PX / ATLAS_GRID;
/// UV inset from a slot's edge so bilinear + aniso-8 sampling never reads the neighbour.
const SLOT_MARGIN_PX: f32 = 6.0;

/// One impostor sprite's region, texels: two azimuths per species. 512×1024 since the
/// impostor is RENDERED from the authored tree (2026-09-03, `scripts/flora/bake_impostor.py`)
/// instead of splatted on the CPU from its deck.
pub const IMPOSTOR_SPRITE_W: u32 = super::authored::IMPOSTOR_VIEW_W;
pub const IMPOSTOR_SPRITE_H: u32 = super::authored::IMPOSTOR_VIEW_H;
/// Sprites per row: the first row starts past the leaf grid, the others are whole; the rows
/// stack from the top of the page down to the cluster block.
const IMPOSTOR_ROW0_SPRITES: u32 = (ATLAS_WIDTH - LEAF_GRID_PX) / IMPOSTOR_SPRITE_W;
const IMPOSTOR_ROW_SPRITES: u32 = ATLAS_WIDTH / IMPOSTOR_SPRITE_W;
const IMPOSTOR_ROWS: u32 = CLUSTER_BLOCK_Y / IMPOSTOR_SPRITE_H;
/// Sprites per species: one per variant per azimuth.
pub const IMPOSTOR_SPRITES_PER_SPECIES: u32 = super::authored::VARIANTS * 2;
// Every species' variants' two azimuths fit the rows, by construction.
const _: () = assert!(
    (TreeSpecies::ALL.len() as u32) * IMPOSTOR_SPRITES_PER_SPECIES
        <= IMPOSTOR_ROW0_SPRITES + (IMPOSTOR_ROWS - 1) * IMPOSTOR_ROW_SPRITES
);
// The first row's sprites stand beside the leaf grid, never inside it.
const _: () = assert!(IMPOSTOR_SPRITE_H >= LEAF_GRID_PX);

/// Where azimuth `which` (0 or 1 — 0° and 90°) of `variant` of `species`' impostor lives,
/// as texel origin. The paint side pastes the sprite here; the quads sample it.
pub fn impostor_origin(species: TreeSpecies, variant: u32, which: u32) -> (u32, u32) {
    let index = super::authored::species_index(species) * IMPOSTOR_SPRITES_PER_SPECIES
        + (variant % super::authored::VARIANTS) * 2
        + which.min(1);
    if index < IMPOSTOR_ROW0_SPRITES {
        (LEAF_GRID_PX + index * IMPOSTOR_SPRITE_W, 0)
    } else {
        let rest = index - IMPOSTOR_ROW0_SPRITES;
        (
            (rest % IMPOSTOR_ROW_SPRITES) * IMPOSTOR_SPRITE_W,
            (1 + rest / IMPOSTOR_ROW_SPRITES) * IMPOSTOR_SPRITE_H,
        )
    }
}

/// The impostor sampling rectangle as `[u0, v0, u1, v1]`, inset by the bleed margin.
pub fn impostor_rect(species: TreeSpecies, variant: u32, which: u32) -> [f32; 4] {
    let (x, y) = impostor_origin(species, variant, which);
    [
        (x as f32 + SLOT_MARGIN_PX) / ATLAS_WIDTH as f32,
        (y as f32 + SLOT_MARGIN_PX) / ATLAS_HEIGHT as f32,
        ((x + IMPOSTOR_SPRITE_W) as f32 - SLOT_MARGIN_PX) / ATLAS_WIDTH as f32,
        ((y + IMPOSTOR_SPRITE_H) as f32 - SLOT_MARGIN_PX) / ATLAS_HEIGHT as f32,
    ]
}

/// The reserved no-op slot: opaque white, the whole world's uv (0,0).
pub const SLOT_WHITE: u8 = 0;

/// The review gate for the atlas bytes (bless deliberately; covers BOTH pages).
/// Re-blessed 2026-09-01 (Inny Poziom F1): the page grew 1024×512 → 2048×1024 to hold every
/// species' impostor sprite pair; the leaf slots themselves are byte-identical (same grid,
/// same painter, same seeds — only the page around them changed). Re-blessed 2026-09-02
/// (route 2): the page grew again, 2048×1024 → 2048×2048, and the bottom half carries the
/// oak's authored cluster block (`authored::OAK_CLUSTERS_GOLDEN` is that block's own lock).
pub const LEAF_ATLAS_GOLDEN: u64 = 0xfae1_904b_0326_8547;

/// The first cluster slot of a species' block.
pub fn cluster_slot_base(species: TreeSpecies) -> u8 {
    CLUSTER_SLOT_BASE
        + (super::authored::species_index(species) * super::authored::CLUSTER_SPRITES) as u8
}

/// The slots a species' cards are dealt from: its authored cluster block (route 2), else its
/// two procedural mask variants.
pub fn card_slots(species: TreeSpecies) -> Vec<u8> {
    match super::authored::clusters(species) {
        Some(_) => (0..super::authored::CLUSTER_SPRITES as u8)
            .map(|sprite| cluster_slot_base(species) + sprite)
            .collect(),
        None => species_slots(species).to_vec(),
    }
}

/// Where a cluster slot's sprite sits in the page, texels — `None` for a leaf-grid slot.
pub fn cluster_slot_origin(slot: u8) -> Option<(u32, u32)> {
    let global = u32::from(slot.checked_sub(CLUSTER_SLOT_BASE)?);
    let species = global / super::authored::CLUSTER_SPRITES;
    if species >= TreeSpecies::ALL.len() as u32 {
        return None;
    }
    let (x, y) = super::authored::sprite_origin(global % super::authored::CLUSTER_SPRITES);
    let (block_x, block_y) = cluster_block_origin(species);
    Some((block_x + x, block_y + y))
}

/// Where a species' cluster block starts: two blocks per page row, under the impostor rows.
pub fn cluster_block_origin(species_index: u32) -> (u32, u32) {
    (
        (species_index % 2) * super::authored::CLUSTER_PAGE_W,
        CLUSTER_BLOCK_Y + (species_index / 2) * super::authored::CLUSTER_PAGE_H,
    )
}

/// The two procedural mask variants a species owns (cards mix them per anchor).
pub fn species_slots(species: TreeSpecies) -> [u8; 2] {
    match species {
        TreeSpecies::Oak => [1, 2],
        TreeSpecies::Poplar => [3, 4],
        TreeSpecies::Willow => [5, 6],
        TreeSpecies::FruitTree => [7, 8],
        TreeSpecies::Bush => [9, 10],
        TreeSpecies::Pine => [11, 12],
    }
}

/// A slot's sampling rectangle as `[u0, v0, u1, v1]`, inset by the bleed margin — a leaf-grid
/// slot or an authored cluster slot, whichever the number names.
pub fn atlas_rect(slot: u8) -> [f32; 4] {
    let ((x, y), size) = match cluster_slot_origin(slot) {
        Some(origin) => (origin, CLUSTER_SLOT_PX),
        None => (slot_origin(slot), SLOT_SIZE),
    };
    let (width, height) = (ATLAS_WIDTH as f32, ATLAS_HEIGHT as f32);
    [
        (x as f32 + SLOT_MARGIN_PX) / width,
        (y as f32 + SLOT_MARGIN_PX) / height,
        ((x + size) as f32 - SLOT_MARGIN_PX) / width,
        ((y + size) as f32 - SLOT_MARGIN_PX) / height,
    ]
}

/// Both baked pages, tightly packed RGBA8. `rgba` is color·alpha (the cutout mask), `normal`
/// is the tangent-space dome page (flat texels encode (128, 128, 255)).
#[derive(Debug, Clone)]
pub struct LeafAtlasImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub normal: Vec<u8>,
}

impl LeafAtlasImage {
    pub fn deterministic_hash(&self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        for byte in self.rgba.iter().chain(self.normal.iter()) {
            super::super::fnv(&mut hash, u64::from(*byte));
        }
        hash
    }
}

/// Bake the whole atlas. No inputs: the atlas is a single shared asset, one per build, and
/// its identity is the golden hash above.
pub fn bake_leaf_atlas() -> LeafAtlasImage {
    let texels = (ATLAS_WIDTH * ATLAS_HEIGHT) as usize;
    let mut rgba = vec![0u8; texels * 4];
    let mut normal = Vec::with_capacity(texels * 4);
    for _ in 0..texels {
        normal.extend_from_slice(&[128, 128, 255, 255]);
    }

    paint_solid_white(&mut rgba, SLOT_WHITE);
    for species in TreeSpecies::ALL {
        for (variant, slot) in species_slots(species).into_iter().enumerate() {
            paint_cluster(&mut rgba, &mut normal, slot, species, variant as u64);
        }
    }
    paste_authored_clusters(&mut rgba, &mut normal);
    paste_authored_impostors(&mut rgba, &mut normal);
    LeafAtlasImage { width: ATLAS_WIDTH, height: ATLAS_HEIGHT, rgba, normal }
}

/// Paste every species' authored cluster block (route 2) under the procedural page, one
/// 2048×512 row per species in `TreeSpecies::ALL` order.
fn paste_authored_clusters(rgba: &mut [u8], normal: &mut [u8]) {
    for species in TreeSpecies::ALL {
        let Some(pages) = super::authored::clusters(species) else {
            continue;
        };
        let (block_x, block_y) = cluster_block_origin(super::authored::species_index(species));
        for y in 0..pages.height {
            let src = (y * pages.width * 4) as usize;
            let dst = (((block_y + y) * ATLAS_WIDTH + block_x) * 4) as usize;
            let row = (pages.width * 4) as usize;
            rgba[dst..dst + row].copy_from_slice(&pages.color[src..src + row]);
            normal[dst..dst + row].copy_from_slice(&pages.normal[src..src + row]);
        }
    }
}

/// Paste every species' RENDERED impostor pair (route 2, LOD honesty): view `which` of the
/// 2048×512 page (eight 256×512 views: variant × 2 + azimuth) into `impostor_origin`.
fn paste_authored_impostors(rgba: &mut [u8], normal: &mut [u8]) {
    for species in TreeSpecies::ALL {
        let Some(pages) = super::authored::impostor_pages(species) else {
            continue;
        };
        for slot in 0..IMPOSTOR_SPRITES_PER_SPECIES {
            let (x0, y0) = impostor_origin(species, slot / 2, slot % 2);
            for y in 0..IMPOSTOR_SPRITE_H {
                let src = ((y * pages.width + slot * IMPOSTOR_SPRITE_W) * 4) as usize;
                let dst = (((y0 + y) * ATLAS_WIDTH + x0) * 4) as usize;
                let row = (IMPOSTOR_SPRITE_W * 4) as usize;
                rgba[dst..dst + row].copy_from_slice(&pages.color[src..src + row]);
                normal[dst..dst + row].copy_from_slice(&pages.normal[src..src + row]);
            }
        }
    }
}

fn slot_origin(slot: u8) -> (u32, u32) {
    ((slot as u32 % ATLAS_GRID) * SLOT_SIZE, (slot as u32 / ATLAS_GRID) * SLOT_SIZE)
}

fn paint_solid_white(rgba: &mut [u8], slot: u8) {
    let (x0, y0) = slot_origin(slot);
    for y in y0..y0 + SLOT_SIZE {
        for x in x0..x0 + SLOT_SIZE {
            let i = ((y * ATLAS_WIDTH + x) * 4) as usize;
            rgba[i..i + 4].copy_from_slice(&[255, 255, 255, 255]);
        }
    }
}

/// One leaf (or needle-frond) stamp: local frame with the axis from `base` toward `base +
/// dir·len`, half-width from the species profile.
struct Stamp {
    base: Vec2,
    dir: Vec2,
    len: f32,
    scale: f32,
}

/// Signed distance from `p` to the stamp's silhouette, plus the along-axis fraction (for the
/// chlorophyll gradient and the veins). Positive = outside.
fn stamp_distance(stamp: &Stamp, species: TreeSpecies, p: Vec2) -> (f32, f32) {
    let rel = p - stamp.base;
    let along = rel.dot(stamp.dir);
    let lateral = (rel - stamp.dir * along).length();
    let t = (along / stamp.len).clamp(0.0, 1.0);
    let width = half_width(species, t) * stamp.len * stamp.scale.max(0.4);
    let capped_along = if along < 0.0 {
        -along
    } else if along > stamp.len {
        along - stamp.len
    } else {
        0.0
    };
    ((lateral - width).max(capped_along - width * 0.3), t)
}

/// The species silhouette: half-width as a fraction of the stamp length, at `t` along the
/// midrib. THIS is where an oak stops being a poplar.
fn half_width(species: TreeSpecies, t: f32) -> f32 {
    let bell = (std::f32::consts::PI * t).sin();
    match species {
        // Lobed: a bell with 4–5 lobes rippling the boundary — NARROW enough that a spray of
        // blades keeps sky between them; a cluster whose blades merge into a pancake turns
        // every card into a green rectangle up close.
        TreeSpecies::Oak => 0.22 * bell.max(0.12) * (1.0 + 0.30 * (t * 14.5).sin()),
        // Deltoid with a fine sawtooth serration, tapering to the drip tip.
        TreeSpecies::Poplar => 0.34 * (1.0 - t).powf(1.05) * (1.0 + 0.07 * (t * 62.0).sin()),
        // Lanceolate, aspect well past 3 — the streamer the curtain cards need.
        TreeSpecies::Willow => 0.11 * bell.powf(0.7),
        // Serrated oval, orchard-small.
        TreeSpecies::FruitTree => {
            0.27 * (std::f32::consts::PI * t.powf(0.9)).sin() * (1.0 + 0.05 * (t * 44.0).sin())
        }
        // Plain ovate.
        TreeSpecies::Bush => 0.30 * bell,
        // The frond reads through needles, not a blade — the profile is only the fallback
        // stem; `paint_cluster` draws pine needles as line strokes instead.
        TreeSpecies::Pine => 0.03,
    }
}

/// Where a channel value lands after the chlorophyll gradient, venation and per-leaf jitter.
/// The page MULTIPLIES the authored vertex palette in the shader, so everything stays near
/// white — modulation, never a second palette (the material-synthesis lesson: amplitudes
/// small enough to read as life, never as noise).
fn leaf_color(t: f32, vein_darken: f32, jitter: f32) -> [u8; 3] {
    let value = (198.0 + 48.0 * t + jitter * 14.0) * (1.0 - vein_darken);
    let v = value.clamp(0.0, 255.0);
    [(v * 0.93) as u8, v as u8, (v * 0.88) as u8]
}

fn paint_cluster(rgba: &mut [u8], normal: &mut [u8], slot: u8, species: TreeSpecies, variant: u64) {
    let (x0, y0) = slot_origin(slot);
    let mut rng = Rng(0x1EAF_0000 ^ ((slot as u64) << 8) ^ variant);
    let size = SLOT_SIZE as f32;
    let center = Vec2::new(size * 0.5, size * 0.5);

    // The cluster stem — its ORIENTATION is species anatomy, not decoration: a broadleaf
    // spray grows up, a willow curtain HANGS, a pine frond is a horizontal branch sprig.
    // Distinct orientations are also what keeps the masks mutually distinct as bitmaps.
    let (stem_base, stem_tip) = match species {
        TreeSpecies::Willow => (
            Vec2::new(size * (0.42 + rng.unit() * 0.16), size * 0.10),
            Vec2::new(size * (0.40 + rng.unit() * 0.20), size * 0.90),
        ),
        TreeSpecies::Pine => (
            Vec2::new(size * 0.08, size * (0.42 + rng.unit() * 0.16)),
            Vec2::new(size * 0.92, size * (0.40 + rng.unit() * 0.20)),
        ),
        // The bush is a knee-high tuft: a short stem fanning wide from low in the slot —
        // its mask lives in the lower half where a poplar column runs the full height.
        TreeSpecies::Bush => (
            Vec2::new(size * (0.42 + rng.unit() * 0.16), size * 0.92),
            Vec2::new(size * (0.40 + rng.unit() * 0.20), size * 0.48),
        ),
        _ => (
            Vec2::new(size * (0.42 + rng.unit() * 0.16), size * 0.92),
            Vec2::new(size * (0.40 + rng.unit() * 0.20), size * 0.14),
        ),
    };
    let stem_bow = (rng.signed()) * size * 0.10;
    // How far blades reach off the stem: the poplar column keeps its spray tight, the bush
    // tuft fans wide.
    let spread_gain = match species {
        TreeSpecies::Poplar => 0.6,
        TreeSpecies::Bush => 1.5,
        TreeSpecies::Oak => 1.3,
        _ => 1.0,
    };

    // Leaf stamps along the stem, golden-angle alternation, scale falling toward the tip.
    // Count and blade length are SPECIES layout, not luck: an oak spray is a few broad
    // blades, a poplar spray many small deltoids, a willow curtain a sheaf of streamers —
    // the cluster reads as the species before a single boundary lobe does.
    let stamps: Vec<(Stamp, f32)> = {
        let (count, len_base) = match species {
            // Oak blades stay clear of the slot edges: a mask that fills its tile turns every
            // card into a green RECTANGLE up close — the silhouette IS the product.
            TreeSpecies::Oak => (5 + (rng.next() % 2) as u32, 0.34),
            TreeSpecies::Poplar => (6 + (rng.next() % 2) as u32, 0.24),
            TreeSpecies::Willow => (10, 0.34),
            TreeSpecies::FruitTree => (6, 0.28),
            TreeSpecies::Bush => (8 + (rng.next() % 2) as u32, 0.26),
            TreeSpecies::Pine => (0, 0.0),
        };
        (0..count)
            .map(|ordinal| {
                let along = 0.12 + 0.78 * (ordinal as f32 + rng.unit() * 0.5) / count.max(1) as f32;
                let on_stem = stem_point(stem_base, stem_tip, stem_bow, along);
                let side = if ordinal % 2 == 0 { 1.0 } else { -1.0 };
                let spread =
                    0.55 + 0.5 * (ordinal as f32 * super::skeleton::GOLDEN_ANGLE_RAD).sin();
                let stem_dir = (stem_tip - stem_base).normalize_or_zero();
                let out = Vec2::new(-stem_dir.y, stem_dir.x) * side;
                let dir = (stem_dir * (0.35 + 0.3 * rng.unit()) + out * spread * spread_gain)
                    .normalize_or_zero();
                let len = size * (len_base + 0.10 * rng.unit()) * (1.0 - 0.35 * along);
                let jitter = rng.signed();
                (Stamp { base: on_stem, dir, len, scale: 1.0 }, jitter)
            })
            .collect()
    };

    // Pine: needle strokes fanning off the stem instead of blade stamps.
    let needles: Vec<(Vec2, Vec2, f32)> = if species == TreeSpecies::Pine {
        (0..42)
            .map(|ordinal| {
                let along = 0.08 + 0.86 * ordinal as f32 / 42.0;
                let root = stem_point(stem_base, stem_tip, stem_bow, along);
                let side = if ordinal % 2 == 0 { 1.0 } else { -1.0 };
                let stem_dir = (stem_tip - stem_base).normalize_or_zero();
                let out = Vec2::new(-stem_dir.y, stem_dir.x) * side;
                let angle = 0.6 + rng.unit() * 0.35;
                let dir = (stem_dir * angle.cos() + out * angle.sin()).normalize_or_zero();
                let len = size * (0.16 + 0.10 * rng.unit()) * (1.0 - 0.3 * along);
                (root, dir, len)
            })
            .collect()
    } else {
        Vec::new()
    };

    let rim_px = 1.5;
    for py in 0..SLOT_SIZE {
        for px in 0..SLOT_SIZE {
            let p = Vec2::new(px as f32 + 0.5, py as f32 + 0.5);
            // Keep the margin band empty so the inset rect owns everything visible.
            let m = SLOT_MARGIN_PX - 1.0;
            if p.x < m || p.y < m || p.x > size - m || p.y > size - m {
                continue;
            }

            // Nearest stamp wins; the stem stroke itself darkens whatever it crosses.
            let mut best: Option<(f32, f32, f32)> = None; // (sd, t, jitter)
            for (stamp, jitter) in &stamps {
                let (sd, t) = stamp_distance(stamp, species, p);
                if best.map(|(b, _, _)| sd < b).unwrap_or(true) {
                    best = Some((sd, t, *jitter));
                }
            }
            for (root, dir, len) in &needles {
                let rel = p - *root;
                let along = rel.dot(*dir).clamp(0.0, *len);
                let sd = (rel - *dir * along).length() - 1.7;
                if best.map(|(b, _, _)| sd < b).unwrap_or(true) {
                    best = Some((sd, along / len, 0.0));
                }
            }
            let stem_sd = stem_stroke_distance(stem_base, stem_tip, stem_bow, p) - 1.6;
            if best.map(|(b, _, _)| stem_sd < b).unwrap_or(true) {
                best = Some((stem_sd, 0.05, 0.0));
            }
            let Some((sd, t, jitter)) = best else { continue };
            // The raw coverage, then a 3x steepening around 0.5: one texel of antialiasing
            // survives, the half-covered fog does not — bimodality is painted in, not hoped
            // for.
            let soft = (0.5 - sd / rim_px).clamp(0.0, 1.0);
            let coverage = ((soft - 0.5) * 3.0 + 0.5).clamp(0.0, 1.0);
            if coverage <= 0.0 {
                continue;
            }

            // Venation: the midrib of the nearest stamp and its pinnate ripple, as darkening.
            let vein = if species == TreeSpecies::Pine {
                0.0
            } else {
                let ripple = ((t * 26.0).sin().abs() > 0.965) as u32 as f32;
                0.07 * ripple
            };
            let color = leaf_color(t, vein, jitter);
            let alpha = (coverage * 255.0) as u8;
            let i = (((y0 + py) * ATLAS_WIDTH + (x0 + px)) * 4) as usize;
            // Painter's order: a later (nearer) stamp simply overwrites — clusters read as
            // one pressed spray, exactly like the pressed-leaf reference they imitate.
            if alpha >= rgba[i + 3] {
                rgba[i] = color[0];
                rgba[i + 1] = color[1];
                rgba[i + 2] = color[2];
                rgba[i + 3] = alpha;
            }

            // The dome normal: the cluster reads as a curved tuft, not a flat decal — the
            // lit card borrows sphere-cap normals around the slot centre, amplitude kept
            // gentle so `foliage_radiance` shades volume without banding.
            let offset = (p - center) / (size * 0.5);
            let nx = (offset.x * 0.55).clamp(-0.9, 0.9);
            let ny = (-offset.y * 0.55).clamp(-0.9, 0.9);
            let nz = (1.0 - nx * nx - ny * ny).max(0.05).sqrt();
            normal[i] = ((nx * 0.5 + 0.5) * 255.0) as u8;
            normal[i + 1] = ((ny * 0.5 + 0.5) * 255.0) as u8;
            normal[i + 2] = ((nz * 0.5 + 0.5) * 255.0) as u8;
        }
    }
}

/// A quadratic-bowed stem point at fraction `t`.
fn stem_point(base: Vec2, tip: Vec2, bow: f32, t: f32) -> Vec2 {
    let straight = base.lerp(tip, t);
    let dir = (tip - base).normalize_or_zero();
    let out = Vec2::new(-dir.y, dir.x);
    straight + out * bow * (std::f32::consts::PI * t).sin()
}

/// Distance from `p` to the stem stroke (sampled — the stroke is short and this bakes once).
fn stem_stroke_distance(base: Vec2, tip: Vec2, bow: f32, p: Vec2) -> f32 {
    let mut best = f32::MAX;
    for step in 0..=72 {
        let t = step as f32 / 72.0;
        best = best.min(p.distance(stem_point(base, tip, bow, t)));
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    fn coverage(image: &LeafAtlasImage, slot: u8) -> f32 {
        let (x0, y0) = slot_origin(slot);
        let mut covered = 0u32;
        for y in y0..y0 + SLOT_SIZE {
            for x in x0..x0 + SLOT_SIZE {
                let a = image.rgba[((y * ATLAS_WIDTH + x) * 4 + 3) as usize];
                covered += u32::from(a >= 128);
            }
        }
        covered as f32 / (SLOT_SIZE * SLOT_SIZE) as f32
    }

    /// The whole-atlas review gate: bytes on both pages, blessed deliberately.
    #[test]
    fn the_atlas_bakes_deterministic_and_on_its_golden() {
        let first = bake_leaf_atlas();
        let second = bake_leaf_atlas();
        assert_eq!(first.deterministic_hash(), second.deterministic_hash());
        assert_eq!(
            first.deterministic_hash(),
            LEAF_ATLAS_GOLDEN,
            "the leaf atlas changed — bless deliberately (0x{:016x})",
            first.deterministic_hash()
        );
    }

    /// THE no-op contract: every procedural vertex in the world samples texel (0,0) at mip 0.
    /// Slot 0 stays bit-exact opaque white to its last texel, or the whole world tints.
    #[test]
    fn slot_zero_is_load_bearing_white() {
        let image = bake_leaf_atlas();
        let (x0, y0) = slot_origin(SLOT_WHITE);
        for y in y0..y0 + SLOT_SIZE {
            for x in x0..x0 + SLOT_SIZE {
                let i = ((y * ATLAS_WIDTH + x) * 4) as usize;
                assert_eq!(
                    &image.rgba[i..i + 4],
                    &[255, 255, 255, 255],
                    "texel ({x}, {y}) broke the world's no-op"
                );
            }
        }
    }

    /// Masks are present but sparse: 12–70% of each species slot above the cutout. Below the
    /// floor the card is glitter; above the ceiling it is a cabbage.
    #[test]
    fn every_species_slot_holds_a_real_but_sparse_mask() {
        let image = bake_leaf_atlas();
        for species in TreeSpecies::ALL {
            for slot in species_slots(species) {
                let c = coverage(&image, slot);
                assert!((0.12..=0.70).contains(&c), "{species:?} slot {slot}: coverage {c:.3}");
            }
        }
    }

    /// The alpha histogram stays bimodal (≥92% of covered-slot texels outside [64, 192]) so
    /// the 0.5 cutout cuts a shape, not a fog — the discipline the coverage-preserving mips
    /// (PR2) are built on.
    #[test]
    fn the_alpha_histogram_stays_bimodal() {
        let image = bake_leaf_atlas();
        let mut total = 0u32;
        let mut crisp = 0u32;
        for species in TreeSpecies::ALL {
            for slot in species_slots(species) {
                let (x0, y0) = slot_origin(slot);
                for y in y0..y0 + SLOT_SIZE {
                    for x in x0..x0 + SLOT_SIZE {
                        let a = image.rgba[((y * ATLAS_WIDTH + x) * 4 + 3) as usize];
                        total += 1;
                        crisp += u32::from(!(64..=192).contains(&a));
                    }
                }
            }
        }
        assert!(crisp * 100 >= total * 92, "the rim went foggy: {crisp}/{total} texels crisp");
    }

    /// Species read differently: any two species' first slots differ on at least 12% of their
    /// alpha texels, and the willow streamer is unmistakably lanceolate (covered bounding box
    /// aspect > 2).
    #[test]
    fn species_masks_are_mutually_distinct_and_the_willow_is_a_streamer() {
        let image = bake_leaf_atlas();
        let alpha_bits = |slot: u8| -> Vec<bool> {
            let (x0, y0) = slot_origin(slot);
            let mut bits = Vec::with_capacity((SLOT_SIZE * SLOT_SIZE) as usize);
            for y in y0..y0 + SLOT_SIZE {
                for x in x0..x0 + SLOT_SIZE {
                    bits.push(image.rgba[((y * ATLAS_WIDTH + x) * 4 + 3) as usize] >= 128);
                }
            }
            bits
        };
        for a in TreeSpecies::ALL {
            for b in TreeSpecies::ALL {
                if a as u64 >= b as u64 {
                    continue;
                }
                let (bits_a, bits_b) =
                    (alpha_bits(species_slots(a)[0]), alpha_bits(species_slots(b)[0]));
                let differing = bits_a.iter().zip(&bits_b).filter(|(x, y)| x != y).count();
                assert!(
                    differing * 100 >= bits_a.len() * 12,
                    "{a:?} and {b:?} share a silhouette: {differing} differing texels"
                );
            }
        }
        // The willow: its single stamps are lanceolate (aspect > 3), and even the whole
        // curtain cluster stays visibly elongated.
        let widest = |t: f32| half_width(TreeSpecies::Willow, t);
        let peak = (0..=20).map(|i| widest(i as f32 / 20.0)).fold(0.0_f32, f32::max);
        assert!(
            peak * 2.0 < 1.0 / 3.0,
            "a willow leaf is a streamer: aspect {}",
            1.0 / (peak * 2.0)
        );
    }

    /// The margin band is empty on every authored slot — the inset `atlas_rect` owns all the
    /// visible texels, so aniso-8 sampling at the rect edge never drags the neighbour in.
    #[test]
    fn the_bleed_margin_stays_empty() {
        let image = bake_leaf_atlas();
        for species in TreeSpecies::ALL {
            for slot in species_slots(species) {
                let (x0, y0) = slot_origin(slot);
                for edge in 0..SLOT_SIZE {
                    for (x, y) in [
                        (x0 + edge, y0),
                        (x0 + edge, y0 + SLOT_SIZE - 1),
                        (x0, y0 + edge),
                        (x0 + SLOT_SIZE - 1, y0 + edge),
                    ] {
                        let a = image.rgba[((y * ATLAS_WIDTH + x) * 4 + 3) as usize];
                        assert_eq!(a, 0, "slot {slot} painted into its bleed margin at ({x}, {y})");
                    }
                }
            }
        }
    }

    /// The normal page: flat where nothing is painted, a real dome where a mask is — and the
    /// rect table stays inside its slot.
    #[test]
    fn the_normal_page_domes_the_masks_and_rects_stay_inset() {
        let image = bake_leaf_atlas();
        let flat = image
            .normal
            .chunks_exact(4)
            .filter(|texel| texel[0] == 128 && texel[1] == 128 && texel[2] == 255)
            .count();
        assert!(
            flat * 100 >= (ATLAS_WIDTH * ATLAS_HEIGHT) as usize * 50,
            "most of the page is flat (unpainted): {flat}"
        );
        assert!(flat < (ATLAS_WIDTH * ATLAS_HEIGHT) as usize, "painted texels carry dome normals");
        for slot in 0..(ATLAS_GRID * ATLAS_GRID) as u8 {
            let [u0, v0, u1, v1] = atlas_rect(slot);
            assert!(u0 < u1 && v0 < v1);
            assert!(u0 >= 0.0 && v0 >= 0.0 && u1 <= 1.0 && v1 <= 1.0);
        }
    }

    /// Every species' sprite pair has its own texels: no two sprites overlap, none overlaps
    /// the leaf grid, and every rect stays inside the page. The layout is arithmetic, so the
    /// lock walks every pair rather than trusting the row constants.
    /// Route 2's blocks: every species' four cluster slots lie in its own row under both
    /// impostor rows, inside the page, disjoint from every other species', with inset rects;
    /// every species deals its cards from its block, and the block is really pasted.
    #[test]
    fn every_species_cluster_block_sits_under_the_procedural_page() {
        let sprites = super::super::authored::CLUSTER_SPRITES as u8;
        let mut seen = std::collections::BTreeSet::new();
        let image = bake_leaf_atlas();
        for species in super::super::authored::authored_species() {
            let slots = card_slots(species);
            assert_eq!(slots.len(), sprites as usize, "{species:?}");
            assert_eq!(slots[0], cluster_slot_base(species));
            for slot in slots {
                let (x, y) = cluster_slot_origin(slot).expect("a cluster slot");
                assert!(y >= CLUSTER_BLOCK_Y && y >= IMPOSTOR_SPRITE_H * IMPOSTOR_ROWS);
                assert!(x + CLUSTER_SLOT_PX <= ATLAS_WIDTH && y + CLUSTER_SLOT_PX <= ATLAS_HEIGHT);
                assert!(seen.insert((x, y)), "{species:?} slot {slot} shares a sprite");
                let [u0, v0, u1, v1] = atlas_rect(slot);
                assert!(u0 < u1 && v0 < v1 && u0 >= 0.0 && v0 >= 0.25 && u1 <= 1.0 && v1 <= 1.0);
            }
            let pages = super::super::authored::clusters(species).expect("clusters");
            let (x, y) = cluster_slot_origin(cluster_slot_base(species)).expect("slot");
            let i = ((y * ATLAS_WIDTH + x) * 4) as usize;
            assert_eq!(&image.rgba[i..i + 4], &pages.color[..4], "{species:?} pasted");
        }
        assert!(cluster_slot_origin(SLOT_WHITE).is_none());
        assert!(
            cluster_slot_origin(CLUSTER_SLOT_BASE + sprites * TreeSpecies::ALL.len() as u8)
                .is_none()
        );
    }

    #[test]
    fn every_variant_owns_two_disjoint_impostor_sprites_outside_the_leaf_grid() {
        let mut regions: Vec<(TreeSpecies, u32, u32, u32, u32)> = Vec::new();
        for species in TreeSpecies::ALL {
            for variant in 0..super::super::authored::VARIANTS {
                for which in 0..2u32 {
                    let (x, y) = impostor_origin(species, variant, which);
                    assert!(
                        x + IMPOSTOR_SPRITE_W <= ATLAS_WIDTH
                            && y + IMPOSTOR_SPRITE_H <= CLUSTER_BLOCK_Y
                    );
                    assert!(
                        x >= LEAF_GRID_PX || y >= LEAF_GRID_PX,
                        "{species:?} v{variant}/{which}: sprite at ({x}, {y}) sits in the leaf grid"
                    );
                    let [u0, v0, u1, v1] = impostor_rect(species, variant, which);
                    assert!(u0 < u1 && v0 < v1 && u0 >= 0.0 && v0 >= 0.0 && u1 <= 1.0 && v1 <= 1.0);
                    regions.push((species, variant, which, x, y));
                }
            }
        }
        for (i, a) in regions.iter().enumerate() {
            for b in &regions[i + 1..] {
                let apart =
                    a.3 + IMPOSTOR_SPRITE_W <= b.3 || b.3 + IMPOSTOR_SPRITE_W <= a.3 || a.4 != b.4;
                assert!(
                    apart,
                    "{:?} v{}/{} and {:?} v{}/{} share texels",
                    a.0, a.1, a.2, b.0, b.1, b.2
                );
            }
        }
    }
}
