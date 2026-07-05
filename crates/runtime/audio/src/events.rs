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
    /// A shell died against the world instead of a target.
    ShellAbsorbed { position: Vec3, surface: GroundKind },
    /// The player's reload completed (breech clack at the ear).
    GunReady,
    /// The player's target was destroyed (confirmation beat).
    KillConfirmed,
    /// A garage/UI interaction; `accent` marks commits over browsing clicks.
    UiClick { accent: bool },
}
