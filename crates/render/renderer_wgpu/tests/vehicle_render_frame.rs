use game_core::TankId;
use renderer_api::{
    ArmorApertureRender, ArmorDamageInstance, MaterialHandle, MeshHandle, RenderFrame,
    RenderObject, VehicleMaterialFamilies, VehicleMaterialMaps, VehicleMeshAsset,
    VehicleTextureMap, VehicleVertex, view_projection_matrix,
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
fn analytical_aperture_opens_the_vehicle_in_color_and_depth() {
    let Some(ctx) = headless_context() else {
        return;
    };
    let mesh = MeshHandle(92);
    let camera = renderer_api::Camera {
        eye: [0.0, 0.0, 3.0],
        target: [0.0, 0.0, 0.0],
        vertical_fov_degrees: 55.0,
    };
    let render = |armor_damage| {
        let target = OffscreenTarget::new(&ctx, 64, 64).expect("target");
        let mut renderer = SceneRenderer::for_offscreen(&ctx, &[], &[]).expect("renderer");
        renderer.register_vehicle_mesh(&ctx, mesh, &vehicle_triangle());
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
                armor_damage,
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
        target.read_rgba8(&ctx).expect("readback")
    };
    let whole = render(Vec::new());
    let cut = render(vec![ArmorDamageInstance {
        tank_id: TankId(1),
        apertures: vec![ArmorApertureRender {
            center: [0.0, 0.0, 0.0],
            normal: [0.0, 0.0, 1.0],
            tangent: [1.0, 0.0, 0.0],
            major_radius_m: 0.18,
            minor_radius_m: 0.14,
            rotation_rad: 0.2,
            irregularity: 0.1,
            phase_a: 0.7,
            phase_b: 2.1,
            half_depth_m: 0.2,
            glow: 0.0,
            glow_tightness: 1.0,
        }],
    }]);
    let center = (32 * 64 + 32) * 4;
    assert!(
        luma(&whole[center..center + 4]) - luma(&cut[center..center + 4]) > 10.0
            || whole[center..center + 4] != cut[center..center + 4],
        "the aperture must reveal the background at the triangle center"
    );
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
        .expect("render frame");

    let pixels = target.read_rgba8(&ctx).expect("readback");
    assert!(
        pixels.chunks_exact(4).any(|p| p[1] > 80 && p[0] > 30 && p[2] > 20),
        "vehicle pipeline should render a lit tinted triangle"
    );
}

#[test]
fn registered_vehicle_material_overrides_the_neutral_fallback() {
    let Some(ctx) = headless_context() else {
        return;
    };
    let mesh = MeshHandle(91);
    let material = MaterialHandle(0);
    let camera = renderer_api::Camera {
        eye: [0.0, 0.0, 3.0],
        target: [0.0, 0.0, 0.0],
        vertical_fov_degrees: 55.0,
    };
    let frame = RenderFrame {
        camera,
        objects: vec![RenderObject {
            tank_id: Some(TankId(1)),
            mesh,
            material,
            transform: identity(),
            tint: [1.0, 1.0, 1.0],
        }],
        armor_damage: Vec::new(),
    };
    let view_proj = view_projection_matrix(&camera, 1.0, 0.1, 20.0);

    // Fallback path: no material registered, so the neutral white albedo is used.
    let fallback = {
        let target = OffscreenTarget::new(&ctx, 64, 64).expect("target");
        let mut renderer = SceneRenderer::for_offscreen(&ctx, &[], &[]).expect("renderer");
        renderer.register_vehicle_mesh(&ctx, mesh, &vehicle_triangle());
        renderer.set_vehicle_render_frame(&ctx, &frame);
        renderer
            .render(&ctx, target.render_target(), view_proj, camera.eye)
            .expect("render fallback");
        target.read_rgba8(&ctx).expect("readback")
    };

    // Uploaded path: a near-black albedo map must visibly darken the same mesh.
    let uploaded = {
        let target = OffscreenTarget::new(&ctx, 64, 64).expect("target");
        let mut renderer = SceneRenderer::for_offscreen(&ctx, &[], &[]).expect("renderer");
        renderer.register_vehicle_mesh(&ctx, mesh, &vehicle_triangle());
        renderer.register_vehicle_material(&ctx, material, &dark_albedo_families());
        renderer.set_vehicle_render_frame(&ctx, &frame);
        renderer
            .render(&ctx, target.render_target(), view_proj, camera.eye)
            .expect("render uploaded");
        target.read_rgba8(&ctx).expect("readback")
    };

    // The shared sky background is identical, so only the triangle pixels differ — and the dark
    // albedo must make them strictly darker, never brighter.
    let darkened = fallback
        .chunks_exact(4)
        .zip(uploaded.chunks_exact(4))
        .filter(|(a, b)| luma(a) - luma(b) > 30.0)
        .count();
    let brightened = fallback
        .chunks_exact(4)
        .zip(uploaded.chunks_exact(4))
        .filter(|(a, b)| luma(b) - luma(a) > 30.0)
        .count();
    assert!(darkened > 150, "uploaded dark albedo must darken the triangle ({darkened} px)");
    assert_eq!(brightened, 0, "uploaded dark albedo must never brighten ({brightened} px)");
}

#[test]
fn both_surface_mapping_branches_render_lit_geometry() {
    // The triplanar branch is exercised by the default-mapping triangle above; this pins that the
    // parametric branch (authored UV + tangent normal map) also renders non-empty lit geometry, so
    // both shader paths are validated headlessly with no NaN/validation errors.
    let Some(ctx) = headless_context() else {
        return;
    };
    let camera = renderer_api::Camera {
        eye: [0.0, 0.0, 3.0],
        target: [0.0, 0.0, 0.0],
        vertical_fov_degrees: 55.0,
    };
    for mapping in [renderer_api::MAPPING_PARAMETRIC, renderer_api::MAPPING_TRIPLANAR] {
        let target = OffscreenTarget::new(&ctx, 64, 64).expect("target");
        let mut renderer = SceneRenderer::for_offscreen(&ctx, &[], &[]).expect("renderer");
        let mesh = MeshHandle(91);
        renderer.register_vehicle_mesh(&ctx, mesh, &mapped_triangle(mapping));
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
            .expect("render frame");
        let pixels = target.read_rgba8(&ctx).expect("readback");
        assert!(
            pixels.chunks_exact(4).any(|p| p[1] > 80 && p[0] > 30 && p[2] > 20),
            "mapping {mapping} must render a lit tinted triangle",
        );
    }
}

fn mapped_triangle(mapping: u32) -> VehicleMeshAsset {
    let v = |p: [f32; 3], uv: [f32; 2]| VehicleVertex {
        tangent: [1.0, 0.0, 0.0, 1.0],
        ..VehicleVertex::new(p, [0.0, 0.0, 1.0], uv, 0, 1.0).with_mapping_mode(mapping)
    };
    VehicleMeshAsset::new(
        vec![
            v([-0.6, -0.4, 0.0], [0.0, 0.0]),
            v([0.6, -0.4, 0.0], [1.0, 0.0]),
            v([0.0, 0.6, 0.0], [0.5, 1.0]),
        ],
        vec![0, 1, 2],
    )
}

fn luma(p: &[u8]) -> f32 {
    0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32
}

/// Five identical dark-albedo role layers, so whichever `material_id` the mesh uses samples a near
/// black albedo and the triangle visibly darkens.
fn dark_albedo_families() -> VehicleMaterialFamilies {
    let layer = || {
        VehicleMaterialMaps::new(
            VehicleTextureMap::new(1, 1, vec![8, 8, 8, 255]),
            VehicleTextureMap::new(1, 1, vec![128, 128, 255, 255]),
            VehicleTextureMap::new(1, 1, vec![255, 160, 0, 255]),
            Some(VehicleTextureMap::new(1, 1, vec![255, 255, 255, 255])),
        )
    };
    VehicleMaterialFamilies::new((0..VehicleMaterialFamilies::LAYERS).map(|_| layer()).collect())
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
