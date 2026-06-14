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

/// A single decoded RGBA8 texture map ready for GPU upload. Decoupled from any image codec so the
/// renderer backend stays format-agnostic; the catalog decodes baked PNGs into this shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VehicleTextureMap {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

impl VehicleTextureMap {
    /// Builds a map from tightly packed RGBA8 rows. Panics if the buffer is not exactly
    /// `width * height * 4` bytes, since a mismatch would corrupt the upload.
    pub fn new(width: u32, height: u32, rgba: Vec<u8>) -> Self {
        assert!(width > 0 && height > 0, "vehicle texture map must be non-empty");
        assert_eq!(
            rgba.len(),
            width as usize * height as usize * 4,
            "vehicle texture map must be tightly packed RGBA8"
        );
        Self { width, height, rgba }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }
}

/// The decoded PBR-lite material maps for one vehicle: albedo, tangent-space normal, packed
/// AO/roughness/metalness, and an optional cavity map. The renderer uploads these into the
/// vehicle material bind group; missing maps fall back to neutral debug textures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VehicleMaterialMaps {
    albedo: VehicleTextureMap,
    normal: VehicleTextureMap,
    ao_roughness: VehicleTextureMap,
    cavity: Option<VehicleTextureMap>,
}

impl VehicleMaterialMaps {
    pub fn new(
        albedo: VehicleTextureMap,
        normal: VehicleTextureMap,
        ao_roughness: VehicleTextureMap,
        cavity: Option<VehicleTextureMap>,
    ) -> Self {
        Self { albedo, normal, ao_roughness, cavity }
    }

    pub fn albedo(&self) -> &VehicleTextureMap {
        &self.albedo
    }

    pub fn normal(&self) -> &VehicleTextureMap {
        &self.normal
    }

    pub fn ao_roughness(&self) -> &VehicleTextureMap {
        &self.ao_roughness
    }

    pub fn cavity(&self) -> Option<&VehicleTextureMap> {
        self.cavity.as_ref()
    }
}
