//! Teren 2.0 (Inny Poziom T3): the ground's detail MATERIAL on the GPU. `wgsl_layout` locks
//! the mechanism in the shader text; this renders it. A flat, all-grass field through the
//! terrain pipeline from a tank's eye, with the detail tiles on and off:
//!
//! - the mid-field (50–150 m) is NOT one flat tone — T3's macro variation is in the picture,
//!   measured as block-mean luma spread across the band after the fog/sun gradient is removed;
//! - the near field carries a fine grain with the tiles on and none with them off (the detail
//!   bit gates the taps, so the min-spec probe can price them);
//! - and the tiles are neutral: the field's MEAN luma moves by under one percent when they
//!   come on — a material adds detail, it does not retint the map.
//!
//! GPU-only; skips without an adapter like the other render tests.

use renderer_api::{
    LightingQuality, SceneLighting, SceneVertex, ShaderDetailMask, TerrainGroundMaps,
    TerrainMaterialSet, view_projection_matrix,
};
use renderer_wgpu::{GpuContext, OffscreenTarget, SceneRenderer};

const WIDTH: u32 = 960;
const HEIGHT: u32 = 540;
const FIELD_M: f32 = 400.0;

fn luma(px: &[u8]) -> f32 {
    let lin = |c: u8| {
        let v = c as f32 / 255.0;
        if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * lin(px[0]) + 0.7152 * lin(px[1]) + 0.0722 * lin(px[2])
}

/// A flat field spanning the ground maps' extent exactly (UV = world.xz / extent).
fn field() -> (Vec<SceneVertex>, Vec<u32>) {
    let corners =
        [[0.0, 0.0, 0.0], [FIELD_M, 0.0, 0.0], [FIELD_M, 0.0, FIELD_M], [0.0, 0.0, FIELD_M]];
    let vertices: Vec<SceneVertex> =
        corners.map(|p| SceneVertex::new(p, [0.0, 1.0, 0.0], [0.3, 0.3, 0.3])).to_vec();
    (vertices, vec![0, 2, 1, 0, 3, 2])
}

/// All grass, flat macro normal, no puddles: whatever varies in the frame is the material.
fn uniform_grass_maps() -> TerrainGroundMaps {
    let size = 16u32;
    let texels = (size * size) as usize;
    TerrainGroundMaps {
        size,
        splat: [255u8, 0, 0, 0].repeat(texels),
        macro_normal: [128u8, 255, 128, 0].repeat(texels),
        extent_m: [FIELD_M, FIELD_M],
    }
}

fn render_field(ctx: &GpuContext, detail_tiles: bool) -> Vec<u8> {
    let (vertices, indices) = field();
    // A far, tiny static so the scene pipeline has something to own; the field itself goes
    // through the terrain pipeline.
    let far = [[-900.0, -50.0, -900.0], [-899.0, -50.0, -900.0], [-900.0, -50.0, -899.0]]
        .map(|p| SceneVertex::new(p, [0.0, 1.0, 0.0], [0.3; 3]))
        .to_vec();
    let mut quality = LightingQuality::canonical();
    if !detail_tiles {
        quality.shader_detail = ShaderDetailMask(
            quality.shader_detail.0
                & !(ShaderDetailMask::TERRAIN_NORMAL_BEND | ShaderDetailMask::TERRAIN_MICRO_OCTAVE),
        );
    }
    let mut renderer = SceneRenderer::for_offscreen_with_quality(ctx, &far, &[0, 1, 2], quality)
        .expect("renderer");
    // No field quilt: the plots are a separate (vertex-stage) structure this test does not
    // measure — the frame's variation must come from the material and the macro tile alone.
    let materials =
        TerrainMaterialSet { field_patch_strength: 0.0, ..TerrainMaterialSet::default() };
    renderer.set_battlefield_ground(ctx, &vertices, &indices, &uniform_grass_maps(), &materials);
    let mut lighting = SceneLighting::battlefield_default();
    // No cloud shade: its own macro pattern would be counted as the ground's.
    lighting.cloud_shadow_strength = 0.0;
    renderer.scene_lighting = lighting;
    renderer.set_ssao_enabled(false);
    // A tank's eye, 3 m up in the middle of the field, looking down the -z half.
    let eye = [FIELD_M * 0.5, 3.0, FIELD_M * 0.75];
    let camera = renderer_api::Camera {
        eye,
        target: [FIELD_M * 0.5, 0.0, FIELD_M * 0.75 - 60.0],
        vertical_fov_degrees: 48.0,
    };
    renderer.shadow_focus = Some(camera.target);
    let view_proj = view_projection_matrix(&camera, WIDTH as f32 / HEIGHT as f32, 0.1, 1500.0);
    let target = OffscreenTarget::new(ctx, WIDTH, HEIGHT).expect("target");
    renderer.render(ctx, target.render_target(), view_proj, eye).expect("render");
    target.read_rgba8(ctx).expect("readback")
}

/// The screen row at which flat ground `distance_m` ahead of the eye projects.
fn row_of_distance(distance_m: f32) -> usize {
    let eye_h = 3.0_f32;
    let pitch = (3.0_f32 / 60.0).atan(); // the camera looks 60 m out
    let angle_below = (eye_h / distance_m).atan() - pitch; // below the view axis
    let half_fov = 24.0_f32.to_radians();
    let f = 0.5 * HEIGHT as f32 / half_fov.tan();
    (0.5 * HEIGHT as f32 + f * angle_below.tan()).round().clamp(0.0, HEIGHT as f32 - 1.0) as usize
}

fn luma_plane(rgba: &[u8]) -> Vec<f32> {
    rgba.chunks_exact(4).map(luma).collect()
}

/// Spread of 8x8 block means across a row band, each row centred first (the sun/fog gradient
/// runs down the frame; what is left is variation ALONG the ground).
fn block_spread(plane: &[f32], rows: std::ops::Range<usize>, block: usize) -> f32 {
    let w = WIDTH as usize;
    let mut values = Vec::new();
    let mut y = rows.start;
    while y + block <= rows.end {
        let mut row_means = Vec::new();
        let mut x = 0;
        while x + block <= w {
            let mut sum = 0.0;
            for yy in y..y + block {
                for xx in x..x + block {
                    sum += plane[yy * w + xx];
                }
            }
            row_means.push(sum / (block * block) as f32);
            x += block;
        }
        let mean = row_means.iter().sum::<f32>() / row_means.len() as f32;
        values.extend(row_means.iter().map(|v| v - mean));
        y += block;
    }
    let var = values.iter().map(|v| v * v).sum::<f32>() / values.len().max(1) as f32;
    var.sqrt()
}

/// High-pass energy inside a row band: the RMS of each pixel against its 9x9 box mean — the
/// fine grain, with the row gradient and the macro blotches removed.
fn grain(plane: &[f32], rows: std::ops::Range<usize>) -> f32 {
    let w = WIDTH as usize;
    let r = 4isize;
    let mut sum = 0.0;
    let mut n = 0;
    for y in rows.start.max(r as usize)..rows.end.min(HEIGHT as usize - r as usize) {
        for x in r as usize..w - r as usize {
            let mut local = 0.0;
            for dy in -r..=r {
                for dx in -r..=r {
                    local += plane[(y as isize + dy) as usize * w + (x as isize + dx) as usize];
                }
            }
            local /= ((2 * r + 1) * (2 * r + 1)) as f32;
            let d = plane[y * w + x] - local;
            sum += d * d;
            n += 1;
        }
    }
    (sum / n.max(1) as f32).sqrt()
}

fn band_mean(plane: &[f32], rows: std::ops::Range<usize>) -> f32 {
    let w = WIDTH as usize;
    let slice = &plane[rows.start * w..rows.end * w];
    slice.iter().sum::<f32>() / slice.len() as f32
}

#[test]
fn the_ground_material_varies_the_mid_field_and_grains_the_near_field_without_retinting() {
    let Some(ctx) = (match GpuContext::headless() {
        Ok(ctx) => Some(ctx),
        Err(error) => {
            eprintln!("skipping ground material test: {error}");
            None
        }
    }) else {
        return;
    };
    let with = luma_plane(&render_field(&ctx, true));
    let without = luma_plane(&render_field(&ctx, false));

    // Rows grow downward: a farther distance is a higher row (nearer the horizon).
    let mid = row_of_distance(150.0)..row_of_distance(50.0);
    let near = row_of_distance(20.0)..row_of_distance(6.0);
    assert!(mid.end > mid.start + 16 && near.end > near.start + 16, "bands {mid:?} {near:?}");

    let mid_spread_with = block_spread(&with, mid.clone(), 8);
    let mid_spread_without = block_spread(&without, mid.clone(), 8);
    let near_grain_with = grain(&with, near.clone());
    let near_grain_without = grain(&without, near.clone());
    let mean_with = band_mean(&with, mid.start..near.end);
    let mean_without = band_mean(&without, mid.start..near.end);
    eprintln!(
        "mid spread {mid_spread_with:.5} (tiles off {mid_spread_without:.5}); near grain \
         {near_grain_with:.5} (off {near_grain_without:.5}); mean {mean_with:.4} vs {mean_without:.4}"
    );

    // T3: the mid-field carries macro variation. The floors are what the material measured
    // on landing (0.0053 / 0.0051) less a margin. The tiles-off frame still carries the macro
    // tone tile, which is not gated — so its spread is the macro tile's own contribution and
    // must itself clear a floor; the detail tiles add the rest on top.
    assert!(mid_spread_with >= 0.0035, "the mid-field reads flat: spread {mid_spread_with:.5}");
    assert!(
        mid_spread_without >= 0.0030,
        "the macro tone tile is not reaching the mid-field: spread {mid_spread_without:.5}"
    );
    // The near grain is the tiles' (0.0067 against a 0.0020 quantization floor on landing),
    // and the detail bits gate it.
    assert!(
        near_grain_with > near_grain_without * 2.5,
        "the near field shows no material grain: {near_grain_with:.5} vs {near_grain_without:.5}"
    );
    // Neutral: the material does not retint the map.
    let drift = (mean_with - mean_without).abs() / mean_without.max(1.0e-4);
    assert!(drift < 0.01, "the tiles moved the field's mean luma by {:.2}%", drift * 100.0);
}
