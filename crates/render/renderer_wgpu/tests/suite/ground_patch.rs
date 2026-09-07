//! T9 (the one program; Q1's terrain half): a crater no longer re-uploads the whole ground.
//! The base binds once; a crater CUTS its cells' triangles in place (twelve bytes a triangle)
//! and uploads the PATCH — the cut cells re-meshed — as its own chunked buffers, sized to the
//! patch. The instrument is the renderer's own count of the bytes each upload put on the GPU.

use std::time::Instant;

use super::common;
use renderer_api::{
    Camera, SceneVertex, TerrainGroundMaps, TerrainMaterialSet, view_projection_matrix,
};
use renderer_wgpu::{OffscreenTarget, SceneRenderer};

const CELLS: usize = 100;
const SIZE_M: f32 = 1000.0;

fn steppe_mesh() -> (Vec<SceneVertex>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let step = SIZE_M / CELLS as f32;
    for gz in 0..=CELLS {
        for gx in 0..=CELLS {
            vertices.push(SceneVertex::new(
                [gx as f32 * step, ((gx * 3 + gz * 5) % 7) as f32 * 0.4, gz as f32 * step],
                [0.0, 1.0, 0.0],
                [0.3, 0.33, 0.22],
            ));
        }
    }
    let row = (CELLS + 1) as u32;
    for gz in 0..CELLS as u32 {
        for gx in 0..CELLS as u32 {
            let a = gz * row + gx;
            indices.extend_from_slice(&[a, a + row, a + 1, a + 1, a + row, a + row + 1]);
        }
    }
    (vertices, indices)
}

/// Four cells at the map's middle re-meshed 8 × 8 finer, sunk a little: the shape of a crater
/// patch, and the ids of the eight base triangles it stands in for.
fn crater_patch() -> (Vec<SceneVertex>, Vec<u32>, Vec<u32>) {
    let step = SIZE_M / CELLS as f32;
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let mut cut = Vec::new();
    for (cx, cz) in [(50usize, 50usize), (51, 50), (50, 51), (51, 51)] {
        let cell = cz * CELLS + cx;
        cut.extend_from_slice(&[(cell * 2) as u32, (cell * 2 + 1) as u32]);
        let base = vertices.len() as u32;
        for sz in 0..=8 {
            for sx in 0..=8 {
                let x = cx as f32 * step + sx as f32 * step / 8.0;
                let z = cz as f32 * step + sz as f32 * step / 8.0;
                let d = ((x - 510.0).powi(2) + (z - 510.0).powi(2)).sqrt();
                let y = -(1.0 - (d / 6.0).min(1.0)).powi(2) * 0.8;
                vertices.push(SceneVertex::new([x, y, z], [0.0, 1.0, 0.0], [0.2, 0.15, 0.1]));
            }
        }
        for sz in 0..8u32 {
            for sx in 0..8u32 {
                let a = base + sz * 9 + sx;
                indices.extend_from_slice(&[a, a + 9, a + 1, a + 1, a + 9, a + 10]);
            }
        }
    }
    (vertices, indices, cut)
}

fn uniform_grass_maps() -> TerrainGroundMaps {
    let size = 16u32;
    let texels = (size * size) as usize;
    TerrainGroundMaps {
        size,
        splat: [255u8, 0, 0, 0].repeat(texels),
        macro_normal: [128u8, 255, 128, 0].repeat(texels),
        extent_m: [SIZE_M, SIZE_M],
    }
}

/// A crater costs the patch and the cuts, never the field: the bytes the renderer puts on the
/// GPU for a crater are the patch's own (plus twelve a cut triangle), under a twentieth of the
/// bind, and the frame still draws — the patch's chunks culled like the base's.
#[test]
fn a_crater_uploads_the_patch_and_the_cuts_never_the_whole_ground() {
    let Some(ctx) = common::headless("ground patch test") else {
        return;
    };
    let (vertices, indices) = steppe_mesh();
    let far = [[-900.0, -50.0, -900.0], [-899.0, -50.0, -900.0], [-900.0, -50.0, -899.0]]
        .map(|p| SceneVertex::new(p, [0.0, 1.0, 0.0], [0.3; 3]))
        .to_vec();
    let mut renderer = SceneRenderer::for_offscreen(&ctx, &far, &[0, 1, 2]).expect("renderer");
    let materials = TerrainMaterialSet::default();
    renderer.set_battlefield_ground(&ctx, &vertices, &indices, &uniform_grass_maps(), &materials);
    let bind_bytes = renderer.last_ground_upload_bytes();
    assert!(bind_bytes >= vertices.len() * std::mem::size_of::<SceneVertex>());

    let (patch_v, patch_i, cut) = crater_patch();
    let patch_bytes = patch_v.len() * std::mem::size_of::<SceneVertex>() + patch_i.len() * 4;

    // The old path, for the record: the whole field again.
    let whole = Instant::now();
    renderer.update_battlefield_ground_geometry(&ctx, &vertices, &indices);
    let whole_ms = whole.elapsed().as_secs_f32() * 1000.0;
    assert_eq!(renderer.last_ground_upload_bytes(), bind_bytes, "the whole field again");

    // The patch path: the cuts, then the patch.
    let patch = Instant::now();
    renderer.cut_ground_triangles(&ctx, &cut);
    assert_eq!(renderer.last_ground_upload_bytes(), cut.len() * 12, "twelve bytes a cut");
    renderer.cut_ground_triangles(&ctx, &cut);
    assert_eq!(renderer.last_ground_upload_bytes(), 0, "a cut is idempotent");
    renderer.set_ground_patch(&ctx, &patch_v, &patch_i);
    let patch_ms = patch.elapsed().as_secs_f32() * 1000.0;
    let uploaded = renderer.last_ground_upload_bytes();
    assert!(uploaded <= patch_bytes, "the patch and nothing else: {uploaded} of {patch_bytes}");
    assert!(
        uploaded * 20 <= bind_bytes,
        "under a twentieth of the bind: {uploaded} vs {bind_bytes}"
    );
    assert!(renderer.ground_patch_chunk_count() >= 1);
    println!(
        "GROUND PATCH: bind {bind_bytes} B; a crater re-upload {bind_bytes} B in {whole_ms:.2} ms \
         (the old path) vs cuts {} B + patch {uploaded} B in {patch_ms:.2} ms",
        cut.len() * 12
    );

    // And the frame still draws, the patch culled with the base.
    let camera = Camera {
        eye: [480.0, 12.0, 480.0],
        target: [520.0, 0.0, 520.0],
        vertical_fov_degrees: 55.0,
    };
    let view_proj = view_projection_matrix(&camera, 1.5, 0.1, 1500.0);
    let target = OffscreenTarget::new(&ctx, 192, 128).expect("target");
    renderer.render(&ctx, target.render_target(), view_proj, camera.eye).expect("render");
    let away = Camera { eye: [20.0, 12.0, 20.0], target: [0.0, 0.0, 0.0], ..camera };
    let away_vp = view_projection_matrix(&away, 1.5, 0.1, 1500.0);
    renderer.render(&ctx, target.render_target(), away_vp, away.eye).expect("render");
    renderer.set_ground_patch(&ctx, &[], &[]);
    assert_eq!(renderer.ground_patch_chunk_count(), 0, "empty slices clear the patch");
}
