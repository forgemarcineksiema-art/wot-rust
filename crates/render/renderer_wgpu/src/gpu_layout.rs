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
    /// Packed shadow controls: x = shadow-map texel UV size (PCF step), y = depth bias,
    /// z = strength (0 disables — the capability fallback), w = world-space normal offset.
    pub shadow_params: GpuVec4,
    /// Packed SSAO controls: x = near plane, y = far plane (for depth linearization),
    /// z = strength (0 disables — the capability fallback), w = projection Y scale (P[1][1],
    /// recovered from the view-projection) for world-radius → pixel-radius conversion.
    pub ssao_params: GpuVec4,
    /// Gradient-sky zenith colour (linear), sampled straight up by the sky pass.
    pub sky_zenith_rgb: GpuVec3,
    /// Gradient-sky horizon colour (linear); also the aerial-perspective fog colour distant
    /// surfaces fade toward in the lit shaders.
    pub sky_horizon_rgb: GpuVec3,
    /// Packed fog controls: x = density, y = height falloff, z/w reserved (0). Density 0 disables
    /// the aerial perspective (interior looks).
    pub fog_params: GpuVec4,
    /// Packed presentation clock: x = scene time in seconds, y/z/w reserved (0). Every shader
    /// animation (water ripple, foliage sway, weather) advances by this one value. Tick-domain
    /// by doctrine: it is derived from the fixed simulation tick plus the sub-tick render phase,
    /// never integrated from render-frame deltas — a jittery frame clock must not wobble the
    /// world (the same rule `engine::TankMotion` follows).
    pub time_params: GpuVec4,
}

/// The per-frame pass parameters that ride the camera uniform beside the view matrices and
/// lighting: the focused sun-shadow matrix with its packed controls, the SSAO controls, and the
/// tick-domain presentation clock.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FramePassParams {
    pub light_view_proj: [[f32; 4]; 4],
    pub shadow_params: [f32; 4],
    pub ssao_params: [f32; 4],
    pub time_s: f32,
}

impl Default for FramePassParams {
    fn default() -> Self {
        Self {
            light_view_proj: IDENTITY_MATRIX,
            // strength 0: no shadow / no SSAO (the default frame is unshadowed).
            shadow_params: [0.0, 0.0, 0.0, 0.0],
            ssao_params: [0.1, 1500.0, 0.0, 1.0],
            time_s: 0.0,
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
            shadow_params: GpuVec4(passes.shadow_params),
            ssao_params: GpuVec4(passes.ssao_params),
            sky_zenith_rgb: GpuVec3(lighting.sky_zenith_rgb),
            sky_horizon_rgb: GpuVec3(lighting.sky_horizon_rgb),
            fog_params: GpuVec4([lighting.fog_density, lighting.fog_height_falloff, 0.0, 0.0]),
            time_params: GpuVec4([passes.time_s, 0.0, 0.0, 0.0]),
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
