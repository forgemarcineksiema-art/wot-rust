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

/// A resource one pass produces and another consumes, within a single frame.
///
/// Only PER-FRAME resources are here. The foliage atlas, the baked cloud tile and the material
/// textures are inputs to the frame rather than products of it, so they carry no ordering
/// constraint and would only make the graph look busier than the frame is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FrameResource {
    /// The focused sun-shadow depth map.
    ShadowNear,
    /// The wide cascade's depth map, at half the resolution and without the fleet.
    ShadowFar,
    /// Half-resolution camera depth, produced solely to feed the AO evaluation.
    SsaoDepth,
    /// Raw ambient occlusion, before the blur that makes a half-resolution buffer usable.
    SsaoRaw,
    /// The AO the world shader actually samples.
    SsaoBlurred,
    /// The scene's depth buffer.
    SceneDepth,
    /// The multisampled HDR attachment the world renders into.
    HdrColor,
    /// The resolved opaque frame the water samples for refraction — the two-pass path only.
    HdrGrab,
    /// The resolved HDR frame the bloom ladder and the display transform read.
    HdrResolved,
    /// The blurred bloom mip chain.
    BloomChain,
    /// The display-transformed picture, encoded, before anti-aliasing.
    LdrFormed,
    /// The caller's target — the picture that leaves this renderer.
    Output,
}

/// What has to be true for a pass to be encoded at all.
///
/// Three switches, and every one of them is read from renderer state inside `render`. They are
/// named here so the graph can be checked for every combination rather than for the one the
/// machine that runs the test happens to be in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PassCondition {
    /// Encoded on every frame.
    Always,
    /// `ssao.strength > 0.0`.
    Ssao,
    /// `scene_lighting.bloom_weight > 0.0`.
    Bloom,
    /// `refraction && sample_count > 1` — the two-pass water grab.
    Refraction,
    /// The single-pass world, taken when the grab is not.
    NoRefraction,
}

impl PassCondition {
    /// Whether this condition holds for a given combination of the three switches.
    pub fn holds(self, ssao: bool, bloom: bool, refraction: bool) -> bool {
        match self {
            PassCondition::Always => true,
            PassCondition::Ssao => ssao,
            PassCondition::Bloom => bloom,
            PassCondition::Refraction => refraction,
            PassCondition::NoRefraction => !refraction,
        }
    }
}

/// One pass, and what it needs and leaves behind.
#[derive(Debug, Clone, Copy)]
pub struct PassNode {
    pub id: PassId,
    pub condition: PassCondition,
    /// Resources this pass cannot run without.
    pub reads: &'static [FrameResource],
    /// Resources it samples WHEN THEY EXIST, with a placeholder bound otherwise — the 1x1 open-AO
    /// view and the 1x1 black bloom view. Modelling these as required reads would make the graph
    /// refuse frames the renderer draws correctly every day.
    pub optional_reads: &'static [FrameResource],
    pub writes: &'static [FrameResource],
}

/// The frame, as it is encoded today.
///
/// Derived from the call sites rather than from intent: a pass reads what the bind groups it sets
/// actually carry. That is why the water pass reads the grab and nothing else — `draw_water_surface`
/// and `draw_overlay_fx` set group 0, and the refraction grab at group 1, but never group 2, so
/// neither the water nor the FX sample the shadow maps or the AO. Only the opaque world does.
///
/// This table changes in the same commit as the code it describes; its diff is the review.
pub const FRAME_GRAPH: &[PassNode] = &[
    PassNode {
        id: PassId::ShadowNear,
        condition: PassCondition::Always,
        reads: &[],
        optional_reads: &[],
        writes: &[FrameResource::ShadowNear],
    },
    PassNode {
        id: PassId::ShadowFar,
        condition: PassCondition::Always,
        reads: &[],
        optional_reads: &[],
        writes: &[FrameResource::ShadowFar],
    },
    PassNode {
        id: PassId::SsaoPrepass,
        condition: PassCondition::Ssao,
        reads: &[],
        optional_reads: &[],
        writes: &[FrameResource::SsaoDepth],
    },
    PassNode {
        id: PassId::Ssao,
        condition: PassCondition::Ssao,
        reads: &[FrameResource::SsaoDepth],
        optional_reads: &[],
        writes: &[FrameResource::SsaoRaw],
    },
    PassNode {
        id: PassId::SsaoBlur,
        condition: PassCondition::Ssao,
        reads: &[FrameResource::SsaoDepth, FrameResource::SsaoRaw],
        optional_reads: &[],
        writes: &[FrameResource::SsaoBlurred],
    },
    PassNode {
        id: PassId::SceneOpaque,
        condition: PassCondition::Refraction,
        reads: &[FrameResource::ShadowNear, FrameResource::ShadowFar],
        optional_reads: &[FrameResource::SsaoBlurred],
        writes: &[FrameResource::SceneDepth, FrameResource::HdrColor, FrameResource::HdrGrab],
    },
    PassNode {
        id: PassId::SceneWater,
        condition: PassCondition::Refraction,
        reads: &[FrameResource::HdrGrab, FrameResource::SceneDepth, FrameResource::HdrColor],
        optional_reads: &[],
        writes: &[FrameResource::HdrColor, FrameResource::HdrResolved],
    },
    PassNode {
        id: PassId::Scene,
        condition: PassCondition::NoRefraction,
        reads: &[FrameResource::ShadowNear, FrameResource::ShadowFar],
        optional_reads: &[FrameResource::SsaoBlurred],
        writes: &[FrameResource::SceneDepth, FrameResource::HdrColor, FrameResource::HdrResolved],
    },
    PassNode {
        id: PassId::Bloom,
        condition: PassCondition::Bloom,
        reads: &[FrameResource::HdrResolved],
        optional_reads: &[],
        writes: &[FrameResource::BloomChain],
    },
    PassNode {
        id: PassId::Post,
        condition: PassCondition::Always,
        reads: &[FrameResource::HdrResolved],
        optional_reads: &[FrameResource::BloomChain],
        writes: &[FrameResource::LdrFormed],
    },
    PassNode {
        id: PassId::Fxaa,
        condition: PassCondition::Always,
        reads: &[FrameResource::LdrFormed],
        optional_reads: &[],
        writes: &[FrameResource::Output],
    },
];
