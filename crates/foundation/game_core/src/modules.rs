//! Swappable vehicle modules (WoT-style): a tank is a [`HullChassis`] plus five module
//! slots (engine, suspension, turret, gun, radio). [`VehicleModules::assemble`] composes the
//! installed loadout into the flat [`crate::TankSpec`] the rest of the game consumes, so
//! modules are the single source of truth for a vehicle's stats.

mod catalog;
mod catalog_german;
mod catalog_misc;
mod catalog_soviet;
mod engine;
mod gun;
mod health;
mod hull;
mod loadout;
mod module_slot;
mod radio;
mod suspension;
mod turret;

pub use engine::EngineModule;
pub use gun::GunModule;
pub use health::{ModuleHealth, engine_power_fraction, suspension_agility_fraction};
pub use hull::HullChassis;
pub use loadout::{ModuleError, VehicleModules};
pub use module_slot::{MODULE_SLOT_COUNT, ModuleSlot};
pub use radio::RadioModule;
pub use suspension::SuspensionModule;
pub use turret::{TurretModule, TurretTraverse};
