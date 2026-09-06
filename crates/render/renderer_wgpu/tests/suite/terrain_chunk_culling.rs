//! The renderer half of terrain chunk culling: the terrain slot really is split into spatial
//! chunks, a battle camera really draws only a fraction of them, and the chunked path renders
//! cleanly end-to-end. The byte-exact image lock lives in the client's `look_goldens` test —
//! pure-culling changes must leave every golden frame byte-identical.

use renderer_api::{Camera, SceneVertex, view_projection_matrix};
use renderer_wgpu::{GpuContext, OffscreenTarget, SceneRenderer};

fn headless_context() -> Option<GpuContext> {
    match GpuContext::headless() {
        Ok(ctx) => Some(ctx),
        Err(error) => {
            eprintln!("skipping terrain chunk test: {error}");
            None
        }
    }
}

/// A synthetic 1000 m steppe: a flat-ish heightfield grid, the shape of the real battlefield
/// meshes without dragging the terrain crate into a renderer test.
fn steppe_mesh(cells: usize, size_m: f32) -> (Vec<SceneVertex>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let step = size_m / cells as f32;
    for gz in 0..=cells {
        for gx in 0..=cells {
            vertices.push(SceneVertex::new(
                [gx as f32 * step, ((gx * 3 + gz * 5) % 7) as f32 * 0.4, gz as f32 * step],
                [0.0, 1.0, 0.0],
                [0.3, 0.33, 0.22],
            ));
        }
    }
    let row = (cells + 1) as u32;
    for gz in 0..cells as u32 {
        for gx in 0..cells as u32 {
            let a = gz * row + gx;
            indices.extend_from_slice(&[a, a + row, a + 1, a + 1, a + row, a + row + 1]);
        }
    }
    (vertices, indices)
}

#[test]
fn a_battle_camera_draws_a_fraction_of_the_terrain_chunks() {
    let Some(ctx) = headless_context() else {
        return;
    };
    let (vertices, indices) = steppe_mesh(100, 1000.0);
    let renderer = SceneRenderer::for_offscreen(&ctx, &vertices, &indices).expect("renderer");

    // The 1000 m mesh splits into a real chunk grid, and the reorder loses no triangle.
    assert!(
        renderer.terrain_chunk_count() > 100,
        "a 1000 m map must chunk into a real grid, got {}",
        renderer.terrain_chunk_count()
    );
    assert_eq!(renderer.terrain_index_count(), indices.len() as u32);

    // A mid-map chase camera looking across the field: everything behind it and outside its
    // wedge stops costing vertex work.
    let camera = Camera {
        eye: [500.0, 12.0, 500.0],
        target: [700.0, 2.0, 500.0],
        vertical_fov_degrees: 55.0,
    };
    let view_proj = view_projection_matrix(&camera, 16.0 / 9.0, 0.1, 2000.0);
    let visible = renderer.visible_terrain_chunks(&view_proj);
    assert!(visible > 0, "the camera must see the field ahead");
    assert!(
        (visible as f32) < renderer.terrain_chunk_count() as f32 * 0.5,
        "a mid-map camera must cull over half the chunks: {visible}/{}",
        renderer.terrain_chunk_count()
    );
}

#[test]
fn the_chunked_terrain_renders_end_to_end_and_swaps_cleanly() {
    let Some(ctx) = headless_context() else {
        return;
    };
    let (vertices, indices) = steppe_mesh(60, 600.0);
    let target = OffscreenTarget::new(&ctx, 96, 54).expect("target");
    let mut renderer = SceneRenderer::for_offscreen(&ctx, &vertices, &indices).expect("renderer");

    let camera = Camera {
        eye: [300.0, 10.0, 300.0],
        target: [400.0, 0.0, 300.0],
        vertical_fov_degrees: 55.0,
    };
    let view_proj = view_projection_matrix(&camera, 96.0 / 54.0, 0.1, 2000.0);
    renderer.render(&ctx, target.render_target(), view_proj, camera.eye).expect("render");
    let pixels = target.read_rgba8(&ctx).expect("readback");
    assert!(pixels.iter().any(|&b| b != 0), "the chunked terrain must draw something");

    // Scene swap (garage <-> battle): set_terrain re-chunks the new geometry.
    let (small_vertices, small_indices) = steppe_mesh(10, 40.0);
    renderer.set_terrain(&ctx, &small_vertices, &small_indices);
    assert_eq!(renderer.terrain_index_count(), small_indices.len() as u32);
    assert!(renderer.terrain_chunk_count() >= 1);
    renderer.render(&ctx, target.render_target(), view_proj, camera.eye).expect("render again");
}

/// Żywy Step P2: the dressing slot culls by frustum AND by distance — a chunk past the
/// cards' collapse band (340 m) never draws even when the frustum sees it, and the slot
/// clears with empty slices (the garage has no meadow).
#[test]
fn the_dressing_slot_distance_cuts_past_the_collapse_band() {
    let Some(ctx) = headless_context() else {
        return;
    };
    let (vertices, indices) = steppe_mesh(4, 40.0);
    let mut renderer = SceneRenderer::for_offscreen(&ctx, &vertices, &indices).expect("renderer");
    let (mut meadow_v, meadow_i) = steppe_mesh(100, 1000.0);
    for vertex in &mut meadow_v {
        vertex.surface = renderer_api::surface_role::GRASS_CARD;
    }
    renderer.set_dressing(&ctx, &meadow_v, &meadow_i);

    let camera = Camera {
        eye: [500.0, 12.0, 500.0],
        target: [700.0, 2.0, 500.0],
        vertical_fov_degrees: 55.0,
    };
    let view_proj = view_projection_matrix(&camera, 16.0 / 9.0, 0.1, 2000.0);
    let near = renderer.visible_dressing_chunks(&view_proj, camera.eye);
    assert!(near > 0, "the meadow ahead draws");

    // The same wedge from the map corner: everything ahead lies past the cutoff.
    let far_camera = Camera {
        eye: [-400.0, 12.0, -400.0],
        target: [700.0, 2.0, 700.0],
        vertical_fov_degrees: 55.0,
    };
    let far_vp = view_projection_matrix(&far_camera, 16.0 / 9.0, 0.1, 4000.0);
    let far = renderer.visible_dressing_chunks(&far_vp, far_camera.eye);
    assert!(
        far < near,
        "distance cuts what the frustum alone would draw: far {far} vs near {near}"
    );
    for chunk_probe in [near, far] {
        let _ = chunk_probe;
    }
    // Every chunk the cutoff admits sits within 340 m + a chunk diagonal of the eye.
    // (The exact per-chunk assert lives in the visible count: with the eye at the map corner
    // and the whole field 550+ m away, nothing may draw.)
    let corner_camera = Camera {
        eye: [-600.0, 12.0, 500.0],
        target: [500.0, 2.0, 500.0],
        vertical_fov_degrees: 55.0,
    };
    let corner_vp = view_projection_matrix(&corner_camera, 16.0 / 9.0, 0.1, 4000.0);
    assert_eq!(
        renderer.visible_dressing_chunks(&corner_vp, corner_camera.eye),
        0,
        "a meadow 550+ m away has fully collapsed — nothing draws"
    );

    renderer.set_dressing(&ctx, &[], &[]);
    assert_eq!(
        renderer.visible_dressing_chunks(&view_proj, camera.eye),
        0,
        "empty slices clear the slot"
    );
}
