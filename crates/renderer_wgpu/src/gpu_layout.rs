use encase::{ShaderSize, ShaderType, UniformBuffer};
use renderer_api::RenderError;

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

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, ShaderType, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    pub view_proj: GpuMat4,
}

impl CameraUniform {
    pub fn identity() -> Self {
        Self {
            view_proj: GpuMat4([
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ]),
        }
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
