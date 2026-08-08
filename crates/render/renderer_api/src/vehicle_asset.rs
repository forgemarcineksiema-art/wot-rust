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

/// The role-aware material families for one vehicle, in `material_id` layer order (rolled armour,
/// cast armour, barrel steel, track metal, rubber). The renderer stacks each map kind into a texture
/// array the vehicle shader indexes by `material_id`, so a vehicle stays one material binding rather
/// than one mesh handle per role.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VehicleMaterialFamilies {
    families: Vec<VehicleMaterialMaps>,
}

impl VehicleMaterialFamilies {
    /// The fixed number of material-texture layers.
    ///
    /// NOT the same thing as the number of material ROLES, and the difference is the bug this
    /// number grew to fix. There are twelve roles and there were five layers, so the shader
    /// clamped with `min(material_id, 4u)` and every role from 5 up — the interior three, torn
    /// steel, and the three whose own declarations argue at length that they must be distinct
    /// (`Canvas`, `Glass`, `Timber`) — sampled the RUBBER layer. A headlight lens wore a tyre.
    ///
    /// Canvas, glass and timber now have layers of their own; the interior roles and torn steel
    /// map onto the nearest real family by an explicit table (`client::material_layer_id`), not
    /// by a clamp. See `client/tests/vehicle_material_ids.rs`, which holds that table and the
    /// shader to each other.
    pub const LAYERS: usize = 8;

    /// Build from exactly [`LAYERS`](Self::LAYERS) families in layer order. Panics otherwise, since a
    /// mismatched layer count would desynchronise the shader's layer indexing.
    pub fn new(families: Vec<VehicleMaterialMaps>) -> Self {
        assert_eq!(
            families.len(),
            Self::LAYERS,
            "a vehicle carries exactly {} material layers",
            Self::LAYERS
        );
        Self { families }
    }

    pub fn families(&self) -> &[VehicleMaterialMaps] {
        &self.families
    }

    /// The family at `material_id` (clamped into range so a stray id cannot index out of bounds).
    pub fn layer(&self, material_id: usize) -> &VehicleMaterialMaps {
        &self.families[material_id.min(Self::LAYERS - 1)]
    }
}
