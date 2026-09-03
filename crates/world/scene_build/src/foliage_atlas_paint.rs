//! The foliage-atlas hand-off (Drzewa 3.0 PR5): `world_forge` bakes the leaf masks as plain
//! bytes (renderer-free by the layer DAG); THIS module folds them into the renderer's upload
//! payloads — the color page through the coverage-preserving mip build (a cutout mask melts
//! under a plain box chain), the normal page through the box filter (normals renormalize in
//! the shader).
//!
//! First production caller of `set_foliage_atlas`: from this PR on, the renderer's 1×1 white
//! no-op is replaced by the real atlas everywhere the battlefield draws, and the frame stays
//! pixel-identical until geometry starts carrying nonzero UVs (the look goldens are the
//! regression proof — byte-exact WITHOUT a re-record).
//!
//! Inny Poziom F1: every species gets its impostor sprite pair here, not just the
//! battlefield oak — the backdrop ring past the red line stands on these sprites, and the
//! painted-frustum kit it used to wear is gone.

use renderer_api::{MipMode, Rgba8MipChain, Rgba8MipLevel};
use world_forge::tree::leaf_atlas::bake_leaf_atlas;

/// Exact bytes both pages cost on the GPU, mips included — locked the way the shadow-memory
/// budget is: to the byte, so growth is a deliberate diff, never drift. Two 2048×1024 RGBA8
/// pages with complete chains (re-locked with the oak's impostor strip, Drzewa 3.0 PR10, and
/// again when every species got its sprite pair, Inny Poziom F1: 5 592 408 → 22 369 624 —
/// the price of the backdrop ring standing on real trees instead of painted cones). Re-locked
/// 2026-09-02 (route 2): 2048×1024 → 2048×2048, 22 369 624 → 44 739 240 — the bottom half is
/// the oak's authored cluster block, eight 512 px sprites of real leaves; the MX330 carries
/// 2 GiB and the whole atlas is 2.1 % of it.
pub const FOLIAGE_ATLAS_BYTES: usize = 178_956_968;

/// The world-space window one impostor sprite's INSET rect maps onto, tree-local metres.
/// The crossed quads (`foliage::push_impostor_quads`) are built to exactly these extents, so
/// the sprite's silhouette and the quad agree by shared constants, not by tuning: `top_m` IS
/// the baked tree's tip.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImpostorWindow {
    pub half_width_m: f32,
    pub top_m: f32,
    pub bottom_m: f32,
}

/// The window of `species`' impostor: the exporter's numbers (`impostor.json`), the same
/// the rendered sprite was framed with — the crossed quads and the sprite agree by shared data.
pub fn impostor_quad_window(
    species: world_forge::tree::TreeSpecies,
    variant: u32,
) -> ImpostorWindow {
    let window = world_forge::tree::authored::impostor_window(species, variant);
    ImpostorWindow { half_width_m: window.half_width_m, top_m: window.top_m, bottom_m: 0.0 }
}

/// Bake and mip both atlas pages: (color·alpha, tangent normals). The color page carries the
/// leaf slots (from `world_forge`) AND every species' impostor sprites, splatted here — the
/// paint side owns colors, so the sprite stores exactly what the card path multiplies:
/// authored tone × shade × mask.
pub fn foliage_atlas_chains() -> (Rgba8MipChain, Rgba8MipChain) {
    let image = bake_leaf_atlas();
    let color = Rgba8MipChain::build(
        Rgba8MipLevel::new(image.width, image.height, image.rgba),
        MipMode::AlphaCoveragePreserving,
    );
    let normal = Rgba8MipChain::build(
        Rgba8MipLevel::new(image.width, image.height, image.normal),
        MipMode::Box,
    );
    (color, normal)
}

/// Every species' bark pair as upload payloads (route 2), one array layer per species in
/// `TreeSpecies::ALL` order — the order `surface_role::bark_for_layer` names: both pages
/// box-filtered (an albedo tile is opaque, no coverage to preserve; normals renormalise in
/// the shader).
pub fn bark_texture_layers() -> Vec<(Rgba8MipChain, Rgba8MipChain)> {
    world_forge::tree::TreeSpecies::ALL
        .into_iter()
        .map(|species| {
            let (albedo, normal) = world_forge::tree::authored::bark_pages(species);
            (
                Rgba8MipChain::build(
                    Rgba8MipLevel::new(albedo.width, albedo.height, albedo.rgba.clone()),
                    MipMode::Box,
                ),
                Rgba8MipChain::build(
                    Rgba8MipLevel::new(normal.width, normal.height, normal.rgba.clone()),
                    MipMode::Box,
                ),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use renderer_api::ALPHA_CUTOUT;

    use super::*;

    fn chain_bytes(chain: &Rgba8MipChain) -> usize {
        chain.levels().iter().map(|level| level.rgba().len()).sum()
    }

    /// The exact-bytes budget, both pages, mips included.
    #[test]
    fn the_atlas_memory_is_locked_to_the_byte() {
        let (color, normal) = foliage_atlas_chains();
        assert_eq!(
            chain_bytes(&color) + chain_bytes(&normal),
            FOLIAGE_ATLAS_BYTES,
            "the atlas grew — re-lock deliberately with a measurement"
        );
    }

    /// The world's no-op contract THROUGH the mip chain: texel (0,0) stays opaque white on
    /// every level whose (0,0) texel still lies inside the white slot (128 px = levels 0..=7).
    /// Procedural content samples mip 0 only (zero UV derivatives), but a regression anywhere
    /// in that range means the chain math touched the reserved slot.
    #[test]
    fn the_white_corner_survives_the_chain() {
        let (color, _) = foliage_atlas_chains();
        for level in 0..=7 {
            let rgba = color.levels()[level].rgba();
            assert_eq!(
                &rgba[0..4],
                &[255, 255, 255, 255],
                "mip {level} texel (0,0) broke the no-op"
            );
        }
    }

    /// The reason the color page rides the coverage mode: the whole-page cutout area holds
    /// within ±10% of the base through the mips the 55–150 m band actually samples.
    #[test]
    fn cutout_coverage_holds_through_the_sampled_mips() {
        let (color, _) = foliage_atlas_chains();
        let coverage = |level: &Rgba8MipLevel| {
            let texels = level.rgba().chunks_exact(4);
            let total = texels.len() as f32;
            texels.filter(|t| t[3] >= ALPHA_CUTOUT).count() as f32 / total
        };
        let base = coverage(&color.levels()[0]);
        for level in 1..=4 {
            let drift = coverage(&color.levels()[level]) - base;
            assert!(
                drift.abs() <= 0.10,
                "mip {level} coverage drifted {drift:+.3} from base {base:.3}"
            );
        }
    }

    /// The normal page never carries alpha below the cutout — a normal texel must not be able
    /// to discard anything if a shader ever sampled the wrong page.
    #[test]
    fn the_normal_page_is_fully_opaque() {
        let (_, normal) = foliage_atlas_chains();
        assert!(
            normal.levels()[0].rgba().chunks_exact(4).all(|texel| texel[3] == 255),
            "the normal page grew transparent texels"
        );
    }
}
