//! The vehicle's specular and environment lobes, mirrored on the CPU (the one program's D39).
//!
//! `vehicle.wgsl` shades a hull with a role roughness wandered by the synthesis' roughness
//! lane, a Blinn lobe whose amplitude falls with the cube of smoothness, and an analytic-sky
//! environment term that scales with smoothness squared and a metalness-driven Fresnel. Until
//! D39 the lane MULTIPLIED the role (`role × (0.55 + G)`) and saturated cast armour, track
//! metal and rubber at roughness 1.0 — three of four exterior roles with no highlight and no
//! sky at all, which is most of why the hull did not read as tonnes of steel. Now the lane
//! ADDS (`role + (G − 0.5) · span`) to an ordered ladder of role roughnesses, so every
//! exterior role keeps a lobe of its own, in a fixed order. This module is the formula the
//! shader must agree with (`renderer_wgpu` locks the constants against the source).

/// The exterior material roles, in the shader's id order where they have one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExteriorRole {
    RolledArmor,
    CastArmor,
    BarrelSteel,
    TrackMetal,
    Rubber,
    Canvas,
    Glass,
    Timber,
}

impl ExteriorRole {
    pub const ALL: [ExteriorRole; 8] = [
        ExteriorRole::RolledArmor,
        ExteriorRole::CastArmor,
        ExteriorRole::BarrelSteel,
        ExteriorRole::TrackMetal,
        ExteriorRole::Rubber,
        ExteriorRole::Canvas,
        ExteriorRole::Glass,
        ExteriorRole::Timber,
    ];

    /// The role's roughness at the lane's midpoint — the shader's `material_params`.
    pub fn roughness(self) -> f32 {
        match self {
            ExteriorRole::Glass => 0.10,
            ExteriorRole::BarrelSteel => 0.45,
            ExteriorRole::TrackMetal => 0.50,
            ExteriorRole::RolledArmor => 0.55,
            ExteriorRole::CastArmor => 0.62,
            ExteriorRole::Rubber => 0.80,
            ExteriorRole::Timber => 0.88,
            ExteriorRole::Canvas => 0.95,
        }
    }

    /// The metalness the synthesis writes into the role's texture (its blue lane), 0..=1 —
    /// `material_synthesis.rs`: track links 128/255, glass a trace, everything else paint.
    pub fn metalness(self) -> f32 {
        match self {
            ExteriorRole::TrackMetal => 128.0 / 255.0,
            ExteriorRole::Glass => 24.0 / 255.0,
            ExteriorRole::RolledArmor => 10.0 / 255.0,
            ExteriorRole::CastArmor => 8.0 / 255.0,
            ExteriorRole::BarrelSteel => 12.0 / 255.0,
            ExteriorRole::Rubber | ExteriorRole::Canvas | ExteriorRole::Timber => 0.0,
        }
    }
}

/// How far the roughness lane (G, 0.5 = the finish) wanders a role's roughness, added.
pub const ROUGHNESS_LANE_SPAN: f32 = 0.30;

/// The shader's surface roughness before wetness and wounds: the role plus the lane's
/// wander, plus the grain and the dust film, clamped as the shader clamps.
pub fn surface_roughness(role: ExteriorRole, lane_g: f32, grain: f32, dust: f32) -> f32 {
    (role.roughness() + (lane_g - 0.5) * ROUGHNESS_LANE_SPAN + (grain - 0.5) * 0.20 + dust * 0.22)
        .clamp(0.04, 1.0)
}

/// The Blinn lobe's amplitude at the highlight's centre: `(1 − r)³ · 0.6`.
pub fn specular_amplitude(roughness: f32) -> f32 {
    (1.0 - roughness).powi(3) * 0.6
}

/// The environment term's energy at normal incidence: `smoothness² · F0`, with F0 riding the
/// metalness lane from paint (0.04) to bare steel (0.32).
pub fn environment_energy(roughness: f32, metalness: f32) -> f32 {
    let smoothness = 1.0 - roughness;
    let f0 = 0.04 + (0.32 - 0.04) * metalness;
    smoothness * smoothness * f0
}
