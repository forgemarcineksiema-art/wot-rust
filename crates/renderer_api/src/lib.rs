mod bindings;
mod capabilities;
mod debug_tools;
mod feature_plan;
mod limits;
mod pipeline;
mod projection;
mod resources;
mod scene;
mod vehicle;
mod vehicle_asset;

use game_core::TankId;

pub const DEFAULT_MSAA_SAMPLES: u8 = 4;

pub use bindings::{
    BindGroupRole, BindGroupSlot, RendererBindingPolicy, TextureBindingStrategy,
    baseline_bind_group_layout, baseline_binding_policy, binding_policy_for_feature_plan,
};
pub use capabilities::{
    GpuBackend, GpuDeviceType, RenderAdapterReport, RenderCapabilityTier, RenderLimitsSummary,
    TextureCompressionSupport,
};
pub use debug_tools::{
    DebugDrawBatch, DebugDrawCommand, DebugDrawKind, DebugToolKind, DebugToolPlan, RgbaDebugColor,
};
pub use feature_plan::{
    FallbackReason, FeatureFallback, RenderFeature, RenderFeaturePlan, select_render_feature_plan,
};
pub use limits::RenderLimitProfile;
pub use pipeline::{
    AlphaMode, ColorFormat, DepthFormat, MaterialPipelineFlags, PipelineCacheMode, PipelineKey,
    PipelineWarmupPlan, ShaderHandle, VertexLayoutKey,
};
pub use projection::{CameraProjectionPolicy, DepthRange};
pub use resources::{MaterialDescriptor, MeshAsset, MeshRegistry, RenderMaterialRegistry};
pub use scene::{HUD_SOLID_UV, HudVertex, SceneVertex, view_projection_matrix};
pub use vehicle::{MAPPING_PARAMETRIC, MAPPING_TRIPLANAR, VehicleVertex, generate_tangents};
pub use vehicle_asset::{
    VehicleMaterialDescriptor, VehicleMaterialFamilies, VehicleMaterialMaps, VehicleMeshAsset,
    VehicleTextureMap,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderSettings {
    pub width: u32,
    pub height: u32,
    pub vsync: bool,
    pub limit_profile: RenderLimitProfile,
    pub msaa_samples: u8,
}

impl Default for RenderSettings {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            vsync: true,
            limit_profile: RenderLimitProfile::LowSpec,
            msaa_samples: DEFAULT_MSAA_SAMPLES,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    pub eye: [f32; 3],
    pub target: [f32; 3],
    pub vertical_fov_degrees: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self { eye: [0.0, 8.0, -12.0], target: [0.0, 0.0, 0.0], vertical_fov_degrees: 60.0 }
    }
}

/// Calibrated three-point scene lighting: a neutral ambient plus key/fill/rim directional lights,
/// consumed by both the scene and the vehicle shaders. Each `*_direction` is a world-space vector
/// pointing *towards* the light (the shader normalizes it); each `*_rgb` is that light's linear
/// colour and intensity. This replaces the former global colour multiplier, so a warm garage key
/// no longer casts amber over the whole battlefield — the look is chosen per scene by profile.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SceneLighting {
    pub ambient_rgb: [f32; 3],
    pub key_direction: [f32; 3],
    pub key_rgb: [f32; 3],
    pub fill_direction: [f32; 3],
    pub fill_rgb: [f32; 3],
    pub rim_direction: [f32; 3],
    pub rim_rgb: [f32; 3],
}

impl SceneLighting {
    /// The battlefield look: a warm overhead sun key, a cool sky-fill from straight above, neutral
    /// ambient and no rim. Calibrated to preserve the pre-lighting battle appearance (no cast).
    pub fn battlefield_default() -> Self {
        Self {
            ambient_rgb: [0.33, 0.34, 0.36],
            key_direction: [0.45, 0.82, 0.35],
            key_rgb: [0.85, 0.83, 0.78],
            fill_direction: [0.0, 1.0, 0.0],
            fill_rgb: [0.12, 0.13, 0.15],
            // A direction is needed for normalization, but the battle rim is off (black).
            rim_direction: [-0.4, 0.5, -0.9],
            rim_rgb: [0.0, 0.0, 0.0],
        }
    }

    /// The garage studio: a soft warm key from front-left-above, a weak cool fill from the right,
    /// and a restrained rear rim to lift the silhouette, all on a neutral ambient so the vehicle's
    /// own material colour reads true. The result is a neutral tint with shaped studio light.
    pub fn garage_studio() -> Self {
        Self {
            ambient_rgb: [0.30, 0.30, 0.32],
            key_direction: [-0.55, 0.72, 0.45],
            key_rgb: [0.92, 0.84, 0.68],
            fill_direction: [0.95, 0.25, 0.10],
            fill_rgb: [0.20, 0.24, 0.30],
            rim_direction: [0.15, 0.55, -0.95],
            rim_rgb: [0.26, 0.26, 0.30],
        }
    }
}

impl Default for SceneLighting {
    fn default() -> Self {
        Self::battlefield_default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MeshHandle(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MaterialHandle(pub u32);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderObject {
    pub tank_id: Option<TankId>,
    pub mesh: MeshHandle,
    pub material: MaterialHandle,
    pub transform: [[f32; 4]; 4],
    /// Per-instance team/ownership tint, multiplied into tint-weighted vertices by the shader.
    pub tint: [f32; 3],
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct RenderFrame {
    pub camera: Camera,
    pub objects: Vec<RenderObject>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderError {
    pub message: String,
}

impl RenderError {
    pub fn new(message: impl Into<String>) -> Self {
        Self { message: message.into() }
    }
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for RenderError {}

pub trait RenderBackend {
    fn name(&self) -> &'static str;
    fn adapter_report(&self) -> Option<&RenderAdapterReport> {
        None
    }
    fn resize(&mut self, width: u32, height: u32);
    fn render_frame(&mut self, frame: &RenderFrame) -> Result<(), RenderError>;
}
