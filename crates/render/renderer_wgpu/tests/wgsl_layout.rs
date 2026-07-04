use renderer_api::{BindGroupRole, FxVertex, VehicleVertex, baseline_bind_group_layout};
use renderer_wgpu::{
    CameraUniform, GpuContext, TankVertex, basic_tank_shader_source,
    build_shadow_bind_group_layout, build_vehicle_pipeline, encode_camera_uniform,
    fx_shader_source, scene_shader_source, shadow_shader_source, tank_vertex_bytes,
    validate_wgsl_shader, vehicle_shader_source,
};

#[test]
fn camera_uniform_is_encoded_with_wgsl_uniform_layout() {
    let bytes = encode_camera_uniform(&CameraUniform::identity()).expect("camera uniform encodes");

    assert_eq!(bytes.len(), CameraUniform::wgsl_size());
    // view_proj (64) + camera_pos + sky ambient + ground ambient + key/fill/rim direction+colour
    // (9 vec3 in 16-byte slots, 144) + light_view_proj (mat4, 64) + shadow_params (vec4, 16):
    // 64 + 144 + 64 + 16 = 304.
    assert_eq!(bytes.len(), 304);
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
fn fx_shader_is_valid_wgsl_and_shares_the_scene_camera_slot() {
    let report = validate_wgsl_shader("fx", fx_shader_source()).expect("fx shader validates");

    assert!(report.entry_points.iter().any(|entry| entry == "vs_main"));
    assert!(report.entry_points.iter().any(|entry| entry == "fs_main"));
    // The FX pipeline reuses the scene camera bind group, so its uniform must sit at the same
    // slot the scene pipeline binds (group 0, binding 0).
    assert!(report.has_uniform_binding("camera", 0, 0));
}

#[test]
fn fx_vertex_is_plain_old_data_matching_the_fx_attribute_layout() {
    // position (12) + uv (8) + sharpness (4) + premultiplied color (16) = 40 bytes, zero
    // padding — the WGSL vertex_attr_array in fx_pipeline.rs assumes this exact packing.
    assert_eq!(core::mem::size_of::<FxVertex>(), 40);
    let vertex = FxVertex::new([1.0, 2.0, 3.0], [-1.0, 1.0], [0.5, 0.4, 0.3, 0.2]);
    assert_eq!(vertex.sharpness, 1.0, "plain particles stay soft");
    let stamped = FxVertex::sharp([0.0; 3], [0.0, 0.0], 6.0, [0.1; 4]);
    assert_eq!(stamped.sharpness, 6.0);
    let bytes: &[u8] = bytemuck::bytes_of(&stamped);
    assert_eq!(bytes.len(), 40);
}

#[test]
fn shadow_shader_is_valid_wgsl() {
    let report =
        validate_wgsl_shader("shadow", shadow_shader_source()).expect("shadow shader validates");
    assert!(report.entry_points.iter().any(|entry| entry == "vs_main"));
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
    for (binding, name) in [
        ("@group(1) @binding(0)", "var albedo_map"),
        ("@group(1) @binding(1)", "var normal_map"),
        ("@group(1) @binding(2)", "var ao_roughness_map"),
        ("@group(1) @binding(3)", "var cavity_map"),
        ("@group(1) @binding(4)", "var vehicle_sampler"),
    ] {
        assert_binding_declared(source, binding, name);
    }
}

fn assert_binding_declared(source: &str, binding: &str, name: &str) {
    let binding_at = source.find(binding).unwrap_or_else(|| panic!("missing {binding}"));
    let tail = &source[binding_at..];
    assert!(
        tail.find(name).is_some_and(|offset| offset < 80),
        "vehicle shader must bind material resource {binding} {name}"
    );
}

#[test]
fn vehicle_pipeline_builds_on_a_real_device() {
    let Ok(ctx) = GpuContext::headless() else {
        eprintln!("skipping vehicle pipeline test: no GPU adapter");
        return;
    };
    // Proves the shader compiles on the GPU and the VehicleVertex/instance layout binds without
    // validation errors — the pipeline is real, not just WGSL-parsed.
    let shadow_bgl = build_shadow_bind_group_layout(&ctx.device);
    let (_, _, material_bgl) =
        build_vehicle_pipeline(&ctx.device, wgpu::TextureFormat::Rgba8UnormSrgb, 1, &shadow_bgl);
    let _ = &material_bgl;
    assert_eq!(core::mem::size_of::<VehicleVertex>(), 64);
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
