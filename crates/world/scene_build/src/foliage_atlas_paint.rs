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

use renderer_api::{MipMode, Rgba8MipChain, Rgba8MipLevel};
use world_forge::tree::leaf_atlas::bake_leaf_atlas;

/// Exact bytes both pages cost on the GPU, mips included — locked the way the shadow-memory
/// budget is: to the byte, so growth is a deliberate diff, never drift. Two 512² RGBA8 pages
/// with complete chains: 2 × 1,398,100.
pub const FOLIAGE_ATLAS_BYTES: usize = 2_796_200;

/// Bake and mip both atlas pages: (color·alpha, tangent normals).
pub fn foliage_atlas_chains() -> (Rgba8MipChain, Rgba8MipChain) {
    let image = bake_leaf_atlas();
    let color = Rgba8MipChain::build(
        Rgba8MipLevel::new(image.size, image.size, image.rgba),
        MipMode::AlphaCoveragePreserving,
    );
    let normal = Rgba8MipChain::build(
        Rgba8MipLevel::new(image.size, image.size, image.normal),
        MipMode::Box,
    );
    (color, normal)
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
