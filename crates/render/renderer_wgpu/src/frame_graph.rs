//! The identity of every pass a frame can encode.
//!
//! This is the smallest half of the frame graph and the half everything else keys on. A pass has
//! to be nameable before its cost can be attributed, budgeted, or argued about — and until now
//! the only name a pass had was a string literal at its `begin_render_pass` call site, invisible
//! to anything but a GPU debugger.
//!
//! The order is ENCODING order, and it is part of the identity: the profiler's timestamp slots,
//! the per-pass budget table and every recorded measurement are keyed by this enum. Reordering it
//! would silently re-key the whole register — the numbers would still line up, against the wrong
//! passes. `pass_ids_are_append_only` pins the list so that cannot happen quietly.

/// Every render pass `SceneRenderer::render` can encode, in the order it encodes them.
///
/// Not every pass runs every frame: SSAO is skipped at zero strength, bloom at zero mips, and the
/// scene takes EITHER the single `Scene` pass or the `SceneOpaque` + `SceneWater` pair when the
/// refraction grab is active. That is why a profiler cannot assume a fixed set of readings — see
/// `FrameProfiler`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PassId {
    /// Near sun-shadow cascade, depth only.
    ShadowNear,
    /// Far sun-shadow cascade, depth only; the fleet is excluded from it.
    ShadowFar,
    /// Half-resolution depth prepass, encoded only to feed SSAO.
    SsaoPrepass,
    /// Ambient-occlusion evaluation at half resolution.
    Ssao,
    /// The AO blur that makes the half-resolution buffer usable.
    SsaoBlur,
    /// World opaque, when the refraction grab splits the scene in two.
    SceneOpaque,
    /// Water over the grabbed opaque frame, refraction path only.
    SceneWater,
    /// World, water and overlay FX in one pass — the shipped path.
    Scene,
    /// The whole dual-Kawase ladder, down and up, as ONE identity: the mip count is a quality
    /// tier, not a different pass, and timing each rung separately would report a budget nobody
    /// can act on.
    Bloom,
    /// The display transform: exposure, ACES, grade, dither.
    Post,
    /// The shipped game's only anti-aliasing. The HUD currently draws inside it.
    Fxaa,
}

impl PassId {
    /// Every pass, in encoding order. APPEND-ONLY — see the module note.
    pub const ALL: &'static [PassId] = &[
        PassId::ShadowNear,
        PassId::ShadowFar,
        PassId::SsaoPrepass,
        PassId::Ssao,
        PassId::SsaoBlur,
        PassId::SceneOpaque,
        PassId::SceneWater,
        PassId::Scene,
        PassId::Bloom,
        PassId::Post,
        PassId::Fxaa,
    ];

    /// How many passes exist, which is how many timestamp PAIRS a frame can produce.
    pub const COUNT: usize = Self::ALL.len();

    /// The `wgpu` debug label this pass carries. These are the strings a GPU capture shows, so
    /// they are the pass's name everywhere — the enum exists to make them addressable, not to
    /// replace them.
    pub fn label(self) -> &'static str {
        match self {
            PassId::ShadowNear => "shadow_pass",
            PassId::ShadowFar => "shadow_pass_far",
            PassId::SsaoPrepass => "ssao_prepass",
            PassId::Ssao => "ssao_pass",
            PassId::SsaoBlur => "ssao_blur_pass",
            PassId::SceneOpaque => "scene_opaque_pass",
            PassId::SceneWater => "scene_water_pass",
            PassId::Scene => "scene_pass",
            PassId::Bloom => "bloom_pass",
            PassId::Post => "post_pass",
            PassId::Fxaa => "fxaa_pass",
        }
    }

    /// This pass's position in the encoding order, which is also its timestamp slot pair.
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|id| *id == self).expect("every PassId is listed in PassId::ALL")
    }
}
