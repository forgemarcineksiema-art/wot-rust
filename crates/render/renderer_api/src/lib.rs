mod bindings;
mod capabilities;
mod culling;
mod debug_tools;
mod feature_plan;
mod lighting;
mod lighting_blend;
mod lighting_quality;
mod limits;
mod pipeline;
mod projection;
mod resources;
mod scene;
mod sun_shadow;
mod terrain_material;
mod texture;
mod vehicle;
mod vehicle_asset;

use game_core::TankId;
/// Re-exported so renderer backends can key per-vehicle frame state (e.g. which tanks press
/// the meadow down) without depending on the sim crate directly.
pub use game_core::TankId as VehicleId;

pub const DEFAULT_MSAA_SAMPLES: u8 = 4;

pub use bindings::{
    BindGroupRole, BindGroupSlot, RendererBindingPolicy, TextureBindingStrategy,
    baseline_bind_group_layout, baseline_binding_policy, binding_policy_for_feature_plan,
};
pub use capabilities::{
    GpuBackend, GpuDeviceType, RenderAdapterReport, RenderCapabilityTier, RenderLimitsSummary,
    TextureCompressionSupport,
};
pub use culling::{Aabb, Frustum, SceneChunk, chunk_scene_indices, scene_mesh_fingerprint};
pub use debug_tools::{
    DebugDrawBatch, DebugDrawCommand, DebugDrawKind, DebugToolKind, DebugToolPlan, RgbaDebugColor,
};
pub use feature_plan::{
    FallbackReason, FeatureFallback, RenderFeature, RenderFeaturePlan, select_render_feature_plan,
};
pub use lighting::{
    LocalLight, MAX_LOCAL_LIGHTS, NO_LOCAL_LIGHTS, SceneLighting, fluorescent_flicker,
};
pub use lighting_quality::{LightingQuality, ShaderDetailMask};
pub use limits::RenderLimitProfile;
pub use pipeline::{
    AlphaMode, ColorFormat, DepthFormat, MaterialPipelineFlags, PipelineCacheMode, PipelineKey,
    PipelineWarmupPlan, ShaderHandle, VertexLayoutKey,
};
pub use projection::{CameraProjectionPolicy, DepthRange};
pub use resources::{MaterialDescriptor, MeshAsset, MeshRegistry, RenderMaterialRegistry};
pub use scene::{
    FxVertex, HUD_SOLID_UV, HudVertex, SceneVertex, WaterVertex, surface_role,
    view_projection_inverse, view_projection_matrix,
};
pub use sun_shadow::{SunShadowParams, forward_shadow_focus, sun_light_view_projection};
pub use terrain_material::{TERRAIN_LAYERS, TerrainGroundMaps, TerrainLayer, TerrainMaterialSet};
pub use texture::{ALPHA_CUTOUT, MipMode, Rgba8MipChain, Rgba8MipLevel};
pub use vehicle::{
    ArmorApertureRender, ArmorDamageInstance, ArmorMarkKind, MAPPING_PARAMETRIC, MAPPING_TRIPLANAR,
    VehicleVertex, generate_tangents,
};
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

/// Scene meshes registered at or above this handle are shadowless dressing (near-field grass
/// and future ground clutter): they draw in the color pass but skip every depth-only pass —
/// sun-shadow cascades and the SSAO prepass alike. Thin instanced blades in a 4-cascade
/// shadow render are all cost and all shimmer; the ground under them already carries the
/// darkness that matters.
pub const SHADOWLESS_DRESSING_MESH_BASE: u32 = 0xFFFF_0000;

/// The projection Y scale (`P[1][1]` = cot(fov_y / 2)) of the widest normal battle view.
/// Anything narrower than this is MAGNIFIED — the sniper scope, and nothing else.
/// cot(48°/2): the Świat 2.0 third-person lens. The battle camera must sit AT the reference
/// (scale 1.0); only the scope ladder narrows past it. Speed only ever WIDENS the lens
/// (`SPEED_FOV_BOOST_DEG`, +2.5° at 14 m/s), so the resting 48° is the NARROWEST normal
/// battle view and therefore the one the reference has to cover.
///
/// Rounded UP from cot(24°) = 2.2460368, and the direction is load-bearing: the scale below
/// clamps at 1.0 from beneath, so a reference truncated even slightly under the shipped lens
/// leaves the resting battle view reading as magnified (measured: 1.0000163x) and the meadow
/// silently believing it is looking down a scope.
pub const GRASS_ZOOM_REFERENCE_PROJ_Y: f32 = 2.2461;
/// How far the grass bands may stretch under magnification (Jedna Trawa D3/P4b). The scope
/// ladder reaches ~20× angular magnification at its 3° step; letting the bands follow that
/// literally would ask for grass geometry kilometres out. The cap is a measured compromise,
/// not a taste: see the program's STATUS ledger.
pub const GRASS_ZOOM_BAND_CAP: f32 = 4.0;

/// Recover the projection's Y scale (`P[1][1]` = cot(fov_y / 2)) from a view-projection.
/// The view's rotation part is unit length, so the second column's norm survives the
/// multiply — the same recovery SSAO uses for its world-radius → pixel-radius conversion.
pub fn projection_y_scale(view_proj: &[[f32; 4]; 4]) -> f32 {
    (view_proj[0][1] * view_proj[0][1]
        + view_proj[1][1] * view_proj[1][1]
        + view_proj[2][1] * view_proj[2][1])
        .sqrt()
}

/// How many vehicles can press the meadow down at once (Jedna Trawa P9). Grass only exists
/// within ~47 m of the eye, and the renderer keeps the nearest crushers, so a 7v7's full
/// roster never needs a slot each — what matters is that the ones you can SEE standing in
/// grass are the ones flattening it.
pub const MAX_GRASS_CRUSHERS: usize = 6;
/// The radius a hull presses flat. A medium tank is ~3.3 m across and ~6.5 m long; one
/// circle is the honest approximation at grass scale, sized so the tracks' own width is
/// inside it rather than fringed by standing blades.
pub const GRASS_CRUSH_RADIUS_M: f32 = 3.6;

/// How hard the meadow is pressed at `distance` from a crusher's centre: 1 under the hull,
/// easing to 0 at the radius. The CPU mirror of `meadow_crush` in `meadow_common.wgsl`, so
/// the model is testable without a GPU.
pub fn grass_crush_strength(distance: f32, radius: f32) -> f32 {
    if radius <= 0.0 {
        return 0.0;
    }
    let t = (1.0 - distance / radius).clamp(0.0, 1.0);
    // Squared: the flattening stays near-total under the hull and releases quickly at the
    // rim, instead of leaving a wide skirt of half-bent grass around every tank.
    t * t
}

/// Costume C (Jedna Trawa P5): how much of the meadow's own darkness the GROUND carries.
/// Grass is darker than the soil it grows from — blades shade each other and the gaps hold
/// shadow. Near the eye the ground shows only the shade between standing tufts
/// ([`MEADOW_SHADE_STANDING`]); where the far costume has folded away it is all the meadow
/// there is, so it takes their full share ([`MEADOW_SHADE_COLLAPSED`]). Mirrored in
/// `meadow_common.wgsl`, which both the scene and terrain passes compose.
pub const MEADOW_SHADE_STANDING: f32 = 0.05;
pub const MEADOW_SHADE_COLLAPSED: f32 = 0.17;

/// The ground's albedo multiplier under a meadow. `far_stand` is the far costume's presence
/// (1 = tufts at full height, 0 = folded), `vegetation` the splat's grass+straw share.
pub fn meadow_ground_shade(vegetation: f32, far_stand: f32) -> f32 {
    let carried = MEADOW_SHADE_COLLAPSED
        + (MEADOW_SHADE_STANDING - MEADOW_SHADE_COLLAPSED) * far_stand.clamp(0.0, 1.0);
    1.0 - carried * vegetation.clamp(0.0, 1.0)
}

/// The ground grain's octave periods in metres (1 / the lattice scales in
/// `noise_common.wgsl`: broad 0.4, fine 1.7, micro 3.2 cycles per metre).
pub const GRAIN_BROAD_PERIOD_M: f32 = 2.5;
pub const GRAIN_FINE_PERIOD_M: f32 = 0.588;
pub const GRAIN_MICRO_PERIOD_M: f32 = 0.3125;
/// Footprint filtering (Inny Poziom T3 / O1): a procedural octave has no mip chain, so the
/// terrain pass fades each one by the metres of ground a fragment covers. An octave shows in
/// full while its period spans at least this many pixels...
pub const GRAIN_SHOWN_PX_PER_PERIOD: f32 = 4.0;
/// ...and is gone (folded to its mean) at Nyquist — two pixels per period. Below that a
/// lattice sampled once per fragment beats against the pixel grid: the concentric ripples
/// the owner photographed on flat meadow from a tank's eye.
pub const GRAIN_GONE_PX_PER_PERIOD: f32 = 2.0;

/// How much of an octave of `period_m` a fragment covering `footprint_m` metres of ground
/// may show, 1 (full) to 0 (gone). The CPU mirror of `octave_reach` in `noise_common.wgsl`.
pub fn grain_octave_reach(period_m: f32, footprint_m: f32) -> f32 {
    let shown = period_m / GRAIN_SHOWN_PX_PER_PERIOD;
    let gone = period_m / GRAIN_GONE_PX_PER_PERIOD;
    let t = ((footprint_m - shown) / (gone - shown)).clamp(0.0, 1.0);
    1.0 - t * t * (3.0 - 2.0 * t)
}

/// The metres of FLAT ground one pixel covers along the depth axis, seen from an eye
/// `eye_height_m` above it at ground distance `distance_m` through a lens of `fov_y_rad` over
/// `viewport_h_px` rows. The pixel's angular size times the slant range, divided by the sine
/// of the grazing angle: `a · r² / h`. The shader reads this off `dpdx`/`dpdy` of the world
/// position; this is the model the lock reasons with.
pub fn ground_pixel_footprint_m(
    eye_height_m: f32,
    distance_m: f32,
    fov_y_rad: f32,
    viewport_h_px: f32,
) -> f32 {
    let rad_per_px = fov_y_rad / viewport_h_px;
    let slant_sq = distance_m * distance_m + eye_height_m * eye_height_m;
    rad_per_px * slant_sq / eye_height_m.max(1.0e-3)
}

/// How far a frame's grass bands stretch, from the projection alone (Jedna Trawa P4b).
///
/// The scope is a COMBAT view: at 3.4× a card 100 m out covers the screen area it would at
/// 29 m, so bands measured in world metres collapse the meadow inside the very view the
/// player aims through. Deriving the factor from `P[1][1]` — which the renderer already
/// recovers from the view-projection every frame — means the CPU chunk cutoff and the
/// shader's collapse band read ONE number with no camera state to synchronise, and every
/// off-game camera (probe, garage, review) lands on exactly 1.0.
pub fn grass_zoom_band_scale(proj_y_scale: f32) -> f32 {
    (proj_y_scale / GRASS_ZOOM_REFERENCE_PROJ_Y).clamp(1.0, GRASS_ZOOM_BAND_CAP)
}

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
    /// Screen-door LOD window `[lo, hi)` over the per-pixel Bayer threshold (0..1): the
    /// object keeps the pixels whose threshold falls inside it; `[0, 1]` is solid. The rungs
    /// of one tree partition the unit interval, so every pixel is drawn by exactly one of
    /// them — a cross-fade band between rungs, and the impostor's two crossed quads sharing
    /// one silhouette by view azimuth. The object whose window holds 0.5 casts the shadow.
    pub dither: [f32; 2],
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct RenderFrame {
    pub camera: Camera,
    pub objects: Vec<RenderObject>,
    /// Analytical armor openings keyed by tank id. Empty for scene/garage frames.
    pub armor_damage: Vec<ArmorDamageInstance>,
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
