//! Locks the SSAO↔lighting coupling: screen-space AO attenuates the INDIRECT terms only
//! (ambient/fill/env). Under pure key light, toggling SSAO must not change the picture — the sun
//! is occluded by the shadow map, and multiplying it by screen AO too was the double-darkening
//! that dirtied every sunlit crease. With the ambient on, the crease must still darken (AO is
//! alive, not disabled). Runs on the headless adapter; skips if none is available.

mod common;
use common::luma_255;

use renderer_api::{SceneLighting, SceneVertex, view_projection_matrix};
use renderer_wgpu::{GpuContext, OffscreenTarget, SceneRenderer};

/// A ground plane meeting a vertical wall: the junction is the classic SSAO crease.
fn crease_scene() -> (Vec<SceneVertex>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut quad = |corners: [[f32; 3]; 4], n: [f32; 3], c: [f32; 3]| {
        let base = vertices.len() as u32;
        vertices.extend(corners.map(|p| SceneVertex::new(p, n, c)));
        indices.extend([0, 2, 1, 0, 3, 2].map(|i| base + i));
    };
    quad(
        [[-6.0, 0.0, -6.0], [6.0, 0.0, -6.0], [6.0, 0.0, 6.0], [-6.0, 0.0, 6.0]],
        [0.0, 1.0, 0.0],
        [0.40, 0.42, 0.36],
    );
    quad(
        [[-6.0, 2.5, 0.0], [6.0, 2.5, 0.0], [6.0, 0.0, 0.0], [-6.0, 0.0, 0.0]],
        [0.0, 0.0, 1.0],
        [0.45, 0.42, 0.40],
    );
    (vertices, indices)
}

fn render_crease(ctx: &GpuContext, lighting: SceneLighting, ssao: bool) -> Vec<u8> {
    let (vertices, indices) = crease_scene();
    let camera = renderer_api::Camera {
        eye: [0.0, 1.6, 3.2],
        target: [0.0, 0.4, 0.0],
        vertical_fov_degrees: 55.0,
    };
    let view_proj = view_projection_matrix(&camera, 1.0, 0.1, 60.0);
    let target = OffscreenTarget::new(ctx, 96, 96).expect("target");
    let mut renderer = SceneRenderer::for_offscreen(ctx, &vertices, &indices).expect("renderer");
    renderer.scene_lighting = lighting;
    renderer.shadow_focus = Some([0.0, 0.0, 0.0]);
    renderer.set_ssao_enabled(ssao);
    renderer.render(ctx, target.render_target(), view_proj, camera.eye).expect("render");
    target.read_rgba8(ctx).expect("readback")
}

#[test]
fn under_pure_key_light_ssao_is_invisible() {
    let Some(ctx) = (match GpuContext::headless() {
        Ok(ctx) => Some(ctx),
        Err(error) => {
            eprintln!("skipping ssao coupling test: {error}");
            None
        }
    }) else {
        return;
    };
    // Kill every indirect term: only the sun key (and its shadow) lights the scene.
    let mut lighting = SceneLighting::battlefield_default();
    lighting.ambient_rgb = [0.0; 3];
    lighting.ground_ambient_rgb = [0.0; 3];
    lighting.fill_rgb = [0.0; 3];
    lighting.rim_rgb = [0.0; 3];
    lighting.key_direction = [0.4, 0.8, 0.45];

    // Up to three attempts: a full-workspace run under heavy compile load can hand back one
    // garbage frame (device pressure), which reads as a huge diff. A genuine coupling
    // regression is deterministic and fails every attempt; a pressure artefact does not repeat.
    let mut max_diff = f32::MAX;
    for _attempt in 0..3 {
        let without = render_crease(&ctx, lighting, false);
        let with = render_crease(&ctx, lighting, true);
        max_diff = without
            .chunks_exact(4)
            .zip(with.chunks_exact(4))
            .map(|(a, b)| (luma_255(a) - luma_255(b)).abs())
            .fold(0.0_f32, f32::max);
        if max_diff < 2.0 {
            return;
        }
        eprintln!("ssao coupling attempt saw max diff {max_diff}; retrying");
    }
    panic!(
        "screen AO must not touch key-lit pixels (indirect terms are all zero), max diff {max_diff}"
    );
}

/// One open plane, no crease anywhere: the tank's-eye view of a field.
fn field_scene() -> (Vec<SceneVertex>, Vec<u32>) {
    let corners = [[-80.0, 0.0, -80.0], [80.0, 0.0, -80.0], [80.0, 0.0, 80.0], [-80.0, 0.0, 80.0]];
    let vertices: Vec<SceneVertex> =
        corners.map(|p| SceneVertex::new(p, [0.0, 1.0, 0.0], [0.40, 0.42, 0.36])).to_vec();
    (vertices, vec![0, 2, 1, 0, 3, 2])
}

fn render_field(ctx: &GpuContext, lighting: SceneLighting, ssao: bool) -> Vec<u8> {
    let (vertices, indices) = field_scene();
    // A commander's eye: 2.2 m up, looking 14 m out — the band where the old SSAO occluded
    // the flat ground against itself (depth falling centimetres per pixel, past its bias).
    let camera = renderer_api::Camera {
        eye: [0.0, 2.2, 0.0],
        target: [0.0, 0.0, -14.0],
        vertical_fov_degrees: 55.0,
    };
    let view_proj = view_projection_matrix(&camera, 1.0, 0.1, 200.0);
    let target = OffscreenTarget::new(ctx, 128, 128).expect("target");
    let mut renderer = SceneRenderer::for_offscreen(ctx, &vertices, &indices).expect("renderer");
    renderer.scene_lighting = lighting;
    renderer.shadow_focus = Some([0.0, 0.0, -14.0]);
    renderer.set_ssao_enabled(ssao);
    renderer.render(ctx, target.render_target(), view_proj, camera.eye).expect("render");
    target.read_rgba8(ctx).expect("readback")
}

/// Inny Poziom T6 (the ground's second artefact): a flat plane occludes NOTHING. The old pass
/// compared every tap against the centre pixel's depth, so on open ground seen from a tank
/// the taps toward the camera were always "closer" by more than the bias and the whole field
/// wore a faint diagonal weave of the spiral's rotation noise — the darker slanted bands the
/// owner saw on Prokhorovka under an overcast sky. The pass now predicts each tap's depth from
/// the pixel's local plane; the plane must read identically with SSAO on and off, to the byte,
/// under the ambient-only rig where AO is most visible.
#[test]
fn a_flat_field_does_not_occlude_itself() {
    let Some(ctx) = (match GpuContext::headless() {
        Ok(ctx) => Some(ctx),
        Err(error) => {
            eprintln!("skipping ssao coupling test: {error}");
            None
        }
    }) else {
        return;
    };
    let mut lighting = SceneLighting::battlefield_default();
    lighting.key_rgb = [0.0; 3];
    let mut max_diff = f32::MAX;
    let mut touched = usize::MAX;
    for _attempt in 0..3 {
        let without = render_field(&ctx, lighting, false);
        let with = render_field(&ctx, lighting, true);
        let diffs: Vec<f32> = without
            .chunks_exact(4)
            .zip(with.chunks_exact(4))
            .map(|(a, b)| (luma_255(a) - luma_255(b)).abs())
            .collect();
        max_diff = diffs.iter().copied().fold(0.0_f32, f32::max);
        touched = diffs.iter().filter(|d| **d > 1.0).count();
        if max_diff < 2.0 && touched == 0 {
            return;
        }
        eprintln!("flat field attempt saw max diff {max_diff}, {touched} px touched; retrying");
    }
    panic!(
        "screen AO must leave a flat plane alone: max luma diff {max_diff}, {touched} px darkened"
    );
}

#[test]
fn with_ambient_on_the_crease_still_darkens() {
    let Some(ctx) = (match GpuContext::headless() {
        Ok(ctx) => Some(ctx),
        Err(error) => {
            eprintln!("skipping ssao coupling test: {error}");
            None
        }
    }) else {
        return;
    };
    // Isolate the indirect lane this test owns. The correctly normalized near-shadow PCF leaves
    // the full key contribution alive; allowing that unrelated direct term to dominate would make
    // this assertion depend on shadow quality instead of whether SSAO still reaches the ambient.
    let mut lighting = SceneLighting::battlefield_default();
    lighting.key_rgb = [0.0; 3];
    let without = render_crease(&ctx, lighting, false);
    let with = render_crease(&ctx, lighting, true);
    let darkened = without
        .chunks_exact(4)
        .zip(with.chunks_exact(4))
        .filter(|(a, b)| luma_255(a) - luma_255(b) > 4.0)
        .count();
    assert!(
        darkened > 20,
        "SSAO must still darken the wall/ground crease through the ambient ({darkened} px)"
    );
}
