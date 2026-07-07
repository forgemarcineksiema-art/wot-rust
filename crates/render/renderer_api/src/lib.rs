mod bindings;
mod capabilities;
mod debug_tools;
mod feature_plan;
mod lighting;
mod limits;
mod pipeline;
mod projection;
mod resources;
mod scene;
mod sun_shadow;
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
pub use lighting::SceneLighting;
pub use limits::RenderLimitProfile;
pub use pipeline::{
    AlphaMode, ColorFormat, DepthFormat, MaterialPipelineFlags, PipelineCacheMode, PipelineKey,
    PipelineWarmupPlan, ShaderHandle, VertexLayoutKey,
};
pub use projection::{CameraProjectionPolicy, DepthRange};
pub use resources::{MaterialDescriptor, MeshAsset, MeshRegistry, RenderMaterialRegistry};
pub use scene::{
    FxVertex, HUD_SOLID_UV, HudVertex, SceneVertex, WaterVertex, view_projection_inverse,
    view_projection_matrix,
};
pub use sun_shadow::{SunShadowParams, forward_shadow_focus, sun_light_view_projection};
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
