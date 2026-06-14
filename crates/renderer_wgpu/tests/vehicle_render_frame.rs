use game_core::TankId;
use renderer_api::{
    MaterialHandle, MeshHandle, RenderFrame, RenderObject, VehicleMeshAsset, VehicleVertex,
    view_projection_matrix,
};
use renderer_wgpu::{GpuContext, OffscreenTarget, SceneRenderer};

fn headless_context() -> Option<GpuContext> {
    match GpuContext::headless() {
        Ok(ctx) => Some(ctx),
        Err(error) => {
            eprintln!("skipping vehicle render frame test: {error}");
            None
        }
    }
}

#[test]
fn scene_renderer_draws_registered_vehicle_mesh_with_vehicle_pipeline() {
    let Some(ctx) = headless_context() else {
        return;
    };
    let target = OffscreenTarget::new(&ctx, 64, 64).expect("target");
    let mut renderer = SceneRenderer::for_offscreen(&ctx, &[], &[]).expect("renderer");
    let mesh = MeshHandle(91);

    renderer.register_vehicle_mesh(&ctx, mesh, &vehicle_triangle());

    let camera = renderer_api::Camera {
        eye: [0.0, 0.0, 3.0],
        target: [0.0, 0.0, 0.0],
        vertical_fov_degrees: 55.0,
    };
    renderer.set_vehicle_render_frame(
        &ctx,
        &RenderFrame {
            camera,
            objects: vec![RenderObject {
                tank_id: Some(TankId(1)),
                mesh,
                material: MaterialHandle(0),
                transform: identity(),
                tint: [0.65, 0.85, 0.52],
            }],
        },
    );
    renderer
        .render(&ctx, target.render_target(), view_projection_matrix(&camera, 1.0, 0.1, 20.0))
        .expect("render frame");

    let pixels = target.read_rgba8(&ctx).expect("readback");
    assert!(
        pixels.chunks_exact(4).any(|p| p[1] > 80 && p[0] > 30 && p[2] > 20),
        "vehicle pipeline should render a lit tinted triangle"
    );
}

fn vehicle_triangle() -> VehicleMeshAsset {
    VehicleMeshAsset::new(
        vec![
            VehicleVertex {
                tangent: [1.0, 0.0, 0.0, 1.0],
                ..VehicleVertex::new([-0.6, -0.4, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0], 0, 1.0)
            },
            VehicleVertex {
                tangent: [1.0, 0.0, 0.0, 1.0],
                ..VehicleVertex::new([0.6, -0.4, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0], 0, 1.0)
            },
            VehicleVertex {
                tangent: [1.0, 0.0, 0.0, 1.0],
                ..VehicleVertex::new([0.0, 0.6, 0.0], [0.0, 0.0, 1.0], [0.5, 1.0], 0, 1.0)
            },
        ],
        vec![0, 1, 2],
    )
}

fn identity() -> [[f32; 4]; 4] {
    [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]]
}
