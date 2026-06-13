mod armor;
mod crew;
mod damage;
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
pub use ids::{TankId, TeamId};
pub use modules::{
    EngineModule, GunModule, HullChassis, MODULE_SLOT_COUNT, ModuleError, ModuleHealth, ModuleSlot,
    RadioModule, SuspensionModule, TurretModule, TurretTraverse, VehicleModules,
};
pub use mount::{MountFrame, MountFrames};
pub use tank::{HitboxProfile, TankSpec};
pub use vehicle_blueprint::{
    ArmorShape, GunShape, HullShape, TrackShape, TurretForm, TurretShape, VehicleBlueprint,
};
pub use vehicle_kind::VehicleKind;
pub use vehicles::known_tank_specs;
pub use weapon::{GunSpec, ShellSpec, ShellType};
