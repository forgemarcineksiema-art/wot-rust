//! What a frame submitted, per pass — the half of the instrument that needs no GPU feature.
//!
//! The numbers here are EXACT, not ranges. A counter that is allowed to be approximately right is
//! worse than no counter: it will be quoted, and nobody will know by how much it was wrong. The
//! scene below is small enough that every draw in it can be reasoned about by hand.

use game_core::TankId;
use renderer_api::{
    Camera, MaterialHandle, MeshAsset, MeshHandle, RenderFrame, RenderObject, SceneVertex,
    view_projection_matrix,
};
use renderer_wgpu::{GpuContext, OffscreenTarget, PassId, SceneRenderer};

/// Four triangles, so a miscount by a factor of three cannot hide behind a one-triangle mesh.
const TRIANGLES_PER_MESH: u64 = 4;
/// Three objects sharing one mesh AND one material, so they must collapse into ONE instanced
/// draw. This is the lesson the fleet's "2302 objects, 66 draws" reading taught, asserted.
const OBJECTS: u64 = 3;

#[test]
fn the_counters_are_exact_for_a_known_scene() {
    let Ok(ctx) = GpuContext::headless() else {
        eprintln!("skipping pass counts test: no headless adapter");
        return;
    };
    let target = OffscreenTarget::new(&ctx, 64, 64).expect("target");
    let mut renderer = SceneRenderer::for_offscreen(&ctx, &[], &[]).expect("renderer");
    let mesh = MeshHandle(7);

    // A fan of four triangles around the origin: 6 vertices, 12 indices.
    let vertices: Vec<SceneVertex> = [
        [0.0, 0.0, 0.0],
        [-0.6, -0.4, 0.0],
        [0.6, -0.4, 0.0],
        [0.6, 0.4, 0.0],
        [-0.6, 0.4, 0.0],
        [0.0, 0.7, 0.0],
    ]
    .into_iter()
    .map(|p| SceneVertex::new(p, [0.0, 0.0, 1.0], [0.9, 0.2, 0.1]))
    .collect();
    let indices = vec![0, 1, 2, 0, 2, 3, 0, 3, 4, 0, 4, 5];
    assert_eq!(indices.len() as u64 / 3, TRIANGLES_PER_MESH, "the fixture must be four triangles");
    renderer.register_mesh(&ctx, mesh, &MeshAsset::new(vertices, indices));

    let camera =
        Camera { eye: [0.0, 0.0, 3.0], target: [0.0, 0.0, 0.0], vertical_fov_degrees: 55.0 };
    renderer.set_render_frame(
        &ctx,
        &RenderFrame {
            camera,
            objects: (0..OBJECTS)
                .map(|i| RenderObject {
                    tank_id: Some(TankId(i + 1)),
                    mesh,
                    material: MaterialHandle(0),
                    transform: translation(i as f32 * 0.01),
                    tint: [1.0, 1.0, 1.0],
                })
                .collect(),
            armor_damage: Vec::new(),
        },
    );
    renderer
        .render(
            &ctx,
            target.render_target(),
            view_projection_matrix(&camera, 1.0, 0.1, 20.0),
            camera.eye,
        )
        .expect("render");

    let counts = renderer.last_frame_counts();
    for id in PassId::ALL {
        let pass = counts.pass(*id);
        if pass.draws > 0 {
            eprintln!(
                "{:<18} {:>3} draws  {:>5} tris  {:>3} inst",
                id.label(),
                pass.draws,
                pass.triangles,
                pass.instances
            );
        }
    }

    // The fullscreen passes are one triangle each, always — nothing about the scene can change
    // them, which makes them the tightest assertion available.
    for fullscreen in [PassId::Post, PassId::Fxaa] {
        let pass = counts.pass(fullscreen);
        assert_eq!(pass.draws, 1, "{} draws a single fullscreen triangle", fullscreen.label());
        assert_eq!(pass.triangles, 1, "{} is one triangle", fullscreen.label());
        assert_eq!(pass.instances, 1, "{} is one instance", fullscreen.label());
    }

    // Three objects, one mesh, one material: ONE draw carrying three instances of four triangles.
    // If batching ever regressed to a draw per object this reads 3 draws and the same 12
    // triangles, so the two numbers together say what changed.
    // The gradient sky rides the same pass and is one triangle at one instance.
    let scene = counts.pass(PassId::Scene);
    assert_eq!(scene.draws, 2, "the batch and the sky, and nothing else");
    assert_eq!(
        scene.triangles,
        TRIANGLES_PER_MESH * OBJECTS + 1,
        "four triangles times three instances, plus the sky"
    );
    assert_eq!(scene.instances, OBJECTS + 1);

    // The shadow pass draws the same batch from the light, with no sky.
    let shadow = counts.pass(PassId::ShadowNear);
    assert_eq!(shadow.draws, 1, "one batch casts");
    assert_eq!(shadow.triangles, TRIANGLES_PER_MESH * OBJECTS);
    assert_eq!(shadow.instances, OBJECTS);

    let total = counts.total();
    assert_eq!(
        total.draws,
        PassId::ALL.iter().map(|id| counts.pass(*id).draws).sum::<u32>(),
        "the total must be the sum of the parts"
    );
}

fn translation(x: f32) -> [[f32; 4]; 4] {
    [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [x, 0.0, 0.0, 1.0]]
}
