use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::{ArmorFacing, ArmorZone, ModuleSlot, ShellType, TankId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DamageCause {
    #[default]
    Shell,
    Ram,
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
