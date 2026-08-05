use renderer_api::{
    BindGroupRole, FxVertex, VehicleVertex, baseline_bind_group_layout, surface_role,
};
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

/// Read a `const <name>: f32 = <value>;` straight out of a shader. Parsing the real source
/// (rather than restating the literal) means these locks fail when the shader moves, which
/// is the entire point of a CPU↔GPU contract test.
fn wgsl_const(source: &str, name: &str) -> f32 {
    let needle = format!("const {name}: f32 = ");
    let start = source.find(&needle).unwrap_or_else(|| panic!("{name} is missing from the shader"))
        + needle.len();
    let rest = &source[start..];
    let end = rest.find(';').expect("a terminated const");
    rest[..end].trim().parse().expect("a numeric const")
}

#[test]
fn grass_blade_shader_contract_fades_the_tuft_and_scales_its_wind() {
    let source = scene_shader_source();

    // The role value is an append-only CPU/GPU protocol, so lock both ends together.
    assert_eq!(surface_role::GRASS_CARD, 5.0);
    assert_eq!(surface_role::GRASS_BLADE, 6.0);
    assert!(source.contains("abs(input.surface - 6.0) < 0.5"));

    // The costume hand-off (Jedna Trawa P4): ONE function decides where grass changes
    // costume — the near ring takes its value, the far meadow takes the complement, so
    // stand_near + stand_far ≡ 1 at every place by construction, and the radius is a
    // world-anchored coastline (noise), never a ring line.
    assert!(source.contains("fn grass_handoff_stand"));
    assert!(source.contains("stand = grass_handoff_stand(root.xz, camera.camera_pos.xz);"));
    assert!(source.contains("(1.0 - grass_handoff_stand(world.xz, camera.camera_pos.xz))"));
    assert!(source.contains("root + (world.xyz - root) * stand"));

    // The coastline's outermost reach stays inside the shader ring the CPU cache's
    // anti-streaming lock is written against (scene_build's GRASS_RADIUS_M — restated here
    // because the renderer sits BELOW scene_build in the layer DAG and cannot import it).
    const CPU_SHADER_RING_M: f32 = 48.0;
    let base = wgsl_const(&source, "GRASS_HANDOFF_BASE_M");
    let spread = wgsl_const(&source, "GRASS_HANDOFF_SPREAD_M");
    let half_band = wgsl_const(&source, "GRASS_HANDOFF_HALF_BAND_M");
    assert!(
        base + spread + half_band < CPU_SHADER_RING_M,
        "the coastline's farthest reach ({}) must stay inside the {CPU_SHADER_RING_M} m ring \
         the CPU population is cached for",
        base + spread + half_band
    );

    // Zoom-aware bands (D3/P4b): the shader's magnification constants ARE the CPU's, read
    // from the projection lane the renderer already fills — one number, no camera state
    // synchronised across the boundary.
    assert_eq!(
        wgsl_const(&source, "GRASS_ZOOM_REFERENCE_PROJ_Y"),
        renderer_api::GRASS_ZOOM_REFERENCE_PROJ_Y
    );
    assert_eq!(wgsl_const(&source, "GRASS_ZOOM_BAND_CAP"), renderer_api::GRASS_ZOOM_BAND_CAP);
    assert!(source.contains("camera.ssao_params.w / GRASS_ZOOM_REFERENCE_PROJ_Y"));
    // Only the FAR collapse stretches. The near hand-off must not: the ring is a CPU cache
    // of fixed world radius whose instance count grows with the square of any stretch.
    assert!(source.contains("meadow_far_stand(d)"));
    assert!(source.contains("MEADOW_FAR_COLLAPSE_START_M * zoom"));
    assert!(
        !source.contains("GRASS_HANDOFF_BASE_M * zoom"),
        "stretching the near ring would multiply the CPU population, not the view"
    );

    // Wind displacement is mesh-local, scaled into world metres, and dies with the tuft.
    assert!(source.contains("let model_scale = length(model[1].xyz);"));
    assert!(source.contains("let scaled_sway = input.sway * model_scale * stand;"));
    assert!(
        !source.contains("gust * input.sway"),
        "raw sway would turn a small faded tuft into a long wind-blown needle"
    );

    // Grass geometry roles must not fall through to bark or the costlier generic ground path;
    // the single octave rides the shared fine frame so the cards match the ground's grain.
    assert!(source.contains("if (role > 4.5)"));
    assert!(
        source.contains("return 0.94 + value_noise(octave_frame_fine(world.xz) * 1.7) * 0.12;")
    );
}

/// Costume C (P5): the ground carries the meadow's darkness, and the curve it takes over on
/// is the SAME one the scene pass folds the far tufts with — one shared fragment, composed
/// into both passes, so the collapse cannot drift into a visible horizon.
#[test]
fn the_ground_takes_over_the_meadow_where_the_far_tufts_fold() {
    let scene = scene_shader_source();
    let terrain = terrain_shader_source();

    // ONE definition, two consumers: the shared fragment declares the curve, and each pass
    // calls it rather than restating a threshold of its own.
    for (pass, source) in [("scene", &scene), ("terrain", &terrain)] {
        assert_eq!(
            source.matches("fn meadow_far_stand").count(),
            1,
            "{pass} composes exactly one copy of the shared meadow fragment"
        );
        assert_eq!(source.matches("fn grass_zoom_band_scale").count(), 1);
    }
    assert!(
        terrain.contains("meadow_ground_shade(w.r + w.g, eye_dist)"),
        "the ground's meadow share is vegetation-weighted and distance-aware"
    );

    // The shade constants are the CPU's, and they mean what the doctrine says: the ground
    // carries MORE of the meadow once the tufts in front of it are gone.
    let standing = wgsl_const(&terrain, "MEADOW_SHADE_STANDING");
    let collapsed = wgsl_const(&terrain, "MEADOW_SHADE_COLLAPSED");
    assert_eq!(standing, renderer_api::MEADOW_SHADE_STANDING);
    assert_eq!(collapsed, renderer_api::MEADOW_SHADE_COLLAPSED);
    assert!(collapsed > standing, "a folded meadow leaves MORE for the ground to carry");

    // The hand-over itself: bare ground is never touched, and vegetated ground darkens as
    // the far costume folds — continuously, with no step at the collapse band.
    assert_eq!(renderer_api::meadow_ground_shade(0.0, 0.0), 1.0, "a road is never shaded");
    let near = renderer_api::meadow_ground_shade(1.0, 1.0);
    let far = renderer_api::meadow_ground_shade(1.0, 0.0);
    assert!(far < near, "the ground darkens as the tufts fold: {near} -> {far}");
    let midpoint = renderer_api::meadow_ground_shade(1.0, 0.5);
    assert!(
        (midpoint - (near + far) * 0.5).abs() < 1.0e-6,
        "the take-over is linear in the far costume's presence — no step to see"
    );
}

/// The magnification factor itself (P4b): every off-game camera lands on exactly 1.0, the
/// scope ladder stretches the bands, and the cap holds at its narrowest steps.
#[test]
fn the_grass_band_magnification_is_one_off_scope_and_capped_on_it() {
    let proj_y = |fov_degrees: f32| 1.0 / (fov_degrees.to_radians() * 0.5).tan();
    for battle_fov in [55.0, 60.0, 65.0, 75.0] {
        assert_eq!(
            renderer_api::grass_zoom_band_scale(proj_y(battle_fov)),
            1.0,
            "a normal battle view at {battle_fov}° is not magnified"
        );
    }
    // The scope ladder (client `SNIPER_FOV_STEPS_DEGREES`): 18° is the entry step.
    let entry = renderer_api::grass_zoom_band_scale(proj_y(18.0));
    assert!((3.0..3.6).contains(&entry), "the entry scope step stretches ~3.3x: {entry}");
    for narrow in [8.0, 5.0, 3.0] {
        assert_eq!(
            renderer_api::grass_zoom_band_scale(proj_y(narrow)),
            renderer_api::GRASS_ZOOM_BAND_CAP,
            "the deep steps hold at the cap instead of asking for kilometres of grass"
        );
    }
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
    let source = fx_shader_source();
    let report = validate_wgsl_shader("fx", &source).expect("fx shader validates");

    assert!(report.entry_points.iter().any(|entry| entry == "vs_main"));
    assert!(report.entry_points.iter().any(|entry| entry == "fs_main"));
    // The FX pipeline reuses the scene camera bind group, so its uniform must sit at the same
    // slot the scene pipeline binds (group 0, binding 0).
    assert!(report.has_uniform_binding("camera", 0, 0));

    // Teren F3 (slice 1): PHYSICAL media (alpha carries coverage) breathe the world's air
    // through the ONE shared fog implementation, while pure-additive glow — tracers, fire —
    // stays unfogged: an emissive read at range is a gameplay promise.
    assert!(source.contains("apply_fog"), "physical FX must fog through the shared lane");
    assert!(source.contains("if (color.a > 0.011)"), "additive glow must pass the fog untouched");
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
fn each_near_shadow_pcf_tier_normalizes_its_own_tap_count() {
    // The two tiers are separate paths now (the reduced one rotates its cross per pixel), so
    // each carries its own explicit normalizer — the invariant is that BOTH exist: a shared
    // divisor would over- or under-normalize whichever tier it was not written for.
    let source = scene_shader_source();

    assert!(source.contains("sum / 9.0"), "the wide 3x3 tier must normalize its nine taps");
    assert!(source.contains("sum / 4.0"), "the reduced tier must normalize its four taps");
    // The reduced tier's four taps sit on the per-pixel rotated cross — the fixed sparse
    // lattice printed a weave into every penumbra (D32), so the rotation is load-bearing.
    assert!(
        source.contains("let rot = pcf_rotation(frag.xy);"),
        "the reduced PCF cross must rotate per pixel"
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
fn terrain_furrows_are_plot_oriented_gated_and_distance_faded() {
    let terrain = terrain_shader_source();

    // Teren B1's three promises: the feature hides behind its redemption bit (never in the
    // shipped mask without a measurement), each plot ploughs by its OWN hash — never a
    // world axis — and the stripes die by 150 m so the far field cannot shimmer (rule 5).
    assert!(
        terrain.contains("detail_bit(64u)"),
        "furrows must stay behind the TERRAIN_FURROWS redemption bit"
    );
    assert!(
        terrain.contains("floor(plough_lane * 8.0)"),
        "the plough direction is quantized from the plot's own hash"
    );
    assert!(
        terrain.contains("detail_hash(vec2<f32>(cell * 5.7"),
        "the direction hash reads the quilt's plot identity, not a world axis"
    );
    assert!(
        terrain.contains("smoothstep(60.0, 150.0, eye_dist)) * furrow_mask"),
        "the stripes must fade out with distance"
    );
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

    // The ground grain is ONE implementation (noise_common.wgsl): the terrain pass and the
    // statics standing on it must never grow a second hash/lattice copy, or their grains
    // silently drift apart and the ground stops reading as one picture.
    for (label, source) in [("scene", scene_shader_source()), ("terrain", terrain_shader_source())]
    {
        for function in
            ["fn value_noise(", "fn value_noise_grad(", "fn ground_grain(", "fn puddle_pool("]
        {
            assert_eq!(
                source.matches(function).count(),
                1,
                "{label}: exactly one shared {function}"
            );
        }
    }
}

#[test]
fn ground_grain_is_lattice_decorrelated() {
    // The ground twin of `sky_cloud_field_is_lattice_decorrelated`. The pixelated-square
    // ground had the same three roots as the square sky, each locked here:
    for (label, source) in [("scene", scene_shader_source()), ("terrain", terrain_shader_source())]
    {
        // 1. The world-metre lattices must not hash through sin — fract(sin(dot)) collapses
        //    into flat hard-edged plates once its argument leaves the GPU sin's accurate
        //    range, which world.xz * 1.7 does within metres of the origin. The sin hash
        //    survives ONLY as the small-index corner-tone picker (detail_hash).
        let noise_body = source
            .split("fn value_noise")
            .nth(1)
            .expect("value_noise present")
            .split("fn ")
            .next()
            .expect("function body");
        assert!(
            !noise_body.contains("sin("),
            "{label}: the value-noise lattice hash must be integer-domain, not sin"
        );
        // 2. The detail octaves must ride rotated frames, never the bare world axes —
        //    axis-aligned octaves all reinforce ONE square lattice.
        assert!(
            source.contains("octave_frame_broad(world_xz) * 0.4")
                && source.contains("octave_frame_fine(world_xz) * 1.7")
                && source.contains("octave_frame_broad(world_xz) * 3.2"),
            "{label}: every ground octave must sample a rotated frame"
        );
        // 3. The light-catching bend must be the ANALYTIC gradient. A finite difference
        //    stepped at over half a lattice cell facets the grain into square plates.
        assert!(
            source.contains("fn value_noise_grad("),
            "{label}: the grain gradient must be analytic"
        );
        assert!(
            !source.contains("let e = 0.35;"),
            "{label}: the coarse finite-difference step must not return"
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
