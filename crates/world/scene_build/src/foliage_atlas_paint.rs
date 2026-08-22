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

use glam::Vec3;
use renderer_api::{MipMode, Rgba8MipChain, Rgba8MipLevel};
use world_forge::tree::leaf_atlas::{
    self, IMPOSTOR_SPRITE_H, IMPOSTOR_SPRITE_W, LeafAtlasImage, bake_leaf_atlas,
};

/// Exact bytes both pages cost on the GPU, mips included — locked the way the shadow-memory
/// budget is: to the byte, so growth is a deliberate diff, never drift. Two 1024×512 RGBA8
/// pages with complete chains (re-locked with the impostor strip, Drzewa 3.0 PR10).
pub const FOLIAGE_ATLAS_BYTES: usize = 5_592_408;

/// The world-space window one impostor sprite's INSET rect maps onto, tree-local metres.
/// The crossed quads in `tree_lod` are built to exactly these extents, so the sprite's
/// silhouette and the quad agree by shared constants, not by tuning: `top_m` IS the baked
/// tree's tip.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImpostorWindow {
    pub half_width_m: f32,
    pub top_m: f32,
    pub bottom_m: f32,
}

/// The window of the battlefield tree's impostor (the representative rung individual).
pub fn battle_tree_impostor_window() -> ImpostorWindow {
    let tree = world_forge::tree::bake_tree_lod(
        crate::tree_lod::BATTLE_TREE,
        crate::tree_lod::RUNG_SEED,
        world_forge::tree::TreeLod::Close,
    );
    let tip = tree.tip();
    let reach = tree
        .leaves
        .iter()
        .map(|card| {
            let r = card.center + card.half_right.abs() + card.half_up.abs();
            (r.x * r.x + r.z * r.z).sqrt()
        })
        .fold(1.0_f32, f32::max);
    ImpostorWindow { half_width_m: reach, top_m: tip, bottom_m: 0.0 }
}

/// Bake and mip both atlas pages: (color·alpha, tangent normals). The color page carries the
/// leaf slots (from `world_forge`) AND the battlefield tree's impostor sprites, splatted here
/// — the paint side owns colors, so the sprite stores exactly what the card path multiplies:
/// authored tone × shade × mask.
pub fn foliage_atlas_chains() -> (Rgba8MipChain, Rgba8MipChain) {
    let mut image = bake_leaf_atlas();
    splat_battle_tree_impostor(&mut image);
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

fn srgb_to_linear(byte: u8) -> f32 {
    let c = byte as f32 / 255.0;
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

fn linear_to_srgb(linear: f32) -> u8 {
    let c = linear.clamp(0.0, 1.0);
    let encoded = if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
    (encoded * 255.0).round() as u8
}

/// Splat the battlefield tree into its two impostor sprites: a CPU orthographic painter's
/// rasterizer over the SAME bake the Near rung draws — bark triangles filled flat in the
/// trunk tone, cards filled by sampling their own atlas slot times the canopy tone and the
/// card's shade lane. No render-to-texture, no new pass, no baked sun: the sprite stores
/// albedo·shade and the FOLIAGE path lights it live like every card.
fn splat_battle_tree_impostor(image: &mut LeafAtlasImage) {
    let tree = world_forge::tree::bake_tree_lod(
        crate::tree_lod::BATTLE_TREE,
        crate::tree_lod::RUNG_SEED,
        world_forge::tree::TreeLod::Close,
    );
    let window = battle_tree_impostor_window();
    let (canopy_color, _) = crate::foliage::canopy_color_for_species(crate::tree_lod::BATTLE_TREE);
    let (trunk_color, _) = crate::foliage::TRUNK_TONE;
    // Pre-decode the leaf grid once: card splats sample their slot in linear space, and the
    // alpha snapshot keeps the reads off the buffer the splat is writing into.
    let leaf_page: Vec<f32> = image.rgba.iter().map(|&byte| srgb_to_linear(byte)).collect();
    let leaf_alpha: Vec<u8> = image.rgba.chunks_exact(4).map(|texel| texel[3]).collect();

    for which in 0..2u32 {
        // Azimuth 0 looks down -Z (the sprite plane spans X); azimuth 1 spans Z.
        let (right, depth_dir) = if which == 0 { (Vec3::X, Vec3::Z) } else { (Vec3::Z, -Vec3::X) };
        // Painter's order: primitives sorted far-to-near along the view direction.
        enum Splat<'a> {
            Bark([Vec3; 3]),
            Card(&'a world_forge::tree::leaves::LeafCard),
        }
        let mut primitives: Vec<(f32, Splat)> = Vec::new();
        let trunk = &tree.trunk;
        for triangle in trunk.indices().chunks_exact(3) {
            let corners = [
                trunk.vertices()[triangle[0] as usize].position,
                trunk.vertices()[triangle[1] as usize].position,
                trunk.vertices()[triangle[2] as usize].position,
            ];
            let depth = (corners[0] + corners[1] + corners[2]).dot(depth_dir) / 3.0;
            primitives.push((depth, Splat::Bark(corners)));
        }
        for card in &tree.leaves {
            primitives.push((card.center.dot(depth_dir), Splat::Card(card)));
        }
        primitives.sort_by(|a, b| a.0.total_cmp(&b.0));

        let (origin_x, origin_y) = leaf_atlas::impostor_origin(which);
        let margin = 6.0_f32;
        let px_w = IMPOSTOR_SPRITE_W as f32 - 2.0 * margin;
        let px_h = IMPOSTOR_SPRITE_H as f32 - 2.0 * margin;
        let to_px = |world: Vec3| -> (f32, f32) {
            let x_r = world.dot(right);
            (
                margin + (x_r + window.half_width_m) / (2.0 * window.half_width_m) * px_w,
                margin + (window.top_m - world.y) / (window.top_m - window.bottom_m) * px_h,
            )
        };
        let mut put = |px: f32, py: f32, rgb: [f32; 3], alpha: u8| {
            if px < 0.0
                || py < 0.0
                || px >= IMPOSTOR_SPRITE_W as f32
                || py >= IMPOSTOR_SPRITE_H as f32
            {
                return;
            }
            let index = (((origin_y + py as u32) * leaf_atlas::ATLAS_WIDTH) + origin_x + px as u32)
                as usize
                * 4;
            image.rgba[index] = linear_to_srgb(rgb[0]);
            image.rgba[index + 1] = linear_to_srgb(rgb[1]);
            image.rgba[index + 2] = linear_to_srgb(rgb[2]);
            image.rgba[index + 3] = image.rgba[index + 3].max(alpha);
            // The sprite's dome normal (user verdict 2026-08-22): a flat-lit portrait next
            // to volume-lit cards was half the 150 m pop. The whole sprite borrows a gentle
            // sphere-cap around its own center, exactly like a leaf slot does — the FOLIAGE
            // path then shades the far oak as a mass, not a billboard.
            let nx = ((px / IMPOSTOR_SPRITE_W as f32) * 2.0 - 1.0) * 0.55;
            let ny = (1.0 - (py / IMPOSTOR_SPRITE_H as f32) * 2.0) * 0.55;
            let nz = (1.0 - nx * nx - ny * ny).max(0.05).sqrt();
            image.normal[index] = ((nx * 0.5 + 0.5) * 255.0) as u8;
            image.normal[index + 1] = ((ny * 0.5 + 0.5) * 255.0) as u8;
            image.normal[index + 2] = ((nz * 0.5 + 0.5) * 255.0) as u8;
        };

        for (_, primitive) in primitives {
            match primitive {
                Splat::Bark(corners) => {
                    let projected = corners.map(to_px);
                    let (min_x, max_x, min_y, max_y) = projected
                        .iter()
                        .fold((f32::MAX, f32::MIN, f32::MAX, f32::MIN), |acc, &(x, y)| {
                            (acc.0.min(x), acc.1.max(x), acc.2.min(y), acc.3.max(y))
                        });
                    let edge = |a: (f32, f32), b: (f32, f32), p: (f32, f32)| {
                        (b.0 - a.0) * (p.1 - a.1) - (b.1 - a.1) * (p.0 - a.0)
                    };
                    for py in min_y.floor() as i32..=max_y.ceil() as i32 {
                        for px in min_x.floor() as i32..=max_x.ceil() as i32 {
                            let p = (px as f32 + 0.5, py as f32 + 0.5);
                            let e0 = edge(projected[0], projected[1], p);
                            let e1 = edge(projected[1], projected[2], p);
                            let e2 = edge(projected[2], projected[0], p);
                            if (e0 >= 0.0 && e1 >= 0.0 && e2 >= 0.0)
                                || (e0 <= 0.0 && e1 <= 0.0 && e2 <= 0.0)
                            {
                                put(p.0, p.1, trunk_color, 255);
                            }
                        }
                    }
                }
                Splat::Card(card) => {
                    let center = to_px(card.center);
                    let hr = {
                        let tip = to_px(card.center + card.half_right);
                        (tip.0 - center.0, tip.1 - center.1)
                    };
                    let hu = {
                        let tip = to_px(card.center + card.half_up);
                        (tip.0 - center.0, tip.1 - center.1)
                    };
                    let det = hr.0 * hu.1 - hr.1 * hu.0;
                    if det.abs() < 1.0e-3 {
                        continue; // edge-on: invisible from this azimuth
                    }
                    let rect = leaf_atlas::atlas_rect(card.slot);
                    let reach_x = hr.0.abs() + hu.0.abs();
                    let reach_y = hr.1.abs() + hu.1.abs();
                    for py in
                        (center.1 - reach_y).floor() as i32..=(center.1 + reach_y).ceil() as i32
                    {
                        for px in
                            (center.0 - reach_x).floor() as i32..=(center.0 + reach_x).ceil() as i32
                        {
                            let d = (px as f32 + 0.5 - center.0, py as f32 + 0.5 - center.1);
                            // Invert p = a·hr + b·hu.
                            let a = (d.0 * hu.1 - d.1 * hu.0) / det;
                            let b = (hr.0 * d.1 - hr.1 * d.0) / det;
                            if a.abs() > 1.0 || b.abs() > 1.0 {
                                continue;
                            }
                            let u = rect[0] + (a * 0.5 + 0.5) * (rect[2] - rect[0]);
                            let v = rect[3] + (b * 0.5 + 0.5) * (rect[1] - rect[3]);
                            let tx = (u * leaf_atlas::ATLAS_WIDTH as f32) as usize;
                            let ty = (v * leaf_atlas::ATLAS_HEIGHT as f32) as usize;
                            let t = (ty * leaf_atlas::ATLAS_WIDTH as usize + tx) * 4;
                            let alpha = leaf_alpha[t / 4];
                            if alpha < 128 {
                                continue;
                            }
                            let rgb = [
                                leaf_page[t] * canopy_color[0] * card.shade,
                                leaf_page[t + 1] * canopy_color[1] * card.shade,
                                leaf_page[t + 2] * canopy_color[2] * card.shade,
                            ];
                            put(px as f32 + 0.5, py as f32 + 0.5, rgb, alpha);
                        }
                    }
                }
            }
        }
    }
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

    /// The impostor sprites hold a real tree (PR10): substantial cutout coverage, a crisp
    /// bimodal alpha rim, and — the ladder's whole invariant, in PIXELS — the silhouette's
    /// top row lands where the window mapping puts the baked tip (the inset margin row).
    #[test]
    fn the_impostor_sprites_hold_the_tree_and_its_tip() {
        let (color, _) = foliage_atlas_chains();
        let page = color.levels()[0].rgba();
        let window = battle_tree_impostor_window();
        let tree = world_forge::tree::bake_tree_lod(
            crate::tree_lod::BATTLE_TREE,
            crate::tree_lod::RUNG_SEED,
            world_forge::tree::TreeLod::Close,
        );
        assert!(
            (window.top_m - tree.tip()).abs() < 1.0e-4,
            "the window's top IS the baked tip: {} vs {}",
            window.top_m,
            tree.tip()
        );
        for which in 0..2u32 {
            let (ox, oy) = leaf_atlas::impostor_origin(which);
            let (mut covered, mut crisp, mut top_row) = (0u32, 0u32, u32::MAX);
            for py in 0..IMPOSTOR_SPRITE_H {
                for px in 0..IMPOSTOR_SPRITE_W {
                    let index = (((oy + py) * leaf_atlas::ATLAS_WIDTH + ox + px) * 4 + 3) as usize;
                    let alpha = page[index];
                    crisp += u32::from(!(64..=192).contains(&alpha));
                    if alpha >= 128 {
                        covered += 1;
                        top_row = top_row.min(py);
                    }
                }
            }
            let total = IMPOSTOR_SPRITE_W * IMPOSTOR_SPRITE_H;
            let coverage = covered as f32 / total as f32;
            assert!(
                (0.05..=0.6).contains(&coverage),
                "azimuth {which}: the sprite holds a tree, not a smear: {coverage:.3}"
            );
            assert!(crisp * 100 >= total * 92, "azimuth {which}: the sprite rim went foggy");
            // The tip lands near the inset margin row (6 px). The slack above the margin is
            // the tip cluster's own mask inset — a cluster mask never reaches its quad
            // corner, and since the cross-pair rework the geometric tip also carries quad
            // B's diagonal — and the LIVE card render has exactly the same gap between quad
            // top and cutout, so the sprite and the mesh agree about where the crown
            // visually ends.
            assert!(
                (3..=28).contains(&top_row),
                "azimuth {which}: the sprite tip drifted to row {top_row}"
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
