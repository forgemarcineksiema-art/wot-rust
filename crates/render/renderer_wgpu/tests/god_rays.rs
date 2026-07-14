//! The crepuscular-ray contract (`docs/art-direction-policy.md` rules 4/6): with the low sun in
//! frame, the profile's ray strength streaks hot HDR energy toward the camera — pixels between
//! the viewer and the sun brighten — while a zero strength leaves the frame untouched and the
//! picture's total energy stays bounded (painted light, not free light). Skips without a GPU.

use renderer_api::{Camera, SceneLighting, SceneVertex, view_projection_matrix};
use renderer_wgpu::{GpuContext, OffscreenTarget, SceneRenderer};

fn headless_context() -> Option<GpuContext> {
    match GpuContext::headless() {
        Ok(ctx) => Some(ctx),
        Err(error) => {
            eprintln!("skipping god ray test: {error}");
            None
        }
    }
}

fn render_with_rays(ctx: &GpuContext, strength: f32) -> Vec<u8> {
    let vertices = vec![
        SceneVertex::new([-50.0, 0.0, -50.0], [0.0, 1.0, 0.0], [0.3, 0.33, 0.22]),
        SceneVertex::new([50.0, 0.0, -50.0], [0.0, 1.0, 0.0], [0.3, 0.33, 0.22]),
        SceneVertex::new([50.0, 0.0, 50.0], [0.0, 1.0, 0.0], [0.3, 0.33, 0.22]),
        SceneVertex::new([-50.0, 0.0, 50.0], [0.0, 1.0, 0.0], [0.3, 0.33, 0.22]),
    ];
    let indices = vec![0u32, 2, 1, 0, 3, 2];
    let target = OffscreenTarget::new(ctx, 192, 108).expect("target");
    let mut renderer = SceneRenderer::for_offscreen_with_quality(
        ctx,
        &vertices,
        &indices,
        renderer_api::LightingQuality::rich(),
    )
    .expect("renderer");
    let mut lighting = SceneLighting::prokhorovka_golden_evening();
    lighting.god_ray_strength = strength;
    lighting.bloom_weight = 0.0;
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
fn rays_streak_toward_the_sun_without_flooding_the_frame() {
    let Some(ctx) = headless_context() else {
        return;
    };
    let off = render_with_rays(&ctx, 0.0);
    let on = render_with_rays(&ctx, 0.25);

    let mut brightened = 0u32;
    for (a, b) in off.chunks_exact(4).zip(on.chunks_exact(4)) {
        let la = 0.2126 * a[0] as f32 + 0.7152 * a[1] as f32 + 0.0722 * a[2] as f32;
        let lb = 0.2126 * b[0] as f32 + 0.7152 * b[1] as f32 + 0.0722 * b[2] as f32;
        if la < 235.0 && lb > la + 3.0 {
            brightened += 1;
        }
    }
    assert!(brightened > 50, "rays must brighten the air toward the sun: {brightened} px");

    let ratio = total_luma(&on) / total_luma(&off).max(1.0);
    assert!(
        (1.0..=1.10).contains(&ratio),
        "rays are painted accents, never a flood: energy ratio {ratio:.4}"
    );
}
