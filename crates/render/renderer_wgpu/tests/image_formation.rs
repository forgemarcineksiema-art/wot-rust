//! The executable contrast budget for the profile-driven image formation (exposure, black point,
//! grade — `SceneLighting::grade_reference` / `lighting_common.wgsl`): a battle frame containing
//! deep shade and the sun must span real dynamic range — true blacks AND bright highlights. The
//! old hardcoded grade + no exposure could not reach both ends (ACES-lite lifts near-black to
//! grey); this locks that the picture never regresses to that milky look. Runs on the headless
//! adapter; skips if none is available.

use renderer_api::{SceneLighting, SceneVertex, view_projection_matrix};
use renderer_wgpu::{GpuContext, OffscreenTarget, SceneRenderer};

fn luma(p: &[u8]) -> f32 {
    (0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32) / 255.0
}

#[test]
fn a_battle_frame_spans_true_blacks_and_bright_highlights() {
    let Some(ctx) = (match GpuContext::headless() {
        Ok(ctx) => Some(ctx),
        Err(error) => {
            eprintln!("skipping image formation test: {error}");
            None
        }
    }) else {
        return;
    };

    // A ground plane plus a dark canopy plate overhead: the camera beneath it sees the plate's
    // underside — a down-facing surface lit by the ground bounce alone, the deepest shade an
    // outdoor scene produces. The camera looks out toward the low sun, so the sky pass puts the
    // sun disc (the brightest thing the renderer draws) in the same frame.
    let mut vertices = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut quad = |corners: [[f32; 3]; 4], n: [f32; 3], c: [f32; 3], double: bool| {
        let base = vertices.len() as u32;
        vertices.extend(corners.map(|p| SceneVertex::new(p, n, c)));
        indices.extend([0, 2, 1, 0, 3, 2].map(|i| base + i));
        if double {
            indices.extend([0, 1, 2, 0, 2, 3].map(|i| base + i));
        }
    };
    quad(
        [[-30.0, 0.0, -30.0], [30.0, 0.0, -30.0], [30.0, 0.0, 30.0], [-30.0, 0.0, 30.0]],
        [0.0, 1.0, 0.0],
        [0.35, 0.45, 0.28],
        false,
    );
    // The canopy: double-sided so it renders from below and casts into the shadow map.
    quad(
        [[-6.0, 2.6, -2.0], [6.0, 2.6, -2.0], [6.0, 2.6, 8.0], [-6.0, 2.6, 8.0]],
        [0.0, -1.0, 0.0],
        [0.30, 0.30, 0.30],
        true,
    );

    let camera = renderer_api::Camera {
        eye: [0.0, 1.2, 6.0],
        target: [0.0, 2.0, -6.0],
        vertical_fov_degrees: 55.0,
    };
    let view_proj = view_projection_matrix(&camera, 1.0, 0.1, 200.0);

    let target = OffscreenTarget::new(&ctx, 128, 128).expect("target");
    let mut renderer = SceneRenderer::for_offscreen(&ctx, &vertices, &indices).expect("renderer");
    let mut lighting = SceneLighting::battlefield_default();
    // The sun sits low ahead of the camera (which looks toward -Z), so its disc is in frame.
    lighting.key_direction = [0.0, 0.30, -1.0];
    renderer.scene_lighting = lighting;
    renderer.shadow_focus = Some([0.0, 0.0, 0.0]);
    renderer.render(&ctx, target.render_target(), view_proj, camera.eye).expect("render");
    let pixels = target.read_rgba8(&ctx).expect("readback");

    let min = pixels.chunks_exact(4).map(luma).fold(f32::MAX, f32::min);
    let max = pixels.chunks_exact(4).map(luma).fold(0.0_f32, f32::max);
    assert!(
        min <= 0.06,
        "deep shade must reach near-black (the black point's contract), got min luma {min:.4}"
    );
    assert!(
        max >= 0.85,
        "the sun/highlights must stay bright through the grade, got max luma {max:.4}"
    );
}

#[test]
fn cloud_shade_darkens_the_terrain_key_and_strength_zero_is_a_noop() {
    let Some(ctx) = (match GpuContext::headless() {
        Ok(ctx) => Some(ctx),
        Err(error) => {
            eprintln!("skipping cloud shade test: {error}");
            None
        }
    }) else {
        return;
    };
    // A plain sunlit ground plane; with the coverage biased into a full lid and the shade
    // strength up, the terrain's key must dim everywhere the sun previously hit.
    let n = [0.0, 1.0, 0.0];
    let c = [0.35, 0.45, 0.28];
    let v = |x: f32, z: f32| SceneVertex::new([x, 0.0, z], n, c);
    let vertices = vec![v(-20.0, -20.0), v(20.0, -20.0), v(20.0, 20.0), v(-20.0, 20.0)];
    let indices = vec![0u32, 2, 1, 0, 3, 2];
    let camera = renderer_api::Camera {
        eye: [0.0, 6.0, 10.0],
        target: [0.0, 0.0, 0.0],
        vertical_fov_degrees: 55.0,
    };
    let view_proj = view_projection_matrix(&camera, 1.0, 0.1, 100.0);

    let render = |strength: f32| {
        let target = OffscreenTarget::new(&ctx, 96, 96).expect("target");
        // Feature test: explicit discrete quality (F3 folds this machine's tier otherwise).
        let mut renderer = SceneRenderer::for_offscreen_with_quality(
            &ctx,
            &vertices,
            &indices,
            renderer_api::LightingQuality::for_device_type(
                renderer_api::GpuDeviceType::DiscreteGpu,
            ),
        )
        .expect("renderer");
        let mut lighting = SceneLighting::battlefield_default();
        lighting.cloud_coverage_bias = 1.0;
        lighting.cloud_shadow_strength = strength;
        renderer.scene_lighting = lighting;
        renderer.shadow_focus = Some([0.0, 0.0, 0.0]);
        renderer.render(&ctx, target.render_target(), view_proj, camera.eye).expect("render");
        target.read_rgba8(&ctx).expect("readback")
    };

    let open = render(0.0);
    let shaded = render(0.8);
    let darkened = open
        .chunks_exact(4)
        .zip(shaded.chunks_exact(4))
        .filter(|(a, b)| luma(a) - luma(b) > 10.0 / 255.0)
        .count();
    assert!(
        darkened > 2000,
        "a full cloud lid at strength 0.8 must dim the sunlit ground broadly ({darkened} px)"
    );
    // And strength 0 twice in a row is deterministic — the gate really skips the modulation.
    let open_again = render(0.0);
    assert_eq!(open, open_again, "cloud shade strength 0 must be a stable no-op");
}
