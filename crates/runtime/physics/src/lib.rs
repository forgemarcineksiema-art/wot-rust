mod collision;
mod contact;
mod contact_impulse;
mod controller_settings;
mod cover;
mod engine;
mod forces;
mod ground;
mod hull_attitude;
mod movement;
mod track_contact;
mod vertical;
pub mod water;
mod world;

pub use collision::{
    HeightBand, TankFootprint, TankObstacle, TankWorldObstacles, footprint_penetration_m,
};
pub use contact::{GroundScales, TerrainContact, sample_tank_terrain_contact};
pub use contact_impulse::{
    ContactBody, ContactCache, ContactImpact, ContactImpulse, ContactPair, ContactReport,
    SOLID_ID_BASE, resolve_contacts, solid_bodies_near,
};
pub use controller_settings::hull_spring_for_spec;
pub use controller_settings::{BeltDrive, TankControllerSettings};
pub use cover::{
    footprint_blocked_by_cover, footprint_overlaps_cover_object, resolve_cover_collision,
    resolve_cover_collision_with_velocity,
};
pub use engine::{EngineState, IDLE_RPM_NORM, SHIFT_UP_RPM_NORM, engine_state, engine_thrust_mps2};
pub use ground::{
    GroundLayers, MAX_STEP_SOLIDS, StepSolid, StepSolids, is_step_for, step_solids_near,
};
pub use hull_attitude::{
    ATTITUDE_REST_EPSILON, HullSpring, MAX_HULL_TILT_RAD, MAX_WEIGHT_TRANSFER_RAD,
    advance_hull_attitude,
};
pub use movement::{
    TankControlInput, TankKinematicState, advance_hull_drive, integrate_hull_position,
    step_custom_tank_controller, step_custom_tank_controller_on_contact,
};
pub use track_contact::{
    StationGround, SupportContact, sample_support, station_ground, support_height,
};
pub use vertical::{GroundStep, is_grounded, resolve_vertical};
pub use world::{
    MAP_BORDER_MARGIN_M, TankStepContact, advance_tank_on_world, clamp_to_map_border,
    settle_tank_on_world, step_tank_on_heightmap, step_tank_on_world,
    step_tank_on_world_with_tanks,
};

// Rapier is gone (2026-08-02, audit finding D6) and parry3d followed it (2026-09-07, the one
// program's X9): the `RapierWorld`, its collider constructors and the `PhysicsOwnershipPolicy`
// were a parallel fiction consumed only by their own tests, and the parry footprint query that
// outlived them had zero production callers. Every real path — SAT footprints, heightmap
// stepping, the support envelope, the contact solver — is custom deterministic code.
