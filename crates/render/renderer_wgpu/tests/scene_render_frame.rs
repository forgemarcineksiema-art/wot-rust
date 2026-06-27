use game_core::TankId;
use renderer_api::{
    MaterialHandle, MeshAsset, MeshHandle, RenderFrame, RenderObject, SceneVertex,
    view_projection_matrix,
};
use renderer_wgpu::{GpuContext, OffscreenTarget, SceneRenderer};

fn headless_context() -> Option<GpuContext> {
    match GpuContext::headless() {
        Ok(ctx) => Some(ctx),
        Err(error) => {
            eprintln!("skipping render frame test: {error}");
            None
        }
    }
}

#[test]
fn scene_renderer_draws_registered_mesh_from_render_frame_transform() {
    let Some(ctx) = headless_context() else {
        return;
    };
    let target = OffscreenTarget::new(&ctx, 64, 64).expect("target");
    let mut renderer = SceneRenderer::for_offscreen(&ctx, &[], &[]).expect("renderer");
    let mesh = MeshHandle(42);

    renderer.register_mesh(
        &ctx,
        mesh,
        &MeshAsset::new(
            vec![
                SceneVertex::new([-0.6, -0.4, 0.0], [0.0, 0.0, 1.0], [0.9, 0.2, 0.1]),
                SceneVertex::new([0.6, -0.4, 0.0], [0.0, 0.0, 1.0], [0.9, 0.2, 0.1]),
                SceneVertex::new([0.0, 0.6, 0.0], [0.0, 0.0, 1.0], [0.9, 0.2, 0.1]),
            ],
            vec![0, 1, 2],
        ),
    );

    let camera = renderer_api::Camera {
        eye: [0.0, 0.0, 3.0],
        target: [0.0, 0.0, 0.0],
        vertical_fov_degrees: 55.0,
    };
    renderer.set_render_frame(
        &ctx,
        &RenderFrame {
            camera,
            objects: vec![RenderObject {
                tank_id: Some(TankId(1)),
                mesh,
                material: MaterialHandle(0),
                transform: identity(),
                tint: [1.0, 1.0, 1.0],
            }],
        },
    );
    renderer
        .render(
            &ctx,
            target.render_target(),
            view_projection_matrix(&camera, 1.0, 0.1, 20.0),
            camera.eye,
        )
        .expect("render frame");

    let pixels = target.read_rgba8(&ctx).expect("readback");
    assert!(pixels.chunks_exact(4).any(|p| p[0] > 120 && p[1] < 120 && p[2] < 120));
}

fn identity() -> [[f32; 4]; 4] {
    [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]]
}
