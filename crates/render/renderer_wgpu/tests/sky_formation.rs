//! Executable contracts for the sky pass's sun/cloud interplay: the disc must be occluded by
//! the cloud cover at its pixel (an overcast lid with a hot key must not show a sun blazing
//! through solid cloud — before the occlusion term, only a weak grey key hid it), and rays
//! looking below the horizon must stay finite (the cloud projections divide by a term that
//! crosses zero ~27° under the horizon; unguarded, fbm(inf) can ride a NaN into the pixel
//! because NaN * 0.0 = NaN). Runs on the headless adapter; skips if none is available.

mod common;
use common::luma;

use renderer_api::{SceneLighting, SceneVertex, view_projection_matrix};
use renderer_wgpu::{GpuContext, OffscreenTarget, SceneRenderer};

/// A single far-away triangle so the renderer has a mesh; every pixel in frame is sky.
fn out_of_frame_geometry() -> (Vec<SceneVertex>, Vec<u32>) {
    let n = [0.0, 1.0, 0.0];
    let c = [0.3, 0.3, 0.3];
    let vertices = vec![
        SceneVertex::new([-1.0, -50.0, 60.0], n, c),
        SceneVertex::new([1.0, -50.0, 60.0], n, c),
        SceneVertex::new([0.0, -50.0, 61.0], n, c),
    ];
    (vertices, vec![0, 1, 2])
}

fn render_sky(
    ctx: &GpuContext,
    lighting: SceneLighting,
    eye: [f32; 3],
    target_at: [f32; 3],
) -> Vec<u8> {
    let (vertices, indices) = out_of_frame_geometry();
    let camera = renderer_api::Camera { eye, target: target_at, vertical_fov_degrees: 55.0 };
    let view_proj = view_projection_matrix(&camera, 1.0, 0.1, 200.0);
    let target = OffscreenTarget::new(ctx, 96, 96).expect("target");
    let mut renderer = SceneRenderer::for_offscreen(ctx, &vertices, &indices).expect("renderer");
    renderer.scene_lighting = lighting;
    renderer.render(ctx, target.render_target(), view_proj, camera.eye).expect("render");
    target.read_rgba8(ctx).expect("readback")
}

#[test]
fn an_overcast_lid_occludes_the_sun_disc() {
    let Some(ctx) = (match GpuContext::headless() {
        Ok(ctx) => Some(ctx),
        Err(error) => {
            eprintln!("skipping sun occlusion test: {error}");
            None
        }
    }) else {
        return;
    };

    // The camera looks straight at a mid-sky sun (elevation ~27° — well inside the cloud band)
    // under the battle profile's own hot key. Since the disc multiplier fell 6.0 -> 3.0 (the
    // D3 close: the flat-white plateau was most of "the sun is whitish"), a WEAK grey key no
    // longer clips at all — correctly: a rain-grade sun has no white core. The scenario that
    // separates the two outcomes is therefore the hot key: its naked disc still clips a real
    // core, while under a full lid even that key's brightest legitimate surface (the lit cloud
    // face, ~0.94 sRGB after the grade) stays below saturation — any additive disc energy
    // leaking through the lid would clip there and fail the zero-count assert.
    let mut lighting = SceneLighting::battlefield_default();
    lighting.key_direction = [0.0, 0.5, -1.0];
    lighting.cloud_coverage_bias = 1.0;
    let eye = [0.0, 2.0, 0.0];
    let at = [0.0, 7.0, -10.0];

    let mut open = lighting;
    open.cloud_opacity = 0.0;
    let clear = render_sky(&ctx, open, eye, at);
    let mut lid = lighting;
    lid.cloud_opacity = 1.0;
    let overcast = render_sky(&ctx, lid, eye, at);

    // The probe is the count of near-saturated pixels: the naked disc's core clips to white,
    // while the brightest thing a full lid may show is the lit cloud face (~0.88 after the
    // grade) — additive disc energy leaking through the lid would clip there too.
    let hot = |pixels: &[u8]| pixels.chunks_exact(4).filter(|p| luma(p) > 0.97).count();
    let clear_hot = hot(&clear);
    let lid_hot = hot(&overcast);
    assert!(clear_hot >= 10, "the unclouded disc must clip a real core, got {clear_hot} px");
    assert_eq!(
        lid_hot, 0,
        "no pixel under a full lid may clip: the disc must die behind the cloud cover"
    );
}

#[test]
fn looking_below_the_horizon_stays_finite_and_deterministic() {
    let Some(ctx) = (match GpuContext::headless() {
        Ok(ctx) => Some(ctx),
        Err(error) => {
            eprintln!("skipping horizon singularity test: {error}");
            None
        }
    }) else {
        return;
    };

    // Every ray in this frame points 3°–58° below the horizon, sweeping across the cloud
    // projections' singular directions (dir.y = -0.45 and -0.55) with both layers turned up.
    // Unguarded, fbm of an inf uv can produce NaN there, and NaN converts to black in the
    // unorm target; the guarded gradient is a bright horizon wash on every pixel, twice over.
    let mut lighting = SceneLighting::battlefield_default();
    lighting.cloud_coverage_bias = 1.0;
    lighting.cloud_opacity = 1.0;
    lighting.cloud_sheet_opacity = 0.5;
    let eye = [0.0, 40.0, 0.0];
    let at = [0.0, 34.0, -10.0];

    let first = render_sky(&ctx, lighting, eye, at);
    let second = render_sky(&ctx, lighting, eye, at);
    assert_eq!(first, second, "the below-horizon sky must render deterministically");
    let min = first.chunks_exact(4).map(luma).fold(f32::MAX, f32::min);
    assert!(
        min > 0.1,
        "below-horizon sky pixels must hold the horizon wash, not NaN-black: min luma {min:.4}"
    );
}
