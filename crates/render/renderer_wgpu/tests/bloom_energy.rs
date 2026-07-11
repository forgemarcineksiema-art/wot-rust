//! The bloom contract (`docs/art-direction-policy.md` rule 6): threshold-free and
//! energy-conserving. With the sun in frame, turning the profile's bloom on must GROW a halo
//! around hot pixels without meaningfully growing the picture's total energy — glow is
//! redistribution, never free light. Runs on the headless adapter; skips without one.

use renderer_api::{Camera, SceneLighting, SceneVertex, view_projection_matrix};
use renderer_wgpu::{GpuContext, OffscreenTarget, SceneRenderer};

fn headless_context() -> Option<GpuContext> {
    match GpuContext::headless() {
        Ok(ctx) => Some(ctx),
        Err(error) => {
            eprintln!("skipping bloom energy test: {error}");
            None
        }
    }
}

fn render_with_bloom(ctx: &GpuContext, weight: f32) -> Vec<u8> {
    let vertices = vec![
        SceneVertex::new([-50.0, 0.0, -50.0], [0.0, 1.0, 0.0], [0.3, 0.33, 0.22]),
        SceneVertex::new([50.0, 0.0, -50.0], [0.0, 1.0, 0.0], [0.3, 0.33, 0.22]),
        SceneVertex::new([50.0, 0.0, 50.0], [0.0, 1.0, 0.0], [0.3, 0.33, 0.22]),
        SceneVertex::new([-50.0, 0.0, 50.0], [0.0, 1.0, 0.0], [0.3, 0.33, 0.22]),
    ];
    let indices = vec![0u32, 2, 1, 0, 3, 2];
    let target = OffscreenTarget::new(ctx, 192, 108).expect("target");
    let mut renderer = SceneRenderer::for_offscreen(ctx, &vertices, &indices).expect("renderer");
    let mut lighting = SceneLighting::battlefield_default();
    lighting.bloom_weight = weight;
    lighting.vignette = 0.0;
    let sun = lighting.key_direction;
    renderer.scene_lighting = lighting;
    let eye = [0.0f32, 2.0, 0.0];
    let look = [eye[0] + sun[0] * 100.0, eye[1] + sun[1] * 100.0, eye[2] + sun[2] * 100.0];
    let camera = Camera { eye, target: look, vertical_fov_degrees: 55.0 };
    let view_proj = view_projection_matrix(&camera, 192.0 / 108.0, 0.1, 2000.0);
    renderer.render(ctx, target.render_target(), view_proj, camera.eye).expect("render");
    target.read_rgba8(ctx).expect("readback")
}

fn total_luma(pixels: &[u8]) -> f64 {
    pixels
        .chunks_exact(4)
        .map(|p| 0.2126 * p[0] as f64 + 0.7152 * p[1] as f64 + 0.0722 * p[2] as f64)
        .sum()
}

#[test]
fn bloom_grows_a_halo_without_growing_the_picture() {
    let Some(ctx) = headless_context() else {
        return;
    };
    let off = render_with_bloom(&ctx, 0.0);
    let on = render_with_bloom(&ctx, 0.10);

    // Redistribution, not free light: total energy moves by at most a few percent.
    let (e_off, e_on) = (total_luma(&off), total_luma(&on));
    let ratio = e_on / e_off.max(1.0);
    assert!(
        (0.98..=1.06).contains(&ratio),
        "bloom must conserve the picture's energy: ratio {ratio:.4}"
    );

    // But the glow is real: pixels NEAR the hot sun (bright with bloom, previously mid) exist.
    let mut halo = 0u32;
    for (a, b) in off.chunks_exact(4).zip(on.chunks_exact(4)) {
        let la = 0.2126 * a[0] as f32 + 0.7152 * a[1] as f32 + 0.0722 * a[2] as f32;
        let lb = 0.2126 * b[0] as f32 + 0.7152 * b[1] as f32 + 0.0722 * b[2] as f32;
        if la < 220.0 && lb > la + 4.0 {
            halo += 1;
        }
    }
    assert!(halo > 30, "the sun must gain a real halo: {halo} brightened pixels");
}
