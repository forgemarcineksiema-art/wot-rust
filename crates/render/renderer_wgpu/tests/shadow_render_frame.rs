//! Locks that the focused sun shadow reaches the screen (`docs/shadow-policy.md`): an occluder above
//! a ground receiver darkens a patch of that ground, and disabling shadows (`strength = 0`, the
//! capability fallback) is a true no-op. Runs on the headless adapter; skips if none is available.

use game_core::TankId;
use renderer_api::{
    MaterialHandle, MeshHandle, RenderFrame, RenderObject, SceneLighting, SceneVertex,
    VehicleMeshAsset, VehicleVertex, view_projection_matrix,
};
use renderer_wgpu::{GpuContext, OffscreenTarget, SceneRenderer};

fn headless() -> Option<GpuContext> {
    match GpuContext::headless() {
        Ok(ctx) => Some(ctx),
        Err(error) => {
            eprintln!("skipping shadow render frame test: {error}");
            None
        }
    }
}

fn ground() -> (Vec<SceneVertex>, Vec<u32>) {
    let n = [0.0, 1.0, 0.0];
    let c = [0.35, 0.45, 0.28];
    let v = |x: f32, z: f32| SceneVertex::new([x, 0.0, z], n, c);
    // Wound so the up-facing ground presents its front (CCW) face to a camera looking down on it.
    (vec![v(-4.0, -4.0), v(4.0, -4.0), v(4.0, 4.0), v(-4.0, 4.0)], vec![0, 2, 1, 0, 3, 2])
}

/// A small horizontal quad floating 1.2 m above the ground — the shadow caster. It is double-sided
/// (both windings) so a back face survives the shadow pass's front-face cull (which real closed tank
/// hulls satisfy naturally) and this flat plate still casts.
fn occluder() -> VehicleMeshAsset {
    let v = |x: f32, z: f32| VehicleVertex {
        tangent: [1.0, 0.0, 0.0, 1.0],
        ..VehicleVertex::new([x, 1.2, z], [0.0, 1.0, 0.0], [0.0, 0.0], 0, 1.0)
    };
    VehicleMeshAsset::new(
        vec![v(-0.9, -0.9), v(0.9, -0.9), v(0.9, 0.9), v(-0.9, 0.9)],
        vec![0, 1, 2, 0, 2, 3, 0, 2, 1, 0, 3, 2],
    )
}

fn luma(p: &[u8]) -> f32 {
    0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32
}

fn identity() -> [[f32; 4]; 4] {
    [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]]
}

#[test]
fn the_sun_shadow_darkens_the_ground_under_an_occluder() {
    let Some(ctx) = headless() else {
        return;
    };
    let (gv, gi) = ground();
    let mesh = MeshHandle(7);
    // Sun low from the right, camera from the front-above: the caster's shadow stretches to the
    // left, clear of the caster in screen space, so the darkened ground is actually visible.
    let camera = renderer_api::Camera {
        eye: [0.0, 4.0, 6.0],
        target: [0.0, 0.0, 0.0],
        vertical_fov_degrees: 55.0,
    };
    let view_proj = view_projection_matrix(&camera, 1.0, 0.1, 40.0);
    let frame = RenderFrame {
        camera,
        objects: vec![RenderObject {
            tank_id: Some(TankId(1)),
            mesh,
            material: MaterialHandle(0),
            transform: identity(),
            tint: [0.6, 0.7, 0.5],
        }],
    };

    let render = |shadows: bool| {
        let target = OffscreenTarget::new(&ctx, 96, 96).expect("target");
        let mut r = SceneRenderer::for_offscreen(&ctx, &gv, &gi).expect("renderer");
        let mut lighting = SceneLighting::battlefield_default();
        // A low sun from the right throws a long shadow to the left, onto ground the camera sees.
        lighting.key_direction = [1.0, 0.6, 0.0];
        r.scene_lighting = lighting;
        r.shadow_focus = Some([0.0, 0.0, 0.0]);
        r.set_shadows_enabled(shadows);
        r.register_vehicle_mesh(&ctx, mesh, &occluder());
        r.set_vehicle_render_frame(&ctx, &frame);
        r.render(&ctx, target.render_target(), view_proj, camera.eye).expect("render");
        target.read_rgba8(&ctx).expect("readback")
    };

    let lit = render(false);
    let shadowed = render(true);

    // The only difference is the shadow, which strictly darkens the shaded ground and never
    // brightens anything.
    let darkened = lit
        .chunks_exact(4)
        .zip(shadowed.chunks_exact(4))
        .filter(|(a, b)| luma(a) - luma(b) > 18.0)
        .count();
    let brightened = lit
        .chunks_exact(4)
        .zip(shadowed.chunks_exact(4))
        .filter(|(a, b)| luma(b) - luma(a) > 18.0)
        .count();

    assert!(
        darkened > 15,
        "the sun shadow must darken a ground patch under the occluder ({darkened} px)"
    );
    assert!(brightened < 5, "shadows only darken; got {brightened} brightened px");
}
