mod ammo;
mod armor;
mod contact_footprint;
mod crew;
mod damage;
mod damage_layout;
mod ids;
pub mod math;
mod modules;
mod mount;
mod tank;
mod track;
mod vehicle_blueprint;
mod vehicle_kind;
mod vehicles;
mod weapon;
mod weather;

pub use ammo::{AmmoLoadout, MAX_AMMO_SLOTS, default_ammo_capacity};
pub use armor::{
    ArmorFacet, ArmorFacetProfile, ArmorFacing, ArmorPatch, ArmorProfile, ArmorVolume, ArmorZone,
    PenetrationResult, TaggedPlane, VehicleArmorVolumes, resolve_penetration,
    resolve_penetration_at_distance, resolve_penetration_at_distance_on_zone,
    resolve_penetration_through_track, segment_volume_entry, segment_volume_entry_with_margin,
    vehicle_armor_volumes,
};
pub use contact_footprint::{ContactFootprint, MAX_CONTACT_STATIONS};
pub use crew::{Crew, CrewRole};
pub use damage::{DamageCause, DamageEvent, ImpactSurface, ShellImpact, TrackHit};
pub use damage_layout::{DamageLayout, ModuleVolume};
pub use ids::{ShellId, TankId, TeamId};
pub use modules::{
    EngineModule, GunModule, HullChassis, MODULE_SLOT_COUNT, ModuleCondition, ModuleError,
    ModuleHealth, ModuleSlot, RadioModule, SuspensionModule, TurretModule, TurretTraverse,
    VehicleModules, engine_power_fraction, gun_reload_multiplier, module_condition,
    suspension_agility_fraction,
};
pub use mount::{MountFrame, MountFrames};
pub use tank::{HitboxProfile, TankSpec};
pub use track::{
    TRACK_HP_MAX, TrackDamageMask, TrackHealth, TrackSeverity, TrackSide, track_hit_damage,
    track_traction_fraction,
};
pub use vehicle_blueprint::{
    ArmorShape, BlueprintFile, BoxVisual, DetailVisual, FenderVisual, FittingsVisual, GunShape,
    GunVisual, HullPlatesVisual, HullShape, HullVisual, HybridVisual, LoftStation, SkirtShape,
    TrackShape, TurretForm, TurretLoftVisual, TurretShape, TurretVisual, VehicleBlueprint, lint,
    parse_blueprint,
};
pub use vehicle_kind::{Era, Nation, VehicleKind};
pub use vehicles::known_tank_specs;
pub use weapon::{GunSpec, ShellSpec, ShellType};
pub use weather::WeatherVariant;
