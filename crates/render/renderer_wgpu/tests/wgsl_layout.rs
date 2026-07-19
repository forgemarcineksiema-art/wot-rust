use renderer_api::{BindGroupRole, FxVertex, VehicleVertex, baseline_bind_group_layout};
use renderer_wgpu::{
    CameraUniform, GpuContext, TankVertex, basic_tank_shader_source,
    build_camera_bind_group_layout, build_shadow_bind_group_layout, build_vehicle_pipeline,
    encode_camera_uniform, fx_shader_source, scene_shader_source, shadow_shader_source,
    sky_shader_source, tank_vertex_bytes, terrain_shader_source, validate_wgsl_shader,
    vehicle_shader_source,
};

#[test]
fn camera_uniform_is_encoded_with_wgsl_uniform_layout() {
    let bytes = encode_camera_uniform(&CameraUniform::identity()).expect("camera uniform encodes");

    assert_eq!(bytes.len(), CameraUniform::wgsl_size());
    // view_proj (64) + camera_pos + sky ambient + ground ambient + key/fill/rim direction+colour
    // (9 vec3 in 16-byte slots, 144) + light_view_proj (mat4, 64) + shadow_params (vec4, 16)
    // + ssao_params (vec4, 16): 64 + 144 + 64 + 16 + 16 = 304. Phase-2 atmosphere adds the gradient
    // sky zenith + horizon (2 vec3, 32) and fog_params (vec4, 16): 304 + 48 = 352, plus the
    // inv_view_proj mat4 (64) the sky pass unprojects with: 352 + 64 = 416, plus time_params
    // (vec4, 16) — the tick-domain presentation clock shader animation runs on: 416 + 16 = 432.
    // The shadow cascades add light_view_proj_far (mat4, 64) and cascade_params (vec4, 16):
    // 432 + 80 = 512. The profile display grade adds grade_params (vec4, 16): 512 + 16 = 528.
    // The profile sky adds cloud_params + sky_params (2 vec4, 32): 528 + 32 = 560. The local
    // fill pools add light_pos_radius + light_rgb_intensity (2 x array<vec4, 6>, 192):
    // 560 + 192 = 752. The two-layer air + second cloud layer append haze_params +
    // cloud2_params (2 vec4, 32): 752 + 32 = 784. Dynamic weather appends one vec4: 800.
    assert_eq!(bytes.len(), 800);
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
        validate_wgsl_shader("scene", &scene_shader_source()).expect("scene shader validates");

    assert!(report.entry_points.iter().any(|entry| entry == "vs_main"));
    assert!(report.entry_points.iter().any(|entry| entry == "fs_main"));
}

#[test]
fn terrain_shader_is_valid_wgsl() {
    let report = validate_wgsl_shader("terrain", &terrain_shader_source())
        .expect("terrain shader validates");

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
fn sky_shader_is_valid_wgsl_and_shares_the_scene_camera_slot() {
    let report = validate_wgsl_shader("sky", &sky_shader_source()).expect("sky shader validates");

    assert!(report.entry_points.iter().any(|entry| entry == "vs_main"));
    assert!(report.entry_points.iter().any(|entry| entry == "fs_main"));
    // The gradient-sky pass reuses the scene camera bind group (group 0, binding 0) to unproject
    // the per-pixel view ray, so its uniform must sit at the same slot the scene pipeline binds.
    assert!(report.has_uniform_binding("camera", 0, 0));
}

#[test]
fn water_shader_is_valid_wgsl_and_shares_the_scene_camera_slot() {
    let report = validate_wgsl_shader("water", &renderer_wgpu::water_shader_source())
        .expect("water shader validates");
    assert!(report.entry_points.iter().any(|entry| entry == "vs_main"));
    assert!(report.entry_points.iter().any(|entry| entry == "fs_main"));
    // The refraction variant (A6b) is a second fragment entry sampling the opaque grab; it must
    // coexist with the analytic entry so the discrete tier can pick it without a second shader.
    assert!(
        report.entry_points.iter().any(|entry| entry == "fs_refract"),
        "the refraction entry point must be present"
    );
    // The water pass reuses the scene camera bind group - the ripple runs on its time uniform.
    assert!(report.has_uniform_binding("camera", 0, 0));
    // The refraction params ride group 1 (the grab texture/sampler share the group); the analytic
    // entry never touches group 1, so its pipeline layout stays camera-only.
    assert!(
        report.has_uniform_binding("refraction", 1, 2),
        "refraction params must sit at group 1 binding 2"
    );
}

#[test]
fn rain_shader_is_valid_wgsl_and_shares_the_scene_camera_slot() {
    let report = validate_wgsl_shader("rain", &renderer_wgpu::rain_shader_source())
        .expect("rain shader validates");
    assert!(report.entry_points.iter().any(|entry| entry == "vs_main"));
    assert!(report.entry_points.iter().any(|entry| entry == "fs_main"));
    // Stateless: the streaks are a pure function of (instance, time, camera) in this uniform.
    assert!(report.has_uniform_binding("camera", 0, 0));
}

#[test]
fn shadow_shader_is_valid_wgsl() {
    let report =
        validate_wgsl_shader("shadow", &shadow_shader_source()).expect("shadow shader validates");
    assert!(report.entry_points.iter().any(|entry| entry == "vs_main"));
}

#[test]
fn reduced_near_shadow_pcf_normalizes_its_four_samples() {
    let source = scene_shader_source();

    assert!(source.contains("var tap_count = 9.0;"), "wide 3x3 PCF must normalize nine taps");
    assert!(source.contains("tap_count = 4.0;"), "reduced 2x2 PCF must normalize four taps");
    assert!(
        source.contains("sum / tap_count"),
        "near shadow visibility must use the active PCF tap count"
    );
    assert!(
        !source.contains("sum / 9.0"),
        "the reduced path must never divide four samples by nine"
    );
}

#[test]
fn dynamic_weather_uses_one_seeded_phase_and_separate_puddle_fill() {
    let scene = scene_shader_source();
    let terrain = terrain_shader_source();
    let sky = sky_shader_source();
    let rain = renderer_wgpu::rain_shader_source();

    for source in [&scene, &terrain, &sky] {
        assert!(source.contains("camera.weather_params.xy"), "cloud phase must be shared");
    }
    for source in [&scene, &terrain] {
        assert!(source.contains("camera.weather_params.z"), "puddles need standing-water fill");
        assert!(source.contains("mix(0.80, 0.54"), "fill must expand continuous basins");
    }
    assert!(rain.contains("camera.weather_params.w"), "rain needs its seeded time phase");
}

#[test]
fn sky_sun_disc_is_cloud_occluded_and_reads_profile_softness() {
    let sky = sky_shader_source();

    // The disc must be modulated by the cloud cover at its pixel — purely additive sun energy
    // burns through a solid overcast lid the moment a profile carries a hot key.
    assert!(
        sky.contains("let sun_cover = cloud * camera.cloud_params.z;"),
        "the sun pass must sample the cloud cover it sits behind"
    );
    assert!(sky.contains("disc * (1.0 - sun_cover)"), "the disc must die behind a full lid");
    // Softness is profile data (haze_params.w), never re-derived from the fog density — that
    // coupling made the fairness fog knob silently retune the sun's look.
    assert!(sky.contains("camera.haze_params.w"), "sun softness rides haze_params.w");
    assert!(
        !sky.contains("fog_params.x * 700.0"),
        "the disc's softness must not be derived from the fog density"
    );
}

#[test]
fn sky_cloud_field_is_lattice_decorrelated() {
    let sky = sky_shader_source();

    // The square-sky artefact had three roots, each locked here. The sin hash collapsed to
    // flat hard-edged plates once the octave chain pushed its argument past the GPU sin's
    // accurate range; the sky hash must stay sin-free.
    assert!(
        !sky.contains("fract(sin(dot"),
        "the sky lattice hash must not run its coordinates through sin"
    );
    // Axis-aligned octaves all reinforced ONE square lattice, and the dome projection blew a
    // single base cell up to tens of degrees of sky: the octave chain must rotate.
    assert!(
        sky.contains("mat2x2<f32>(1.6, 1.2, -1.2, 1.6)"),
        "fbm octaves must rotate off the shared lattice"
    );
    // The high sheet thresholded RAW fbm — the cleanest cell display in the dome — and the
    // ridged crease painted closed-curve rings into the open blue between banks.
    assert!(
        sky.contains("cloud_fbm(sheet_uv + sheet_warp"),
        "the high sheet must be warped and rotated, not raw fbm"
    );
    assert!(
        sky.contains("* smoothstep(0.42, 0.62, body)"),
        "the ridged term must be gated by the bank body"
    );
}

#[test]
fn ground_cloud_shade_scale_matches_the_dome() {
    // The terrain's wandering cloud shade promises coherence "in motion and scale" with the
    // dome (scene.wgsl); the dome's base pattern scale and the ground field's world-metres
    // mapping must carry the SAME factor or the shade stops matching the banks overhead.
    let sky = sky_shader_source();
    assert!(sky.contains("* 1.35 * camera.cloud_params.y"), "dome base scale factor");
    for (label, source) in [("scene", scene_shader_source()), ("terrain", terrain_shader_source())]
    {
        assert!(
            source.contains("(1.35 / 400.0) * camera.cloud_params.y"),
            "{label}: ground cloud shade must keep the dome's pattern scale"
        );
    }
}

#[test]
fn sky_cloud_projections_guard_their_horizon_singularity() {
    let sky = sky_shader_source();

    // dir.xz / (dir.y + c) blows up to inf at dir.y = -c, and fbm(inf) can return NaN — which
    // the band's zero does NOT stop (NaN * 0.0 = NaN). Both cloud layers clamp the denominator.
    assert!(sky.contains("max(dir.y + 0.45,"), "the cumulus projection must clamp its divisor");
    assert!(sky.contains("max(dir.y + 0.55,"), "the sheet projection must clamp its divisor");
    assert!(
        !sky.contains("/ (dir.y + 0.45)") && !sky.contains("/ (dir.y + 0.55)"),
        "no cloud projection may divide by an unclamped horizon-crossing term"
    );
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
    let report = validate_wgsl_shader("vehicle", &vehicle_shader_source())
        .expect("vehicle shader validates");

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
        assert_binding_declared(&source, binding, name);
    }
}

#[test]
fn shared_wgsl_fragments_are_composed_exactly_once_per_shader() {
    use renderer_wgpu::{rain_shader_source, ssao_shader_source, water_shader_source};

    // Every camera-bound pass gets its Camera struct from camera_common.wgsl — one declaration
    // per composed source. A count of 0 means the composition dropped the fragment; 2+ means a
    // duplicated copy crept back into a pass body.
    for (label, source) in [
        ("scene", scene_shader_source()),
        ("vehicle", vehicle_shader_source()),
        ("sky", sky_shader_source()),
        ("water", water_shader_source()),
        ("rain", rain_shader_source()),
        ("shadow", shadow_shader_source()),
        ("ssao", ssao_shader_source()),
    ] {
        assert_eq!(
            source.matches("struct Camera {").count(),
            1,
            "{label}: exactly one shared Camera struct"
        );
    }

    // The lit passes share one copy of the lighting model and display transform.
    for (label, source) in [
        ("scene", scene_shader_source()),
        ("vehicle", vehicle_shader_source()),
        ("sky", sky_shader_source()),
        ("water", water_shader_source()),
    ] {
        for function in ["fn tonemap_aces(", "fn display_grade(", "fn aces_curve(", "fn apply_fog("]
        {
            assert_eq!(
                source.matches(function).count(),
                1,
                "{label}: exactly one shared {function}"
            );
        }
    }

    // Only the geometry passes whose pipeline layout carries the group-2 environment bind group
    // may declare the shadow/SSAO lookups.
    for (label, source) in [("scene", scene_shader_source()), ("vehicle", vehicle_shader_source())]
    {
        for function in ["fn sun_shadow(", "fn screen_ao("] {
            assert_eq!(
                source.matches(function).count(),
                1,
                "{label}: exactly one shared {function}"
            );
        }
    }
    for (label, source) in [
        ("sky", sky_shader_source()),
        ("water", water_shader_source()),
        ("rain", rain_shader_source()),
    ] {
        assert_eq!(
            source.matches("fn sun_shadow(").count(),
            0,
            "{label}: no group-2 shadow bindings in a pass without that bind group"
        );
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
    let camera_bgl = build_camera_bind_group_layout(&ctx.device);
    let (_, material_bgl) = build_vehicle_pipeline(
        &ctx.device,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        1,
        &shadow_bgl,
        &camera_bgl,
    );
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
