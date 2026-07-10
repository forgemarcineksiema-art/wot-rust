//! Locks the HUD upload budget: the buffer holds the full battle HUD (reticle, bars, readouts,
//! damage log, ammo panel and minimap need well over the old 2048-vertex cap), and an oversized
//! upload degrades by truncating to whole triangles instead of silently blanking the frame.

use renderer_api::HudVertex;
use renderer_wgpu::{GpuContext, SceneRenderer};

fn headless_context() -> Option<GpuContext> {
    match GpuContext::headless() {
        Ok(ctx) => Some(ctx),
        Err(error) => {
            eprintln!("skipping hud budget test: {error}");
            None
        }
    }
}

fn hud_vertices(count: usize) -> Vec<HudVertex> {
    (0..count).map(|_| HudVertex::new([0.0, 0.0], [1.0, 1.0, 1.0, 1.0])).collect()
}

#[test]
fn the_hud_buffer_holds_a_full_battle_hud() {
    let Some(ctx) = headless_context() else {
        return;
    };
    let mut renderer = SceneRenderer::for_offscreen(&ctx, &[], &[]).expect("renderer");

    // 12000 vertices: a busy frame with the 36x36-cell minimap, drop-shadowed text, damage
    // log and ammo panel — the 8192-vertex budget cut the minimap's blips off mid-frame.
    let busy_frame = hud_vertices(12000);
    renderer.set_hud(&ctx, &busy_frame);
    assert_eq!(renderer.hud_vertex_count(), 12000, "a busy battle HUD must upload untruncated");
}

#[test]
fn an_oversized_hud_upload_truncates_to_whole_triangles_instead_of_blanking() {
    let Some(ctx) = headless_context() else {
        return;
    };
    let mut renderer = SceneRenderer::for_offscreen(&ctx, &[], &[]).expect("renderer");

    let oversized = hud_vertices(20000);
    renderer.set_hud(&ctx, &oversized);

    let kept = renderer.hud_vertex_count();
    assert!(kept > 0, "an oversized upload must keep the leading vertices, not blank the HUD");
    assert!((kept as usize) < oversized.len(), "an oversized upload must be truncated");
    assert_eq!(kept % 3, 0, "truncation must land on a whole triangle");
}
