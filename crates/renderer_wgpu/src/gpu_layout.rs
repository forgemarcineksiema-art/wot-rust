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

/// The shared camera + lighting uniform bound at group 0, binding 0 for both the scene and the
/// vehicle pipelines. Carries the view-projection, the world-space camera position (for accurate
/// specular view directions), and the calibrated three-point [`SceneLighting`].
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, ShaderType, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    pub view_proj: GpuMat4,
    pub camera_pos: GpuVec3,
    pub ambient_rgb: GpuVec3,
    pub key_direction: GpuVec3,
    pub key_rgb: GpuVec3,
    pub fill_direction: GpuVec3,
    pub fill_rgb: GpuVec3,
    pub rim_direction: GpuVec3,
    pub rim_rgb: GpuVec3,
}

impl CameraUniform {
    /// Build the uniform from a view-projection, the world-space camera position, and a lighting
    /// profile — the single place the backend-neutral [`SceneLighting`] becomes GPU bytes.
    pub fn from_scene(
        view_proj: [[f32; 4]; 4],
        camera_pos: [f32; 3],
        lighting: &SceneLighting,
    ) -> Self {
        Self {
            view_proj: GpuMat4(view_proj),
            camera_pos: GpuVec3(camera_pos),
            ambient_rgb: GpuVec3(lighting.ambient_rgb),
            key_direction: GpuVec3(lighting.key_direction),
            key_rgb: GpuVec3(lighting.key_rgb),
            fill_direction: GpuVec3(lighting.fill_direction),
            fill_rgb: GpuVec3(lighting.fill_rgb),
            rim_direction: GpuVec3(lighting.rim_direction),
            rim_rgb: GpuVec3(lighting.rim_rgb),
        }
    }

    pub fn identity() -> Self {
        Self::from_scene(
            [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            [0.0, 0.0, 0.0],
            &SceneLighting::battlefield_default(),
        )
    }

    pub fn wgsl_size() -> usize {
        Self::SHADER_SIZE.get() as usize
    }
}

pub fn encode_camera_uniform(camera: &CameraUniform) -> Result<Vec<u8>, RenderError> {
    let mut buffer = UniformBuffer::new(Vec::new());
    buffer.write(camera).map_err(|error| RenderError::new(error.to_string()))?;
    Ok(buffer.into_inner())
}
