use renderer_api::{BindGroupRole, VehicleVertex, baseline_bind_group_layout};
use renderer_wgpu::{
    CameraUniform, GpuContext, TankVertex, basic_tank_shader_source, build_vehicle_pipeline,
    encode_camera_uniform, scene_shader_source, tank_vertex_bytes, validate_wgsl_shader,
    vehicle_shader_source,
};

#[test]
fn camera_uniform_is_encoded_with_wgsl_uniform_layout() {
    let bytes = encode_camera_uniform(&CameraUniform::identity()).expect("camera uniform encodes");

    assert_eq!(bytes.len(), CameraUniform::wgsl_size());
    assert_eq!(bytes.len(), 80);
    assert_eq!(bytes.len() % 16, 0);
}

#[test]
fn basic_tank_shader_is_valid_wgsl() {
    let report =
        validate_wgsl_shader("basic_tank", basic_tank_shader_source()).expect("shader validates");

    assert!(report.entry_points.iter().any(|entry| entry == "vs_main"));
    assert!(report.entry_points.iter().any(|entry| entry == "fs_main"));
}

#[test]
fn scene_shader_is_valid_wgsl_with_tint_inputs() {
    let report =
        validate_wgsl_shader("scene", scene_shader_source()).expect("scene shader validates");

    assert!(report.entry_points.iter().any(|entry| entry == "vs_main"));
    assert!(report.entry_points.iter().any(|entry| entry == "fs_main"));
}

#[test]
fn tank_vertex_is_plain_old_data_for_vertex_buffers() {
    let vertices = [TankVertex::new([1.0, 2.0, 3.0], [0.0, 1.0, 0.0])];
    let bytes = tank_vertex_bytes(&vertices);

    assert_eq!(core::mem::size_of::<TankVertex>(), 24);
    assert_eq!(bytes.len(), 24);
}

#[test]
fn vehicle_shader_is_valid_wgsl_with_pbr_lite_inputs() {
    let report =
        validate_wgsl_shader("vehicle", vehicle_shader_source()).expect("vehicle shader validates");

    assert!(report.entry_points.iter().any(|entry| entry == "vs_main"));
    assert!(report.entry_points.iter().any(|entry| entry == "fs_main"));
    // The vehicle pipeline binds its camera uniform at group 0, binding 0 (like the scene pass).
    assert!(report.has_uniform_binding("camera", 0, 0));
    let source = vehicle_shader_source();
    for binding in [
        "@group(1) @binding(0)\nvar albedo_map",
        "@group(1) @binding(1)\nvar normal_map",
        "@group(1) @binding(2)\nvar ao_roughness_map",
        "@group(1) @binding(3)\nvar cavity_map",
        "@group(1) @binding(4)\nvar vehicle_sampler",
    ] {
        assert!(source.contains(binding), "vehicle shader must bind material resource {binding}");
    }
}

#[test]
fn vehicle_pipeline_builds_on_a_real_device() {
    let Ok(ctx) = GpuContext::headless() else {
        eprintln!("skipping vehicle pipeline test: no GPU adapter");
        return;
    };
    // Proves the shader compiles on the GPU and the VehicleVertex/instance layout binds without
    // validation errors — the pipeline is real, not just WGSL-parsed.
    let (_, _, material_bgl) =
        build_vehicle_pipeline(&ctx.device, wgpu::TextureFormat::Rgba8UnormSrgb, 1);
    let _ = &material_bgl;
    assert_eq!(core::mem::size_of::<VehicleVertex>(), 56);
}

#[test]
fn camera_uniform_uses_camera_view_bind_group_slot() {
    let camera_group = baseline_bind_group_layout()
        .iter()
        .find(|slot| slot.role == BindGroupRole::CameraView)
        .expect("camera bind group exists")
        .index;

    let report =
        validate_wgsl_shader("basic_tank", basic_tank_shader_source()).expect("shader validates");

    assert!(report.has_uniform_binding("camera", camera_group, 0));
}
