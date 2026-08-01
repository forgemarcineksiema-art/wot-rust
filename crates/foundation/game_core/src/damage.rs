use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::{
    ArmorFacing, ArmorZone, BattleEventId, ModuleSlot, ShellId, ShellType, TankId, TrackSide,
};

/// Like [`crate::VehicleKind`], the variant order is wire identity (bincode discriminants) —
/// append, never reorder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DamageCause {
    #[default]
    Shell,
    Ram,
    /// The hull slammed into the ground after a flight (fall damage).
    Impact,
    /// Blast damage from a high-explosive burst nearby — not the shell body itself.
    Splash,
    /// The hull sank past its fording depth: the engine flooded first, then the crew lost the
    /// vehicle (protocol v18). Always self-inflicted — the river is not a combatant.
    Drowning,
}

impl DamageCause {
    /// How a hull lost health. The battle report, the kill feed and the replay all read this, so a
    /// cause that exists in the game and not in `ALL` is a cause no summary can count.
    ///
    /// Locked variant-by-variant against the declaration by `quality`, not by counting: a
    /// length assertion cannot tell a forgotten variant from a shorter enum.
    pub const ALL: [DamageCause; 5] = [
        DamageCause::Shell,
        DamageCause::Ram,
        DamageCause::Impact,
        DamageCause::Splash,
        DamageCause::Drowning,
    ];
}

/// What absorbed a shell that did **not** damage an enemy. Like [`crate::VehicleKind`], the
/// variant order is wire identity (bincode discriminants) — append, never reorder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ImpactSurface {
    #[default]
    Terrain,
    Cover,
    /// A hull that blocks without taking damage: a wreck or a friendly vehicle.
    Hull,
    /// Open water swallowed the shell (protocol v18): the splash is the whole story — no
    /// crater, no ricochet, just a column marking where the shot died.
    Water,
}

/// A shell ending its flight without damaging an enemy: eaten by terrain, static cover, a
/// wreck, or a friendly hull. Replicated so the firing client can show *where* the shot died
/// instead of the shell silently vanishing between snapshots.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct ShellImpact {
    /// Who fired the absorbed shell.
    pub owner: TankId,
    pub position: Vec3,
    pub surface: ImpactSurface,
    /// What kind of shell died here (protocol v17): a high-explosive round detonating against
    /// the world is a blast, not a thud — presentation needs to know. Appended last for wire
    /// layout stability; `serde(default)` keeps older fixtures deserializing in tests.
    #[serde(default)]
    pub shell_type: ShellType,
    /// The shell's flight direction at death, normalized (protocol v30, Fizyczny Świat P1):
    /// a full-calibre round hitting soil ploughs a FURROW along its track and throws earth
    /// FORWARD — a directionless mark is physically wrong. `Vec3::ZERO` (old snapshots via
    /// `serde(default)`) degrades the presentation to the directionless mark.
    #[serde(default)]
    pub direction: Vec3,
    /// The shell's calibre (protocol v30): the mark's SIZE is the projectile's size — a
    /// 122 mm crater is not an 76 mm one. `0.0` (old snapshots) falls back to a default.
    #[serde(default)]
    pub caliber_mm: f32,
    /// Stable identity in the authoritative battle-event stream. Zero is an unstamped legacy or
    /// presentation-only event.
    #[serde(default)]
    pub event_id: BattleEventId,
    /// Simulation tick at which the impact occurred.
    #[serde(default)]
    pub occurred_tick: u64,
    /// Stable identity of the projectile whose flight ended here.
    #[serde(default)]
    pub shell_id: ShellId,
}

/// What a shell did to a track band (protocol v22): which side it struck and whether that hit
/// drained the pool to zero (a fresh throw). Presentation-only — the client turns it into the
/// track callout, the damage-log row and the snap/grind cue. `None` on the event means the shell
/// did not touch a track.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackHit {
    pub side: TrackSide,
    /// This hit took the side's pool to zero — a newly thrown track, not just further damage.
    pub broke: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct DamageEvent {
    pub source: TankId,
    pub target: TankId,
    pub hit_position: Vec3,
    pub damage_hp: u32,
    pub penetrated: bool,
    #[serde(default)]
    pub cause: DamageCause,
    #[serde(default)]
    pub module: Option<ModuleSlot>,
    #[serde(default)]
    pub ricocheted: bool,
    #[serde(default)]
    pub shell_type: ShellType,
    #[serde(default)]
    pub impact_angle_degrees: f32,
    #[serde(default)]
    pub effective_armor_mm: f32,
    #[serde(default)]
    pub shell_penetration_mm: f32,
    #[serde(default)]
    pub nominal_armor_mm: f32,
    #[serde(default)]
    pub armor_facing: ArmorFacing,
    #[serde(default)]
    pub armor_zone: ArmorZone,
    /// World-space outward normal of the struck plate (protocol v19). The server already knows
    /// the true sloped/curved plate normal from the armor-volume trace; transmitting it lets the
    /// client seat the impact mark flush on the visual armor instead of guessing a cardinal
    /// facing normal. Zero for causes with no struck plate (Splash/Ram/Impact/Drowning) — the
    /// client reads a zero vector as "no normal, fall back". `serde(default)` keeps pre-v19
    /// fixtures loading.
    #[serde(default)]
    pub plate_normal: Vec3,
    /// Normalized shell travel direction at impact (protocol v19): the client raycasts this line
    /// against the visual mesh to find the exact surface point, and elongates ricochet gouges
    /// along the real scrape direction. Zero when there is no shell (Ram/Impact/Drowning).
    #[serde(default)]
    pub shell_direction: Vec3,
    /// The track band this shell struck, if any (protocol v22). Drives the client's track
    /// feedback (callout / log / audio); never fed into the deterministic drive. `serde(default)`
    /// = `None` keeps pre-v22 fixtures and every `..Default::default()` literal loading.
    #[serde(default)]
    pub track_hit: Option<TrackHit>,
    /// Every module touched by this shell's internal path (protocol v25).
    #[serde(default)]
    pub damaged_modules_mask: u8,
    /// Subset of `damaged_modules_mask` that crossed to zero on this impact (protocol v25).
    #[serde(default)]
    pub destroyed_modules_mask: u8,
    /// Stable physical components touched by the direct path or its deterministic spall cone
    /// (protocol v26). Bit `n` represents `DamageComponentId(n + 1)`; legacy ModuleSlot masks
    /// remain the HUD/repair compatibility layer.
    #[serde(default)]
    pub damaged_components_mask: u32,
    /// Stable identity in the authoritative battle-event stream. Zero is an unstamped legacy or
    /// presentation-only event.
    #[serde(default)]
    pub event_id: BattleEventId,
    /// Simulation tick at which this damage resolved.
    #[serde(default)]
    pub occurred_tick: u64,
    /// Projectile attribution for direct and blast damage; absent for ram, fall and drowning.
    #[serde(default)]
    pub shell_id: Option<ShellId>,
    /// True only on the event that transitioned this target from alive to dead.
    #[serde(default)]
    pub target_destroyed: bool,
}
