//! What a render target is, now that the renderer owns its own depth.
//!
//! The old shape of this file compared the caller's sample count against the renderer's, because
//! the two built their attachments separately and could disagree. They cannot any more: depth is
//! created beside the colour it is written with, from the same width, height and sample count, so
//! the question "do they match?" has no second opinion to ask.
//!
//! What is worth testing instead is that the frame still renders at BOTH sample counts. If depth
//! and colour ever disagreed, `wgpu` would reject the pass outright — so a successful render is
//! the GPU itself certifying the thing the old guard was trying to check by hand.

use renderer_api::{Camera, view_projection_matrix};
use renderer_wgpu::{GpuContext, OffscreenTarget, SceneRenderer, clear_color};

fn headless() -> Option<GpuContext> {
    match GpuContext::headless() {
        Ok(ctx) => Some(ctx),
        Err(error) => {
            eprintln!("skipping MSAA target test: {error}");
            None
        }
    }
}

/// A frame renders at every sample count the renderer ships or reviews at.
///
/// Both counts matter and for different reasons: 1x is what the game draws, 4x is what every
/// committed golden and studio tile is rendered at. A depth buffer built at the wrong count would
/// fail validation on whichever of the two nobody exercised.
#[test]
fn the_scene_depth_always_matches_the_scene_colour() {
    let Some(ctx) = headless() else { return };
    let camera =
        Camera { eye: [0.0, 0.0, 3.0], target: [0.0, 0.0, 0.0], vertical_fov_degrees: 55.0 };
    let view_proj = view_projection_matrix(&camera, 1.0, 0.1, 20.0);

    for sample_count in [1, 4] {
        let target = OffscreenTarget::new(&ctx, 64, 64).expect("offscreen target");
        let mut renderer = SceneRenderer::new_with_sample_count(
            &ctx,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            sample_count,
            &[],
            &[],
        )
        .expect("renderer");
        renderer
            .render(&ctx, target.render_target(), view_proj, camera.eye)
            .unwrap_or_else(|error| panic!("{sample_count}x frame failed to render: {error}"));

        let pixels = target.read_rgba8(&ctx).expect("readback");
        assert!(
            pixels.chunks_exact(4).any(|p| p[..3] != [0, 0, 0]),
            "the {sample_count}x frame came back black — nothing reached the target"
        );
    }
}

/// The target is a single-sample surface with a readback buffer and nothing else, so the smoke
/// clear is a plain clear: no MSAA attachment to resolve, because there is no longer one to own.
#[test]
fn clear_color_fills_the_readable_target() {
    let Some(ctx) = headless() else { return };
    let target = OffscreenTarget::new(&ctx, 4, 4).expect("offscreen target");
    clear_color(&ctx, &target, [0.25, 0.50, 0.75, 1.0]).expect("clear color");

    let pixels = target.read_rgba8(&ctx).expect("readback");
    assert_eq!(pixels.len(), 4 * 4 * 4);
    for pixel in pixels.chunks_exact(4) {
        assert!(pixel[0] > 100 && pixel[1] > 150 && pixel[2] > 200, "unexpected pixel {pixel:?}");
    }
}
