use super::*;

#[test]
fn shipped_atlas_chain_is_complete_deterministic_and_white_at_the_origin() {
    let first = build_catalog().expect("packs");
    let second = build_catalog().expect("packs again");
    assert_eq!(first.atlas_mips, second.atlas_mips, "byte-deterministic");
    assert_eq!(first.atlas_mips.levels().len(), 12, "2048 through 1");
    let mut expected = FLORA_ATLAS_SIZE;
    for level in first.atlas_mips.levels() {
        assert_eq!((level.width(), level.height()), (expected, expected));
        assert_eq!(&level.rgba()[0..4], &[255, 255, 255, 255], "UV(0,0) no-op");
        expected = (expected / 2).max(1);
    }
}

#[test]
fn shipped_assets_keep_cutout_coverage_through_the_sampled_levels() {
    for (asset, rgba, _normal) in decode_shipped().expect("decode") {
        let source_coverage = flora_test_coverage(&rgba);
        let levels = mips::build_asset_mip_chain(asset.texture_width, asset.texture_height, rgba);
        for (index, level) in levels.iter().enumerate().take(FLORA_MAX_SAMPLED_MIP as usize + 1) {
            let actual = flora_test_coverage(level.rgba());
            let quantization = 0.5 / (level.width() * level.height()) as f32;
            assert!(
                (actual - source_coverage).abs() <= quantization + 0.01,
                "{} mip {index}: {actual:.4} vs {source_coverage:.4}",
                asset.name
            );
        }
    }
}

#[test]
fn filtering_is_srgb_correct_and_does_not_make_a_dark_alpha_fringe() {
    let checker = vec![255, 255, 255, 255, 0, 0, 0, 255, 255, 255, 255, 255, 0, 0, 0, 255];
    let checker_mips = mips::build_asset_mip_chain(2, 2, checker);
    let average = checker_mips[1].rgba();
    assert!(
        (186..=189).contains(&average[0]),
        "linear-light half-white encodes near sRGB 188: {average:?}"
    );

    let mut cutout = Vec::new();
    for _ in 0..2 {
        cutout.extend_from_slice(&[40, 200, 60, 255]);
        cutout.extend_from_slice(&[40, 200, 60, 64]);
        cutout.extend_from_slice(&[0, 0, 0, 0]);
        cutout.extend_from_slice(&[0, 0, 0, 0]);
    }
    let cutout_mips = mips::build_asset_mip_chain(4, 2, cutout);
    let edge = &cutout_mips[1].rgba()[0..4];
    assert!(edge[3] >= 128, "coverage preservation keeps the edge");
    assert_eq!(&edge[0..3], &[40, 200, 60], "premultiplication retains leaf hue");
}

#[test]
fn generated_mip_dilates_leaf_rgb_across_a_cutout_sampling_boundary() {
    let leaf = [40, 200, 60, 255];
    let transparent = [0, 0, 0, 0];
    let mut source = Vec::new();
    for _ in 0..2 {
        source.extend_from_slice(&leaf);
        source.extend_from_slice(&leaf);
        source.extend_from_slice(&transparent);
        source.extend_from_slice(&transparent);
    }
    let mips = mips::build_asset_mip_chain(4, 2, source);
    let base_sample = bilinear_sample_pair(mips[0].rgba(), 1, 2, 0.49);
    assert_cutout_sample_keeps_leaf_color(base_sample, leaf);

    let generated = &mips[1];
    assert_eq!((generated.width(), generated.height()), (2, 1));
    assert_eq!(&generated.rgba()[4..7], &leaf[..3], "RGB is dilated without changing alpha");
    assert_eq!(generated.rgba()[7], 0, "dilation must not alter cutout coverage");

    let sample = bilinear_sample_pair(generated.rgba(), 0, 1, 0.49);
    assert_cutout_sample_keeps_leaf_color(sample, leaf);
}

fn assert_cutout_sample_keeps_leaf_color(sample: [f32; 4], leaf: [u8; 4]) {
    assert!(sample[3] >= 0.5, "the boundary sample must survive the alpha cutoff: {sample:?}");
    for (actual, expected) in sample[..3].iter().zip(leaf[..3].iter()) {
        assert!(
            (actual - *expected as f32 / 255.0).abs() <= 1.0 / 255.0,
            "straight-alpha filtering darkened leaf RGB: {sample:?}"
        );
    }
}

#[test]
fn safe_tail_has_non_overlapping_content_and_gutters_for_every_shipped_region() {
    let catalog = build_catalog().expect("packs");
    let mut guarded = Vec::new();
    for (asset, region) in &catalog.entries {
        let base_x = (region.u_offset * FLORA_ATLAS_SIZE as f32 - 0.5).round() as u32;
        let base_y = (region.v_offset * FLORA_ATLAS_SIZE as f32 - 0.5).round() as u32;
        let x = base_x >> FLORA_MAX_SAMPLED_MIP;
        let y = base_y >> FLORA_MAX_SAMPLED_MIP;
        let width = (asset.texture_width >> FLORA_MAX_SAMPLED_MIP).max(1);
        let height = (asset.texture_height >> FLORA_MAX_SAMPLED_MIP).max(1);
        guarded.push((
            asset.name.as_str(),
            x.saturating_sub(1),
            y.saturating_sub(1),
            x + width,
            y + height,
        ));
    }
    for left in 0..guarded.len() {
        for right in left + 1..guarded.len() {
            let a = guarded[left];
            let b = guarded[right];
            let overlap = a.1 <= b.3 && b.1 <= a.3 && a.2 <= b.4 && b.2 <= a.4;
            assert!(!overlap, "{} gutter overlaps {}", a.0, b.0);
        }
    }
}

#[test]
fn remapped_uvs_stay_on_texel_centers_inside_their_region() {
    let catalog = flora_catalog();
    // The roster is data (`SHIPPED`), so the lock walks whatever actually ships — today the
    // hero oak alone — and survives the family growing without a hand-synced name list.
    assert!(catalog.get("dab-hero").is_some(), "the hero oak ships");
    for (asset, region) in &catalog.entries {
        let name = asset.name.as_str();
        for uv in &asset.uvs {
            let u = region.u_offset + uv[0] * region.u_scale;
            let v = region.v_offset + uv[1] * region.v_scale;
            assert!(
                u >= region.u_offset - 1.0e-6
                    && u <= region.u_offset + region.u_scale + 1.0e-6
                    && v >= region.v_offset - 1.0e-6
                    && v <= region.v_offset + region.v_scale + 1.0e-6,
                "{name}: uv {uv:?} escapes"
            );
        }
    }
}

fn flora_test_coverage(rgba: &[u8]) -> f32 {
    rgba.chunks_exact(4).filter(|pixel| pixel[3] >= 128).count() as f32 / (rgba.len() / 4) as f32
}

fn bilinear_sample_pair(
    rgba: &[u8],
    left_pixel: usize,
    right_pixel: usize,
    right_weight: f32,
) -> [f32; 4] {
    let left_weight = 1.0 - right_weight;
    std::array::from_fn(|channel| {
        (rgba[left_pixel * 4 + channel] as f32 * left_weight
            + rgba[right_pixel * 4 + channel] as f32 * right_weight)
            / 255.0
    })
}
