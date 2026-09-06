use renderer_api::{Rgba8MipChain, Rgba8MipLevel};
use renderer_wgpu::{GpuContext, SceneRenderer, scene_shader_source, shadow_shader_source};

#[test]
fn color_and_depth_cutout_paths_both_allow_implicit_mip_selection() {
    let color = scene_shader_source();
    let depth = shadow_shader_source();
    assert!(color.contains("textureSample(foliage_atlas, foliage_sampler, input.uv)"));
    assert!(depth.contains("textureSample(foliage_atlas, foliage_sampler, input.uv)"));
    assert!(
        !depth.contains("textureSampleLevel(foliage_atlas"),
        "depth/shadow must not pin the alpha mask to mip zero"
    );
}

/// Drzewa 3.0 PR11: the scene color pass and the shadow cutout casters route their sway
/// through the SAME `foliage_wind_offset` — the gust field plus the uv-gated leaf flutter —
/// so a fluttering canopy's shadow flutters with it, and grass (uv 0) pays nothing by the
/// gate inside the one function. If either pass reaches around it back to the raw meadow
/// field, the two pictures drift apart.
#[test]
fn the_wind_rides_one_function_in_both_passes() {
    let color = scene_shader_source();
    let depth = shadow_shader_source();
    assert!(
        color.contains("foliage_wind_offset(world.xz"),
        "the color pass sways through the shared foliage wind"
    );
    assert!(
        depth.contains("foliage_wind_offset("),
        "the shadow cutout casters sway through the shared foliage wind"
    );
}

#[test]
fn renderer_accepts_and_uploads_every_level_of_a_complete_foliage_chain() {
    let Ok(ctx) = GpuContext::headless() else {
        eprintln!("skipping foliage upload test: no headless adapter");
        return;
    };
    let mut renderer = SceneRenderer::for_offscreen(&ctx, &[], &[]).expect("renderer");
    let chain = Rgba8MipChain::new(
        vec![
            Rgba8MipLevel::new(4, 4, vec![255; 64]),
            Rgba8MipLevel::new(2, 2, vec![255; 16]),
            Rgba8MipLevel::new(1, 1, vec![255; 4]),
        ],
        2,
    );
    renderer.set_foliage_atlas(&ctx, &chain, None);
}
