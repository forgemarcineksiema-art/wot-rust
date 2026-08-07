//! Locks for the backend-neutral culling module: frustum extraction from this project's
//! column-major `[0, 1]`-depth matrices (perspective camera AND orthographic sun box), the
//! conservative AABB test, and the deterministic scene chunking the renderer's per-pass
//! visibility rests on. No GPU anywhere — the byte-exact golden screenshots are the
//! integration lock (a culling bug that clips a visible chunk fails them immediately).

use renderer_api::{
    Aabb, Camera, Frustum, SceneVertex, SunShadowParams, chunk_scene_indices,
    sun_light_view_projection, view_projection_matrix,
};

fn camera_frustum(eye: [f32; 3], target: [f32; 3]) -> Frustum {
    let camera = Camera { eye, target, vertical_fov_degrees: 55.0 };
    Frustum::from_view_proj(&view_projection_matrix(&camera, 16.0 / 9.0, 0.1, 2000.0))
}

fn unit_box_at(center: [f32; 3], half: f32) -> Aabb {
    Aabb {
        min: [center[0] - half, center[1] - half, center[2] - half],
        max: [center[0] + half, center[1] + half, center[2] + half],
    }
}

#[test]
fn camera_frustum_keeps_what_it_looks_at_and_culls_the_rest() {
    let frustum = camera_frustum([0.0, 10.0, 0.0], [100.0, 0.0, 0.0]);
    // Straight ahead: visible.
    assert!(frustum.intersects_aabb(&unit_box_at([100.0, 0.0, 0.0], 5.0)));
    // Behind the camera: culled.
    assert!(!frustum.intersects_aabb(&unit_box_at([-100.0, 0.0, 0.0], 5.0)));
    // Far off to the side, outside the horizontal FOV: culled.
    assert!(!frustum.intersects_aabb(&unit_box_at([50.0, 0.0, 400.0], 5.0)));
    // Beyond the far plane: culled.
    assert!(!frustum.intersects_aabb(&unit_box_at([5000.0, 0.0, 0.0], 5.0)));
    // High above the look direction, outside the vertical FOV: culled.
    assert!(!frustum.intersects_aabb(&unit_box_at([50.0, 500.0, 0.0], 5.0)));
}

#[test]
fn a_box_containing_the_camera_is_always_visible() {
    let frustum = camera_frustum([500.0, 5.0, 500.0], [600.0, 0.0, 500.0]);
    assert!(frustum.intersects_aabb(&unit_box_at([500.0, 5.0, 500.0], 50.0)));
}

#[test]
fn a_box_straddling_a_frustum_edge_is_kept_conservatively() {
    let frustum = camera_frustum([0.0, 10.0, 0.0], [100.0, 0.0, 0.0]);
    // A wide box whose centre is off-screen but whose near corner pokes into the view: the
    // conservative test must keep it (never cull a partially visible chunk).
    let straddling = Aabb { min: [40.0, -5.0, 30.0], max: [60.0, 5.0, 300.0] };
    assert!(frustum.intersects_aabb(&straddling));
    // The empty box is never visible.
    assert!(!frustum.intersects_aabb(&Aabb::EMPTY));
}

#[test]
fn the_sun_shadow_box_culls_terrain_outside_its_footprint() {
    // The orthographic near-cascade light box focused on mid-map: the same plane extraction
    // must work for it (the shadow passes cull by exactly this frustum).
    let light = sun_light_view_projection(
        [0.62, 0.52, 0.34],
        [500.0, 5.0, 500.0],
        SunShadowParams::default(),
    );
    let frustum = Frustum::from_view_proj(&light);
    // Ground at the focus: inside the box.
    assert!(frustum.intersects_aabb(&unit_box_at([500.0, 0.0, 500.0], 10.0)));
    // A map corner ~700 m away: far outside the focused footprint.
    assert!(!frustum.intersects_aabb(&unit_box_at([0.0, 0.0, 0.0], 10.0)));
}

/// A flat grid of quads spanning `size_m`, one quad per metre-ish cell — a miniature terrain.
fn grid_mesh(cells: usize, size_m: f32) -> (Vec<SceneVertex>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let step = size_m / cells as f32;
    for gz in 0..=cells {
        for gx in 0..=cells {
            vertices.push(SceneVertex::new(
                [gx as f32 * step, ((gx * 7 + gz * 13) % 5) as f32 * 0.5, gz as f32 * step],
                [0.0, 1.0, 0.0],
                [0.3, 0.33, 0.22],
            ));
        }
    }
    let row = cells + 1;
    for gz in 0..cells {
        for gx in 0..cells {
            let a = (gz * row + gx) as u32;
            let b = a + 1;
            let c = a + row as u32;
            let d = c + 1;
            indices.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }
    (vertices, indices)
}

#[test]
fn chunking_preserves_every_triangle_exactly_once() {
    let (vertices, indices) = grid_mesh(20, 200.0);
    let (reordered, chunks) = chunk_scene_indices(&vertices, &indices, 50.0);
    assert_eq!(reordered.len(), indices.len(), "no triangle appears or disappears");
    // Chunk ranges are contiguous, disjoint, and cover the whole buffer.
    let mut cursor = 0u32;
    for chunk in &chunks {
        assert_eq!(chunk.index_start, cursor, "chunks are contiguous");
        assert_eq!(chunk.index_count % 3, 0, "chunks hold whole triangles");
        cursor += chunk.index_count;
    }
    assert_eq!(cursor as usize, reordered.len(), "chunks cover the whole buffer");
    // The reordered buffer is a permutation of the original triangles.
    let tri_key = |ix: &[u32]| {
        let mut t = [ix[0], ix[1], ix[2]];
        t.sort_unstable();
        t
    };
    let mut original: Vec<_> = indices.chunks_exact(3).map(tri_key).collect();
    let mut shuffled: Vec<_> = reordered.chunks_exact(3).map(tri_key).collect();
    original.sort_unstable();
    shuffled.sort_unstable();
    assert_eq!(original, shuffled, "chunking is a pure permutation of triangles");
    // A 200 m mesh at 50 m chunks: a real grid of chunks, not one blob.
    assert_eq!(chunks.len(), 16, "4x4 chunk grid expected");
}

#[test]
fn every_chunk_aabb_contains_all_its_triangles() {
    let (vertices, indices) = grid_mesh(16, 160.0);
    let (reordered, chunks) = chunk_scene_indices(&vertices, &indices, 40.0);
    for chunk in &chunks {
        assert!(!chunk.aabb.is_empty());
        let range = &reordered
            [chunk.index_start as usize..(chunk.index_start + chunk.index_count) as usize];
        for &i in range {
            let p = vertices[i as usize].position;
            for axis in 0..3 {
                assert!(
                    p[axis] >= chunk.aabb.min[axis] - 1.0e-4
                        && p[axis] <= chunk.aabb.max[axis] + 1.0e-4,
                    "vertex {p:?} escapes its chunk box {:?}",
                    chunk.aabb
                );
            }
        }
    }
}

#[test]
fn chunking_is_deterministic() {
    let (vertices, indices) = grid_mesh(12, 120.0);
    let a = chunk_scene_indices(&vertices, &indices, 30.0);
    let b = chunk_scene_indices(&vertices, &indices, 30.0);
    assert_eq!(a.0, b.0);
    assert_eq!(a.1, b.1);
}

#[test]
fn chunk_culling_from_a_mid_map_camera_actually_culls() {
    // The point of the whole module: the battle camera stands IN the field — everything behind
    // it and outside its ~85 degree horizontal wedge must stop costing vertex work. This is
    // the CPU stand-in for the on-screen budget the renderer gains from per-chunk draws.
    let (vertices, indices) = grid_mesh(40, 1000.0);
    let (_, chunks) = chunk_scene_indices(&vertices, &indices, 80.0);
    let frustum = camera_frustum([500.0, 12.0, 500.0], [700.0, 0.0, 500.0]);
    let visible = chunks.iter().filter(|c| frustum.intersects_aabb(&c.aabb)).count();
    assert!(visible > 0, "the camera must see something");
    assert!(
        (visible as f32) < chunks.len() as f32 * 0.5,
        "a mid-map camera must cull more than half the map: {visible}/{}",
        chunks.len()
    );
}

/// The fingerprint's whole job is to answer "is this the mesh already uploaded?" without the
/// render thread comparing tens of megabytes. Equal meshes MUST agree — otherwise the answer is
/// always "changed" and the optimisation it exists for never fires.
#[test]
fn a_mesh_fingerprints_the_same_every_time_it_is_rebuilt() {
    let (vertices, indices) = grid_mesh(24, 240.0);
    let (again_v, again_i) = grid_mesh(24, 240.0);
    assert_eq!(
        renderer_api::scene_mesh_fingerprint(&vertices, &indices),
        renderer_api::scene_mesh_fingerprint(&again_v, &again_i),
        "a deterministic rebake fingerprinted differently; nothing downstream could ever skip",
    );
}

/// And the half that makes skipping safe: a mesh that differs ANYWHERE must fingerprint
/// differently. Each case below is a change a crater rebake can really produce — a moved vertex
/// (ground pushed down), a recoloured one (a card mowed), a shorter mesh (cards removed), and a
/// rewound triangle (the same vertices facing the other way).
#[test]
fn any_difference_in_a_mesh_moves_its_fingerprint() {
    let (vertices, indices) = grid_mesh(12, 120.0);
    let baseline = renderer_api::scene_mesh_fingerprint(&vertices, &indices);

    let mut moved = vertices.clone();
    moved[17].position[1] += 0.01;
    assert_ne!(baseline, renderer_api::scene_mesh_fingerprint(&moved, &indices), "moved vertex");

    let mut tinted = vertices.clone();
    tinted[3].color[0] += 0.02;
    assert_ne!(baseline, renderer_api::scene_mesh_fingerprint(&tinted, &indices), "recolored");

    let mut shorter = vertices.clone();
    shorter.pop();
    let trimmed: Vec<u32> =
        indices.iter().copied().filter(|i| (*i as usize) < shorter.len()).collect();
    assert_ne!(
        baseline,
        renderer_api::scene_mesh_fingerprint(&shorter, &trimmed),
        "a mesh that lost geometry",
    );

    let mut rewound = indices.clone();
    rewound.swap(0, 2);
    assert_ne!(
        baseline,
        renderer_api::scene_mesh_fingerprint(&vertices, &rewound),
        "same vertices, triangle facing the other way",
    );
}
