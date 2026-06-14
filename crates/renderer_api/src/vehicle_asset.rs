use crate::VehicleVertex;

#[derive(Debug, Clone, PartialEq)]
pub struct VehicleMeshAsset {
    vertices: Vec<VehicleVertex>,
    indices: Vec<u32>,
}

impl VehicleMeshAsset {
    pub fn new(vertices: Vec<VehicleVertex>, indices: Vec<u32>) -> Self {
        Self { vertices, indices }
    }

    pub fn vertices(&self) -> &[VehicleVertex] {
        &self.vertices
    }

    pub fn indices(&self) -> &[u32] {
        &self.indices
    }

    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    pub fn index_count(&self) -> usize {
        self.indices.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VehicleMaterialDescriptor {
    label: String,
    albedo_texture: String,
    normal_texture: String,
    ao_roughness_texture: String,
    cavity_texture: Option<String>,
}

impl VehicleMaterialDescriptor {
    pub fn pbr_lite(
        label: impl Into<String>,
        albedo_texture: impl Into<String>,
        normal_texture: impl Into<String>,
        ao_roughness_texture: impl Into<String>,
        cavity_texture: Option<impl Into<String>>,
    ) -> Self {
        Self {
            label: label.into(),
            albedo_texture: albedo_texture.into(),
            normal_texture: normal_texture.into(),
            ao_roughness_texture: ao_roughness_texture.into(),
            cavity_texture: cavity_texture.map(Into::into),
        }
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn albedo_texture(&self) -> &str {
        &self.albedo_texture
    }

    pub fn normal_texture(&self) -> &str {
        &self.normal_texture
    }

    pub fn ao_roughness_texture(&self) -> &str {
        &self.ao_roughness_texture
    }

    pub fn cavity_texture(&self) -> Option<&str> {
        self.cavity_texture.as_deref()
    }
}
