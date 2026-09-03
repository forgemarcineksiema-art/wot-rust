mod common;
use common::{identity, luma_255};

use game_core::TankId;
use renderer_api::{
    ArmorApertureRender, ArmorDamageInstance, ArmorMarkKind, MaterialHandle, MeshHandle,
    RenderFrame, RenderObject, VehicleMaterialFamilies, VehicleMaterialMaps, VehicleMeshAsset,
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
                    dither: 0.0,
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
            cut: true,
            kind: ArmorMarkKind::Breach,
        }],
    }]);
    let center = (32 * 64 + 32) * 4;
    assert!(
        luma_255(&whole[center..center + 4]) - luma_255(&cut[center..center + 4]) > 10.0
            || whole[center..center + 4] != cut[center..center + 4],
        "the aperture must reveal the background at the triangle center"
    );
}

/// A legacy hull without cut truth must never open — its perforation is a scorch mark: the
/// fragment survives (no discard in color or depth) but reads visibly darker than pristine
/// steel. This is the fleet-wide interim until the hybrid migration grants real holes.
#[test]
fn a_scorch_only_aperture_darkens_without_opening_the_mesh() {
    let Some(ctx) = headless_context() else {
        return;
    };
    let mesh = MeshHandle(93);
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
                    dither: 0.0,
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
    let aperture = |cut| ArmorApertureRender {
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
        cut,
        kind: ArmorMarkKind::Breach,
    };
    let whole = render(Vec::new());
    let cut =
        render(vec![ArmorDamageInstance { tank_id: TankId(1), apertures: vec![aperture(true)] }]);
    let scorched =
        render(vec![ArmorDamageInstance { tank_id: TankId(1), apertures: vec![aperture(false)] }]);
    let center = (32 * 64 + 32) * 4;
    assert_ne!(
        scorched[center..center + 4],
        cut[center..center + 4],
        "a scorch-only aperture must not open like a cut one"
    );
    assert!(
        luma_255(&whole[center..center + 4]) - luma_255(&scorched[center..center + 4]) > 8.0,
        "the scorch must read visibly darker than pristine steel: {} vs {}",
        luma_255(&whole[center..center + 4]),
        luma_255(&scorched[center..center + 4])
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
                dither: 0.0,
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
            dither: 0.0,
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
        .filter(|(a, b)| luma_255(a) - luma_255(b) > 30.0)
        .count();
    let brightened = fallback
        .chunks_exact(4)
        .zip(uploaded.chunks_exact(4))
        .filter(|(a, b)| luma_255(b) - luma_255(a) > 30.0)
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
                    dither: 0.0,
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

/// Inny Poziom Z5: a non-penetrating strike is MATERIAL on the plate, not a stamp over it. A
/// scuff record (kind 1) changes the plate's colour at its centre — bared steel and a scorched
/// rim — while the mesh stays closed (the pixel is still the plate, not the background), and a
/// gouge record (kind 2) does the same along its groove. The renderer decides this from the
/// same aperture list the breaches ride, so a wound can never float 6 mm in front of its hull.
#[test]
fn a_scuff_and_a_gouge_mark_the_plate_without_opening_it() {
    let Some(ctx) = headless_context() else {
        return;
    };
    let mesh = MeshHandle(94);
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
                    dither: 0.0,
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
    let wound = |kind: ArmorMarkKind, major: f32, minor: f32| {
        vec![ArmorDamageInstance {
            tank_id: TankId(1),
            apertures: vec![ArmorApertureRender {
                center: [0.0, 0.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                tangent: [1.0, 0.0, 0.0],
                major_radius_m: major,
                minor_radius_m: minor,
                rotation_rad: 0.0,
                irregularity: 0.15,
                phase_a: 0.7,
                phase_b: 2.1,
                half_depth_m: 0.2,
                glow: 0.0,
                glow_tightness: 1.0,
                cut: false,
                kind,
            }],
        }]
    };
    let whole = render(Vec::new());
    let scuffed = render(wound(ArmorMarkKind::Scuff, 0.25, 0.25));
    let gouged = render(wound(ArmorMarkKind::Gouge, 0.45, 0.10));
    let center = (32 * 64 + 32) * 4;
    let corner = 4;
    let plate = &whole[center..center + 4];
    for (label, marked) in [("scuff", &scuffed), ("gouge", &gouged)] {
        let pixel = &marked[center..center + 4];
        // Bared steel is grey where the paint was green: the hue moves even where the luma
        // barely does, so measure the colour distance, not the brightness.
        let distance: i32 = (0..3).map(|c| (pixel[c] as i32 - plate[c] as i32).abs()).sum();
        assert!(
            distance > 40,
            "the {label} changes the plate's colour at its centre visibly: {pixel:?} vs {plate:?}"
        );
        assert_ne!(
            pixel,
            &marked[corner..corner + 4],
            "the {label} does not open the plate: its centre is not the background"
        );
    }
}

/// Z6: the hull's inside is a cavity. An interior material (primer, id 5) under the same sun as
/// the painted plate (id 0) renders far darker — no key, fill or rim light reaches it, only a
/// whisper of ambient and whatever comes through its own apertures (none here).
#[test]
fn an_interior_material_is_a_cavity_lit_only_through_the_apertures() {
    let Some(ctx) = headless_context() else {
        return;
    };
    let camera = renderer_api::Camera {
        eye: [0.0, 0.0, 3.0],
        target: [0.0, 0.0, 0.0],
        vertical_fov_degrees: 55.0,
    };
    let render = |mesh: MeshHandle, material_id: u32| {
        let target = OffscreenTarget::new(&ctx, 64, 64).expect("target");
        let mut renderer = SceneRenderer::for_offscreen(&ctx, &[], &[]).expect("renderer");
        renderer.register_vehicle_mesh(&ctx, mesh, &vehicle_triangle_of(material_id));
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
                    dither: 0.0,
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
            .expect("render");
        target.read_rgba8(&ctx).expect("readback")
    };
    let center = (32 * 64 + 32) * 4;
    let plate = render(MeshHandle(95), 0);
    let cavity = render(MeshHandle(96), 5);
    let plate_luma = luma_255(&plate[center..center + 4]);
    let cavity_luma = luma_255(&cavity[center..center + 4]);
    assert!(
        cavity_luma < plate_luma * 0.45,
        "the interior is a cavity: {cavity_luma} vs the plate's {plate_luma}"
    );
}

/// The test triangle with a chosen material id.
fn vehicle_triangle_of(material_id: u32) -> VehicleMeshAsset {
    VehicleMeshAsset::new(
        vec![
            VehicleVertex {
                tangent: [1.0, 0.0, 0.0, 1.0],
                ..VehicleVertex::new(
                    [-0.6, -0.4, 0.0],
                    [0.0, 0.0, 1.0],
                    [0.0, 0.0],
                    material_id,
                    1.0,
                )
            },
            VehicleVertex {
                tangent: [1.0, 0.0, 0.0, 1.0],
                ..VehicleVertex::new(
                    [0.6, -0.4, 0.0],
                    [0.0, 0.0, 1.0],
                    [1.0, 0.0],
                    material_id,
                    1.0,
                )
            },
            VehicleVertex {
                tangent: [1.0, 0.0, 0.0, 1.0],
                ..VehicleVertex::new([0.0, 0.6, 0.0], [0.0, 0.0, 1.0], [0.5, 1.0], material_id, 1.0)
            },
        ],
        vec![0, 1, 2],
    )
}

/// Z6: through a breach the eye meets the INSIDE of the far wall — dark, a cavity — not the
/// background behind the hull. A closed painted box with a breach cut through its front face:
/// the centre pixel is neither the front face (that is cut) nor the clear colour (the interior
/// shell draws the far wall's back face), and it is far darker than the intact front face.
#[test]
fn a_breach_reveals_the_dark_inside_of_the_far_wall_not_the_background() {
    let Some(ctx) = headless_context() else {
        return;
    };
    let camera = renderer_api::Camera {
        eye: [0.0, 0.0, 3.0],
        target: [0.0, 0.0, 0.0],
        vertical_fov_degrees: 55.0,
    };
    let render = |mesh: MeshHandle, armor_damage| {
        let target = OffscreenTarget::new(&ctx, 64, 64).expect("target");
        let mut renderer = SceneRenderer::for_offscreen(&ctx, &[], &[]).expect("renderer");
        renderer.register_vehicle_mesh(&ctx, mesh, &vehicle_box());
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
                    dither: 0.0,
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
    let breach = vec![ArmorDamageInstance {
        tank_id: TankId(1),
        apertures: vec![ArmorApertureRender {
            center: [0.0, 0.0, 0.5],
            normal: [0.0, 0.0, 1.0],
            tangent: [1.0, 0.0, 0.0],
            major_radius_m: 0.16,
            minor_radius_m: 0.14,
            rotation_rad: 0.0,
            irregularity: 0.1,
            phase_a: 0.7,
            phase_b: 2.1,
            half_depth_m: 0.15,
            glow: 0.0,
            glow_tightness: 1.0,
            cut: true,
            kind: ArmorMarkKind::Breach,
        }],
    }];
    let intact = render(MeshHandle(99), Vec::new());
    let holed = render(MeshHandle(100), breach);
    let center = (32 * 64 + 32) * 4;
    let corner = 4;
    let front = luma_255(&intact[center..center + 4]);
    let inside = luma_255(&holed[center..center + 4]);
    assert_ne!(
        &holed[center..center + 4],
        &holed[corner..corner + 4],
        "the breach does not show the background: the interior shell draws the far wall"
    );
    assert!(
        inside < front * 0.45,
        "the far wall's inside is a cavity: {inside} vs the front face's {front}"
    );
}

/// A closed painted box (1 m wide, 1 m tall, 1 m deep), outward normals, CCW faces.
fn vehicle_box() -> VehicleMeshAsset {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let faces: [([f32; 3], [f32; 3], [f32; 3]); 6] = [
        ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        ([0.0, 0.0, -1.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        ([1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]),
        ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
        ([0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0]),
        ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
    ];
    for (normal, u, v) in faces {
        let base = vertices.len() as u32;
        for (su, sv) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            let position = [
                normal[0] * 0.5 + u[0] * 0.5 * su + v[0] * 0.5 * sv,
                normal[1] * 0.5 + u[1] * 0.5 * su + v[1] * 0.5 * sv,
                normal[2] * 0.5 + u[2] * 0.5 * su + v[2] * 0.5 * sv,
            ];
            vertices.push(VehicleVertex {
                tangent: [u[0], u[1], u[2], 1.0],
                ..VehicleVertex::new(position, normal, [0.0, 0.0], 0, 1.0)
            });
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    VehicleMeshAsset::new(vertices, indices)
}
