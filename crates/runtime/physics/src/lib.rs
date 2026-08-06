mod collision;
mod contact;
mod contact_impulse;
mod controller_settings;
mod cover;
mod forces;
mod hull_attitude;
mod movement;
mod parry_query;
mod tank_resolve;
mod track_contact;
mod vertical;
pub mod water;
mod world;

pub use collision::{TankFootprint, TankObstacle, TankWorldObstacles};
pub use contact::{GroundScales, TerrainContact, sample_tank_terrain_contact};
pub use contact_impulse::{
    ContactBody, ContactCache, ContactImpulse, ContactPair, ContactReport, resolve_contacts,
    separate_overlaps,
};
pub use controller_settings::TankControllerSettings;
pub use cover::{
    footprint_overlaps_cover_object, resolve_cover_collision, resolve_cover_collision_with_velocity,
};
pub use hull_attitude::{HULL_ATTITUDE_RATE_RAD_S, MAX_HULL_TILT_RAD, advance_hull_attitude};
pub use movement::{
    TankControlInput, TankKinematicState, advance_hull_drive, integrate_hull_position,
    step_custom_tank_controller, step_custom_tank_controller_on_contact,
};
pub use parry_query::tank_footprints_intersect_query;
pub use tank_resolve::{resolve_tank_collision, resolve_tank_collision_with_velocity};
pub use track_contact::{SupportContact, sample_support, support_height};
pub use vertical::{GroundStep, is_grounded, resolve_vertical};
pub use world::{
    MAP_BORDER_MARGIN_M, TankStepContact, advance_tank_on_world, clamp_to_map_border,
    settle_tank_on_world, step_tank_on_heightmap, step_tank_on_world,
    step_tank_on_world_with_tanks,
};

// Rapier is gone (2026-08-02, audit finding D6). The `RapierWorld` here, its collider
// constructors and the `PhysicsOwnershipPolicy` beside it were a parallel fiction: an API
// surface consumed only by its own tests while every real path — SAT footprints, heightmap
// stepping, the support envelope — is custom deterministic code. `parry3d` STAYS, on purpose
// and narrowly: `parry_query.rs` runs the live footprint-intersection query.
