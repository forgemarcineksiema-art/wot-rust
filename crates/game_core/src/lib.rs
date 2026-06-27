mod armor;
mod crew;
mod damage;
mod damage_layout;
mod ids;
pub mod math;
mod modules;
mod mount;
mod tank;
mod vehicle_blueprint;
mod vehicle_kind;
mod vehicles;
mod weapon;

pub use armor::{
    ArmorFacet, ArmorFacetProfile, ArmorFacing, ArmorProfile, ArmorZone, PenetrationResult,
    resolve_penetration, resolve_penetration_at_distance, resolve_penetration_at_distance_on_zone,
};
pub use crew::{Crew, CrewRole};
pub use damage::{DamageCause, DamageEvent, ImpactSurface, ShellImpact};
pub use damage_layout::{DamageLayout, ModuleVolume};
pub use ids::{TankId, TeamId};
pub use modules::{
    EngineModule, GunModule, HullChassis, MODULE_SLOT_COUNT, ModuleError, ModuleHealth, ModuleSlot,
    RadioModule, SuspensionModule, TurretModule, TurretTraverse, VehicleModules,
};
pub use mount::{MountFrame, MountFrames};
pub use tank::{HitboxProfile, TankSpec};
pub use vehicle_blueprint::{
    ArmorShape, BoxVisual, DetailVisual, FenderVisual, FittingsVisual, GunShape, GunVisual,
    HullPlatesVisual, HullShape, HullVisual, HybridVisual, LoftStation, RunningGearVisual,
    TrackBeltVisual, TrackShape, TurretForm, TurretLoftVisual, TurretShape, TurretVisual,
    VehicleBlueprint,
};
pub use vehicle_kind::{Nation, VehicleKind};
pub use vehicles::known_tank_specs;
pub use weapon::{GunSpec, ShellSpec, ShellType};
