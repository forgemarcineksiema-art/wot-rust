//! The game-to-audio vocabulary: everything gameplay reports to the soundscape. One enum, plain
//! data, world-space meters — the mixer picks voices and physics from it (see [`crate::mixer`]).

use glam::Vec3;

use crate::voices::impact::GroundKind;

/// Everything the game reports to the soundscape. Positions are world-space meters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AudioEvent {
    /// A gun fired. `own_shot` marks the player's gun: full presence, no travel delay.
    CannonFired { position: Vec3, caliber_mm: f32, own_shot: bool },
    /// A shell struck a vehicle's armor. `high_explosive` swaps the kinetic plate clang for the
    /// charge's own burst (HE direct hits and splash strikes alike).
    ArmorStruck { position: Vec3, penetrated: bool, ricocheted: bool, high_explosive: bool },
    /// A shell died against the world instead of a target. A high-explosive round detonates
    /// (the burst voice) where kinetic rounds thud into the surface.
    ShellAbsorbed { position: Vec3, surface: GroundKind, high_explosive: bool },
    /// The player's reload completed (breech clack at the ear).
    GunReady,
    /// A track band was damaged or thrown. `broken` picks the voice: a sharp metallic snap for a
    /// thrown track, a duller grind for a mere degrade — distinct from the plate clang, and heard
    /// even on a clean 0-HP track break.
    TrackSnapped { position: Vec3, broken: bool },
    /// A supersonic shell passed close to the ear without hitting (D8): the N-wave crack at
    /// its closest-approach point. `miss_distance_m` scales how loud death sounded going by.
    ShellFlyby { position: Vec3, caliber_mm: f32, miss_distance_m: f32 },
    /// The player's target was destroyed (confirmation beat).
    KillConfirmed,
    /// A garage/UI interaction; `accent` marks commits over browsing clicks.
    UiClick { accent: bool },
    /// The garage REFUSED an edit (incompatible fit): the dull knock that answers the red flash,
    /// so a rejection is heard, not only seen.
    UiReject,
    /// The garage repair beat (Hala v4 R2): the wrench works for `seconds` — the shop is
    /// HEARD fixing what the nameplate says it is fixing.
    RepairWork { seconds: f32 },
}
