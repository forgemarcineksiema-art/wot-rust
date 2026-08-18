mod ammo;
mod ammo_catalog;
mod armor;
mod armor_breach;
mod contact_footprint;
mod crew;
mod crew_vitals;
mod damage;
mod damage_layout;
mod hull_plan;
mod ids;
pub mod math;
pub mod mobility;
mod modules;
mod mount;
pub mod roundness;
pub mod stability;
mod steering;
mod tank;
mod track;
mod vehicle_blueprint;
mod vehicle_kind;
mod vehicles;
mod weapon;
mod weather;

pub use ammo::{AmmoLoadout, MAX_AMMO_SLOTS, default_ammo_capacity};
pub use ammo_catalog::RoundId;
pub use armor::{
    ArmorFacet, ArmorFacetProfile, ArmorFacing, ArmorPatch, ArmorProfile, ArmorVolume, ArmorZone,
    PenetrationResult, TaggedPlane, VehicleArmorVolumes, VolumeInterval, WeakspotFrame,
    WeakspotPoint, resolve_penetration, resolve_penetration_at_distance,
    resolve_penetration_at_distance_on_zone, resolve_penetration_at_distance_on_zone_scaled,
    resolve_penetration_through_open_channel, resolve_penetration_through_screens,
    resolve_penetration_through_track, segment_volume_entry, segment_volume_entry_with_margin,
    segment_volume_interval_with_margin, vehicle_armor_volumes,
};
pub use armor_breach::surface_basis as armor_surface_basis;
pub use armor_breach::{
    ApertureLobe, ArmorBreach, ArmorBreachAdd, ArmorBreachDescriptor, ArmorBreachSet, ArmorFrame,
    ArmorMaterial, ArmorScar, ArmorSurfaceId, BreachContour, BreachFace, MAX_APERTURE_LOBES,
    MAX_ARMOR_BREACHES, MAX_ARMOR_SCARS, MAX_BREACH_FRAGMENTS_PER_GROUP,
};
pub use contact_footprint::{ContactFootprint, MAX_CONTACT_STATIONS};
pub use crew::{Crew, CrewRole};
pub use crew_vitals::{
    CREW_COVERED_EFFECTIVENESS, CREW_FIRST_AID_S, CREW_ROLE_COUNT, CREW_WEAKENED_EFFECTIVENESS,
    CrewMemberState, CrewVitals, crew_time_multiplier,
};
pub use damage::{DamageCause, DamageEvent, ImpactSurface, ShellImpact, ShotFired, TrackHit};
pub use damage_layout::authoring::HullEnvelope;
pub use damage_layout::{
    DamageComponent, DamageComponentId, DamageComponentKind, DamageLayout, DamageMaterial,
    DamagePlane, DamageShape, ModuleIntersection,
};
pub use hull_plan::HullPlan;
pub use ids::{BattleEventId, ShellId, TankId, TeamId};
pub use mobility::{MAX_CLIMB_GRADE, ROAD_COMFORT_GRADE};
pub use modules::{
    EngineModule, GunModule, HullChassis, MODULE_SLOT_COUNT, ModuleCondition, ModuleError,
    ModuleHealth, ModuleSlot, RadioModule, SuspensionModule, TurretModule, TurretTraverse,
    VehicleModules, engine_power_fraction, gun_reload_multiplier, module_condition,
    suspension_agility_fraction,
};
pub use mount::{MountFrame, MountFrames};
pub use stability::{Stability, stability, stock_stability};
pub use steering::SteeringKind;
pub use tank::{HitboxProfile, TankSpec};
pub use track::{
    TRACK_HP_MAX, TrackDamageMask, TrackHealth, TrackSeverity, TrackSide, track_hit_damage,
    track_traction_fraction,
};
pub use vehicle_blueprint::{
    ArmorShape, BlueprintFile, BoxVisual, CanvasCoverVisual, CompleteVisual, DetailVisual,
    FenderVisual, FittingsVisual, GlacisPort, GunShape, GunVisual, HullPlatesVisual, HullShape,
    HullVisual, LoftStation, MuzzleBrakeVisual, ShoePattern, SkirtShape, SuspensionKind,
    TrackShape, TurretForm, TurretLoftVisual, TurretShape, TurretVisual, VehicleBlueprint,
    VisualDetail, VisualDetailFile, WheelFace, lint, parse_blueprint, parse_visual_detail,
};
pub use vehicle_kind::{Era, Nation, VehicleKind};
pub use vehicles::known_tank_specs;
pub use weapon::{GunSpec, ShellSpec, ShellType};
pub use weather::{MatchWeather, WeatherVariant};
