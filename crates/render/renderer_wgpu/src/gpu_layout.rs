use encase::{ShaderSize, ShaderType, UniformBuffer};
use renderer_api::{RenderError, SceneLighting};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TankVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
}

impl TankVertex {
    pub const fn new(position: [f32; 3], normal: [f32; 3]) -> Self {
        Self { position, normal }
    }
}

pub fn tank_vertex_bytes(vertices: &[TankVertex]) -> &[u8] {
    bytemuck::cast_slice(vertices)
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuMat4(pub [[f32; 4]; 4]);

impl AsRef<[[f32; 4]; 4]> for GpuMat4 {
    fn as_ref(&self) -> &[[f32; 4]; 4] {
        &self.0
    }
}

impl AsMut<[[f32; 4]; 4]> for GpuMat4 {
    fn as_mut(&mut self) -> &mut [[f32; 4]; 4] {
        &mut self.0
    }
}

impl From<[[f32; 4]; 4]> for GpuMat4 {
    fn from(columns: [[f32; 4]; 4]) -> Self {
        Self(columns)
    }
}

encase::impl_matrix!(4, 4, GpuMat4, f32; using AsRef AsMut From);

/// A `vec3<f32>`-laid-out value for uniform structs. A bare `[f32; 3]` field would encode as a
/// std140 `array<f32, 3>` (16-byte stride per element), not a `vec3`; this newtype carries the
/// proper `vec3` alignment so the lighting directions/colours match the WGSL `Camera` struct.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuVec3(pub [f32; 3]);

impl AsRef<[f32; 3]> for GpuVec3 {
    fn as_ref(&self) -> &[f32; 3] {
        &self.0
    }
}

impl AsMut<[f32; 3]> for GpuVec3 {
    fn as_mut(&mut self) -> &mut [f32; 3] {
        &mut self.0
    }
}

impl From<[f32; 3]> for GpuVec3 {
    fn from(values: [f32; 3]) -> Self {
        Self(values)
    }
}

encase::impl_vector!(3, GpuVec3, f32; using AsRef AsMut From);

/// A `vec4<f32>`-laid-out value for uniform structs (16-byte aligned), used for packed shadow
/// parameters alongside the `vec3` lighting fields.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuVec4(pub [f32; 4]);

impl AsRef<[f32; 4]> for GpuVec4 {
    fn as_ref(&self) -> &[f32; 4] {
        &self.0
    }
}

impl AsMut<[f32; 4]> for GpuVec4 {
    fn as_mut(&mut self) -> &mut [f32; 4] {
        &mut self.0
    }
}

impl From<[f32; 4]> for GpuVec4 {
    fn from(values: [f32; 4]) -> Self {
        Self(values)
    }
}

encase::impl_vector!(4, GpuVec4, f32; using AsRef AsMut From);

/// The shared camera + lighting uniform bound at group 0, binding 0 for both the scene and the
/// vehicle pipelines. Carries the view-projection, the world-space camera position (for accurate
/// specular view directions), and the [`SceneLighting`] profile (hemispheric sky/ground ambient
/// plus key/fill/rim). Field order is mirrored in both WGSL `Camera` structs and locked by
/// `wgsl_layout`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, ShaderType, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    pub view_proj: GpuMat4,
    /// Inverse of `view_proj`, so a shader can unproject a clip point back to world space. The
    /// gradient-sky pass reconstructs a per-pixel view ray direction from it.
    pub inv_view_proj: GpuMat4,
    pub camera_pos: GpuVec3,
    pub ambient_rgb: GpuVec3,
    pub ground_ambient_rgb: GpuVec3,
    pub key_direction: GpuVec3,
    pub key_rgb: GpuVec3,
    pub fill_direction: GpuVec3,
    pub fill_rgb: GpuVec3,
    pub rim_direction: GpuVec3,
    pub rim_rgb: GpuVec3,
    /// The focused sun shadow map's light view-projection (see `renderer_api::sun_shadow`).
    pub light_view_proj: GpuMat4,
    /// The far cascade's light view-projection: the same texel-snapped sun box, 4.5× wider and
    /// centred further along the look, so terrain and buildings past the near box still cast
    /// (see `SunShadowParams::far_cascade`).
    pub light_view_proj_far: GpuMat4,
    /// Packed shadow controls: x = shadow-map texel UV size (PCF step), y = depth bias,
    /// z = strength (0 disables — the capability fallback), w = world-space normal offset.
    pub shadow_params: GpuVec4,
    /// Packed far-cascade controls: x = far texel UV size, y = far world-space normal offset,
    /// z = cascade count (< 2 disables the far lookup), w = the near box's containment margin in
    /// UV — fragments inside it sample the near map, outside it fall through to the far cascade.
    pub cascade_params: GpuVec4,
    /// Packed SSAO controls: x = near plane, y = far plane (for depth linearization),
    /// z = strength (0 disables — the capability fallback), w = projection Y scale (P[1][1],
    /// recovered from the view-projection) for world-radius → pixel-radius conversion.
    pub ssao_params: GpuVec4,
    /// Gradient-sky zenith colour (linear), sampled straight up by the sky pass.
    pub sky_zenith_rgb: GpuVec3,
    /// Gradient-sky horizon colour (linear); also the aerial-perspective fog colour distant
    /// surfaces fade toward in the lit shaders.
    pub sky_horizon_rgb: GpuVec3,
    /// Packed fog controls + render size: x = density, y = height falloff (density 0 disables
    /// the aerial perspective — interior looks); z/w = inverse render-target width/height, which
    /// `screen_ao` uses to address the (possibly reduced-resolution) AO chain by framebuffer
    /// pixel.
    pub fog_params: GpuVec4,
    /// Packed presentation clock: x = scene time in seconds, y/z/w reserved (0). Every shader
    /// animation (water ripple, foliage sway, weather) advances by this one value. Tick-domain
    /// by doctrine: it is derived from the fixed simulation tick plus the sub-tick render phase,
    /// never integrated from render-frame deltas — a jittery frame clock must not wobble the
    /// world (the same rule `engine::TankMotion` follows).
    pub time_params: GpuVec4,
    /// Packed display grade from the lighting profile: x = exposure (pre-curve HDR multiplier),
    /// y = black point, z = saturation, w = contrast. Mirrored on the CPU by
    /// `SceneLighting::grade_reference`.
    pub grade_params: GpuVec4,
    /// Packed cloud layer from the lighting profile: x = coverage bias, y = pattern scale,
    /// z = opacity, w = drift speed (UV per presentation second).
    pub cloud_params: GpuVec4,
    /// Packed sky/air extras: x = cloud-shadow strength on the terrain key (profile strength,
    /// already gated by `LightingQuality::cloud_shadows` — 0 disables), y = sun-directional
    /// scatter in the aerial perspective, z = bloom composite weight (profile, gated by
    /// `LightingQuality::bloom_mips` — 0 disables), w = display vignette strength.
    pub sky_params: GpuVec4,
    /// Local fill pools ([`renderer_api::LocalLight`]): xyz = world position, w = radius (0
    /// disables the slot). Appended at the END of the struct so no existing offset moves.
    pub light_pos_radius: [GpuVec4; 6],
    /// Local pool colours: xyz = linear rgb, w = intensity multiplier.
    pub light_rgb_intensity: [GpuVec4; 6],
    /// Two-layer air (appended at the END so no existing offset moves): x = valley haze
    /// density, y = valley haze fade-out height (m, 0 disables), z = crepuscular-ray strength
    /// in the post pass (0 skips the march), w = sun-disc softness (profile data — the sky pass
    /// no longer derives it from the fog density).
    pub haze_params: GpuVec4,
    /// Cloud layer 2 (appended): x = high-sheet opacity, y = high-sheet scale, z = storm-front
    /// heading (radians, world XZ), w = storm-front strength (0 disables).
    pub cloud2_params: GpuVec4,
    /// Dynamic match-weather lanes: xy = seeded cloud UV offset, z = standing-water fill,
    /// w = seeded rain time phase. Appended so all established uniform offsets remain stable.
    pub weather_params: GpuVec4,
    /// Vehicles pressing the meadow down (Jedna Trawa P9): xyz = world position, w = crush
    /// radius (0 disables the slot, which is the whole array on every grass-free scene).
    /// Appended, like every lane before it.
    pub crusher_pos_radius: [GpuVec4; renderer_api::MAX_GRASS_CRUSHERS],
    /// The hero probe (Hala 3.0 B2): the garage hall's bounced light at the station as a
    /// six-axis irradiance cube (+x, −x, +y, −y, +z, −z; xyz = linear rgb, w unused). The
    /// vehicle shader blends the three faces its normal leans into — the vehicle-side
    /// equivalent of the scene mesh's baked `bounce` lane. All-zero on every battle frame,
    /// which is a bit-exact no-op in the shader. Appended, like every lane before it.
    pub hero_probe: [GpuVec4; 6],
    /// Per-scene render flags: x = interior detail-normal enable (C1), y = interior
    /// reflection cube bound (D1), z = hero vehicle dust 0..1 (J2), w = sun-shadow penumbra
    /// radius in texels (0 = the battle's shipped kernel). Appended, like every lane before
    /// it.
    pub scene_params: GpuVec4,
}

/// The per-frame pass parameters that ride the camera uniform beside the view matrices and
/// lighting: the focused sun-shadow matrix with its packed controls, the SSAO controls, and the
/// tick-domain presentation clock.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FramePassParams {
    pub light_view_proj: [[f32; 4]; 4],
    /// The far cascade's light view-projection (`CameraUniform::light_view_proj_far`).
    pub light_view_proj_far: [[f32; 4]; 4],
    pub shadow_params: [f32; 4],
    /// Packed far-cascade controls (`CameraUniform::cascade_params`).
    pub cascade_params: [f32; 4],
    pub ssao_params: [f32; 4],
    /// Inverse render-target size (`1/width`, `1/height`) — rides `fog_params.zw` so the shaders
    /// can address reduced-resolution screen targets by framebuffer pixel.
    pub inv_render_size: [f32; 2],
    /// Whether this adapter tier runs terrain cloud shadows (`LightingQuality::cloud_shadows`);
    /// false zeroes the profile's strength in `sky_params.x`.
    pub cloud_shadows_enabled: bool,
    /// Whether this adapter tier runs the bloom chain (`LightingQuality::bloom_mips > 0`);
    /// false zeroes the profile's weight in `sky_params.z`.
    pub bloom_enabled: bool,
    pub time_s: f32,
    /// Rain streak density 0..1 (`time_params.y`); 0 in every non-rain look.
    pub rain_intensity: f32,
    /// World wetness 0..1 (`time_params.z`): rain darkens albedo, sharpens finishes, pools
    /// sheen on level ground — in the scene and vehicle shaders alike.
    pub wetness: f32,
    pub weather_params: [f32; 4],
    /// Per-feature shader-detail mask (`time_params.w`, Żywy Step P0): the lane carries the
    /// bits of `ShaderDetailMask` as a small float integer; shaders test bits independently.
    pub shader_detail: renderer_api::ShaderDetailMask,
    /// Vehicles pressing the meadow down (Jedna Trawa P9), nearest-first: xyz world position,
    /// w crush radius. An all-zero array is a bit-exact no-op — every grass-free scene, and
    /// every battle frame with no tank standing in view of the grass, pays nothing.
    pub crushers: [[f32; 4]; renderer_api::MAX_GRASS_CRUSHERS],
    /// The garage hero probe (`CameraUniform::hero_probe`); all-zero outside the garage.
    pub hero_probe: [[f32; 4]; 6],
    /// Per-scene render flags (`CameraUniform::scene_params`); all-zero outside the garage.
    pub scene_params: [f32; 4],
}

impl Default for FramePassParams {
    fn default() -> Self {
        Self {
            light_view_proj: IDENTITY_MATRIX,
            light_view_proj_far: IDENTITY_MATRIX,
            // strength 0: no shadow / no SSAO (the default frame is unshadowed).
            shadow_params: [0.0, 0.0, 0.0, 0.0],
            cascade_params: [0.0, 0.0, 0.0, 0.0],
            ssao_params: [0.1, 1500.0, 0.0, 1.0],
            inv_render_size: [0.0, 0.0],
            cloud_shadows_enabled: true,
            bloom_enabled: true,
            time_s: 0.0,
            rain_intensity: 0.0,
            wetness: 0.0,
            weather_params: [0.0; 4],
            shader_detail: renderer_api::ShaderDetailMask::FULL,
            crushers: [[0.0; 4]; renderer_api::MAX_GRASS_CRUSHERS],
            hero_probe: [[0.0; 4]; 6],
            scene_params: [0.0; 4],
        }
    }
}

impl CameraUniform {
    /// Build the uniform from a view-projection, the world-space camera position, a lighting
    /// profile, and the frame's pass parameters — the single place the backend-neutral
    /// [`SceneLighting`] becomes GPU bytes.
    pub fn from_scene(
        view_proj: [[f32; 4]; 4],
        inv_view_proj: [[f32; 4]; 4],
        camera_pos: [f32; 3],
        lighting: &SceneLighting,
        passes: FramePassParams,
    ) -> Self {
        Self {
            view_proj: GpuMat4(view_proj),
            inv_view_proj: GpuMat4(inv_view_proj),
            camera_pos: GpuVec3(camera_pos),
            ambient_rgb: GpuVec3(lighting.ambient_rgb),
            ground_ambient_rgb: GpuVec3(lighting.ground_ambient_rgb),
            key_direction: GpuVec3(lighting.key_direction),
            key_rgb: GpuVec3(lighting.key_rgb),
            fill_direction: GpuVec3(lighting.fill_direction),
            fill_rgb: GpuVec3(lighting.fill_rgb),
            rim_direction: GpuVec3(lighting.rim_direction),
            rim_rgb: GpuVec3(lighting.rim_rgb),
            light_view_proj: GpuMat4(passes.light_view_proj),
            light_view_proj_far: GpuMat4(passes.light_view_proj_far),
            shadow_params: GpuVec4(passes.shadow_params),
            cascade_params: GpuVec4(passes.cascade_params),
            ssao_params: GpuVec4(passes.ssao_params),
            sky_zenith_rgb: GpuVec3(lighting.sky_zenith_rgb),
            sky_horizon_rgb: GpuVec3(lighting.sky_horizon_rgb),
            fog_params: GpuVec4([
                lighting.fog_density,
                lighting.fog_height_falloff,
                passes.inv_render_size[0],
                passes.inv_render_size[1],
            ]),
            time_params: GpuVec4([
                passes.time_s,
                passes.rain_intensity,
                passes.wetness,
                passes.shader_detail.0 as f32,
            ]),
            grade_params: GpuVec4([
                lighting.exposure,
                lighting.black_point,
                lighting.saturation,
                lighting.contrast,
            ]),
            cloud_params: GpuVec4([
                lighting.cloud_coverage_bias,
                lighting.cloud_scale,
                lighting.cloud_opacity,
                lighting.cloud_drift,
            ]),
            sky_params: GpuVec4([
                if passes.cloud_shadows_enabled { lighting.cloud_shadow_strength } else { 0.0 },
                lighting.fog_sun_scatter,
                if passes.bloom_enabled { lighting.bloom_weight } else { 0.0 },
                lighting.vignette,
            ]),
            light_pos_radius: lighting.local_lights.map(|light| {
                GpuVec4([light.position[0], light.position[1], light.position[2], light.radius_m])
            }),
            light_rgb_intensity: lighting
                .local_lights
                .map(|light| GpuVec4([light.rgb[0], light.rgb[1], light.rgb[2], light.intensity])),
            haze_params: GpuVec4([
                lighting.valley_haze_density,
                lighting.valley_haze_height_m,
                // One-look: the 8-tap crepuscular march ships only in the dev rich profile —
                // fullscreen taps the minimum spec cannot afford, so nobody ships them.
                if passes.shader_detail.has(renderer_api::ShaderDetailMask::GOD_RAYS) {
                    lighting.god_ray_strength
                } else {
                    0.0
                },
                lighting.sun_softness,
            ]),
            cloud2_params: GpuVec4([
                lighting.cloud_sheet_opacity,
                lighting.cloud_sheet_scale,
                lighting.storm_front_dir_rad,
                lighting.storm_front_strength,
            ]),
            weather_params: GpuVec4(passes.weather_params),
            crusher_pos_radius: passes.crushers.map(GpuVec4),
            hero_probe: passes.hero_probe.map(GpuVec4),
            scene_params: GpuVec4(passes.scene_params),
        }
    }

    pub fn identity() -> Self {
        Self::from_scene(
            IDENTITY_MATRIX,
            IDENTITY_MATRIX,
            [0.0, 0.0, 0.0],
            &SceneLighting::battlefield_default(),
            FramePassParams::default(),
        )
    }

    pub fn wgsl_size() -> usize {
        Self::SHADER_SIZE.get() as usize
    }
}

const IDENTITY_MATRIX: [[f32; 4]; 4] =
    [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]];

pub fn encode_camera_uniform(camera: &CameraUniform) -> Result<Vec<u8>, RenderError> {
    let mut buffer = UniformBuffer::new(Vec::new());
    buffer.write(camera).map_err(|error| RenderError::new(error.to_string()))?;
    Ok(buffer.into_inner())
}
