use glam::{Mat4, Vec3};

use crate::Camera;

/// Backend-neutral water-surface vertex: a world-space point ON the still-water plane plus the
/// water depth under it (`surface level − riverbed height`, baked from the heightmap). Depth
/// drives the shore fade and the shallow→deep tint in the water shader without any per-pixel
/// depth-texture read; the animated ripple is purely a shader-side normal perturbation, so the
/// mesh itself stays static (uploaded once per scene swap). POD for zero-copy upload.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct WaterVertex {
    pub position: [f32; 3],
    pub depth_m: f32,
}

impl WaterVertex {
    pub const fn new(position: [f32; 3], depth_m: f32) -> Self {
        Self { position, depth_m }
    }
}

/// Backend-neutral lit vertex: world-space position, normal, an RGB base color, and a tint weight.
///
/// `tint_weight` controls how much of the per-instance team tint multiplies the base color:
/// `0.0` keeps the color absolute (terrain, barrels, tracks, rubber), `1.0` fully tints it (hull
/// and turret armor). This lets one team-neutral mesh be drawn in any team color from instance
/// data, instead of baking a separate mesh per color. POD so backends can upload it zero-copy.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SceneVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 3],
    pub tint_weight: f32,
    /// The material lane (materials v2): 0.0 is fully matte (the historical look), 1.0 a
    /// polished surface. One number drives the scene shader's whole specular response —
    /// strength, sharpness, and the analytic-sky reflection weight — so grass, plaster,
    /// slate and steel finally answer light differently.
    pub gloss: f32,
}

impl SceneVertex {
    /// An absolute-colored vertex (`tint_weight` 0.0): the per-instance tint never touches it.
    /// Matte by default — surfaces with a real finish use [`Self::surfaced`].
    pub const fn new(position: [f32; 3], normal: [f32; 3], color: [f32; 3]) -> Self {
        Self { position, normal, color, tint_weight: 0.0, gloss: 0.0 }
    }

    /// A vertex with a material finish (see `gloss`).
    pub const fn surfaced(
        position: [f32; 3],
        normal: [f32; 3],
        color: [f32; 3],
        gloss: f32,
    ) -> Self {
        Self { position, normal, color, tint_weight: 0.0, gloss }
    }

    /// A vertex that opts into the per-instance team tint by `tint_weight` (`1.0` = fully tinted).
    pub const fn tinted(
        position: [f32; 3],
        normal: [f32; 3],
        color: [f32; 3],
        tint_weight: f32,
    ) -> Self {
        Self { position, normal, color, tint_weight, gloss: 0.0 }
    }
}

/// A world-space FX vertex: unlit, alpha-blended battle effects (muzzle flash, smoke, dirt,
/// sparks, tracers) drawn after the lit scene with depth *test* but no depth write.
///
/// `color` is **premultiplied** RGBA, which lets one pipeline cover the whole range of combat
/// effects: `alpha = 0` with non-zero RGB blends purely additively (flash, tracer glow), full
/// premultiplied color blends as ordinary transparency (smoke, dust), and anything in between
/// mixes the two. `uv` spans `[-1, 1]` across the quad; the FX shader fades the fragment by
/// `1 - dot(uv, uv)` so every particle is born soft-edged instead of a hard billboard rectangle.
/// POD so backends can upload it zero-copy.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct FxVertex {
    pub position: [f32; 3],
    pub uv: [f32; 2],
    /// Edge sharpness: 1.0 is the soft gaussian-ish falloff every particle uses; larger values
    /// steepen the radial edge toward a hard-edged disc (decal holes, gouges) — one pipeline
    /// covers glow and stamped marks alike.
    pub sharpness: f32,
    pub color: [f32; 4],
}

impl FxVertex {
    /// A soft quad (sharpness 1.0) — the default for every particle and tracer.
    pub const fn new(position: [f32; 3], uv: [f32; 2], color: [f32; 4]) -> Self {
        Self { position, uv, sharpness: 1.0, color }
    }

    /// A quad with an explicit edge sharpness (decal stamps).
    pub const fn sharp(position: [f32; 3], uv: [f32; 2], sharpness: f32, color: [f32; 4]) -> Self {
        Self { position, uv, sharpness, color }
    }
}

/// A 2D overlay vertex in clip space (NDC), with atlas UVs and a straight RGBA color. Used for
/// the HUD (crosshair, health/reload bars, text) drawn on top of the scene in one pass.
///
/// `uv` addresses the font/coverage atlas. A negative `uv.x` is a sentinel for "solid": the HUD
/// shader skips the texture and treats coverage as fully opaque, so bars and crosshairs keep
/// rendering as flat colored quads while glyph quads sample the atlas — all from one buffer.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct HudVertex {
    pub position: [f32; 2],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

/// `uv` sentinel marking a solid (non-textured) HUD vertex. Any negative `uv.x` qualifies; this is
/// the canonical value `HudVertex::new` stamps in.
pub const HUD_SOLID_UV: [f32; 2] = [-1.0, -1.0];

impl HudVertex {
    /// A solid colored vertex: the HUD shader fills it at full coverage, ignoring the atlas.
    pub const fn new(position: [f32; 2], color: [f32; 4]) -> Self {
        Self { position, uv: HUD_SOLID_UV, color }
    }

    /// A textured vertex sampling the font/coverage atlas at `uv`; `color` tints the sampled
    /// coverage (alpha = color.a * coverage).
    pub const fn textured(position: [f32; 2], uv: [f32; 2], color: [f32; 4]) -> Self {
        Self { position, uv, color }
    }
}

/// World -> clip matrix using this project's WebGPU depth convention: `perspective_rh`
/// maps the depth range to `[0, 1]` (not OpenGL's `[-1, 1]`), paired with a
/// right-handed look-at. The result is column-major, matching a WGSL `mat4x4<f32>`.
pub fn view_projection_matrix(camera: &Camera, aspect: f32, near: f32, far: f32) -> [[f32; 4]; 4] {
    let proj =
        Mat4::perspective_rh(camera.vertical_fov_degrees.to_radians(), aspect.max(0.01), near, far);
    let view =
        Mat4::look_at_rh(Vec3::from_array(camera.eye), Vec3::from_array(camera.target), Vec3::Y);
    (proj * view).to_cols_array_2d()
}

/// Inverse of a column-major world -> clip matrix, so a shader can unproject a clip/NDC point back
/// to world space (the gradient-sky pass reconstructs a per-pixel view ray direction from it). A
/// singular matrix inverts to all-zeros in glam, which the sky shader tolerates (degenerate ray).
pub fn view_projection_inverse(view_proj: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    Mat4::from_cols_array_2d(&view_proj).inverse().to_cols_array_2d()
}
