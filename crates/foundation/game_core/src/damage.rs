use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::{ArmorFacing, ArmorZone, ModuleSlot, ShellType, TankId};

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
}
