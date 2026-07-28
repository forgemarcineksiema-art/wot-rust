mod collision;
mod contact;
mod contact_impulse;
mod controller_settings;
mod cover;
mod forces;
mod hull_attitude;
mod movement;
mod parry_query;
mod policy;
mod tank_resolve;
mod track_contact;
mod vertical;
pub mod water;
mod world;

use rapier3d::prelude::{
    Array2, BroadPhaseBvh, Collider, ColliderBuilder, ColliderSet, QueryPipeline, RigidBodySet,
    Vector,
};
use terrain::HeightMap;

pub use collision::{TankFootprint, TankObstacle, TankWorldObstacles, tank_footprints_touch};
pub use contact::{TerrainContact, sample_tank_terrain_contact};
pub use contact_impulse::{ContactBody, ContactImpulse, resolve_contacts, separate_overlaps};
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
pub use policy::{CustomPhysicsRole, PhysicsOwner, PhysicsOwnershipPolicy, RapierPhysicsRole};
pub use tank_resolve::{resolve_tank_collision, resolve_tank_collision_with_velocity};
pub use track_contact::{SupportContact, sample_support, support_height};
pub use vertical::{GroundStep, is_grounded, resolve_vertical};
pub use world::{
    MAP_BORDER_MARGIN_M, TankStepContact, advance_tank_on_world, clamp_to_map_border,
    settle_tank_on_world, step_tank_on_heightmap, step_tank_on_world,
    step_tank_on_world_with_tanks,
};

pub type RapierBroadPhase = BroadPhaseBvh;
pub type RapierQueryPipeline<'a> = QueryPipeline<'a>;

#[derive(Debug, Default)]
pub struct RapierWorld {
    pub rigid_bodies: RigidBodySet,
    pub colliders: ColliderSet,
}

impl RapierWorld {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_static_box_collider(&mut self, half_extents: [f32; 3]) {
        let collider =
            ColliderBuilder::cuboid(half_extents[0], half_extents[1], half_extents[2]).build();
        self.colliders.insert(collider);
    }
}

pub fn make_tank_hull_collider(dimensions_m: [f32; 3]) -> Collider {
    ColliderBuilder::cuboid(
        dimensions_m[0].max(0.01) * 0.5,
        dimensions_m[1].max(0.01) * 0.5,
        dimensions_m[2].max(0.01) * 0.5,
    )
    .friction(1.1)
    .restitution(0.0)
    .build()
}

pub fn make_terrain_heightfield_collider(heightmap: &HeightMap) -> Collider {
    let heights = Array2::from_fn(heightmap.height(), heightmap.width(), |z, x| {
        heightmap.sample_at_index(x, z)
    });

    // Rapier's heightfield `scale` is the TOTAL world extent (not the cell size), and
    // the field is centered on the origin. Use the map extent and translate by half of
    // it so the collider aligns with HeightMap's corner-origin [0, extent] frame.
    let [extent_x, extent_z] = heightmap.extent_m();

    ColliderBuilder::heightfield(heights, Vector::new(extent_x, 1.0, extent_z))
        .translation(Vector::new(extent_x * 0.5, 0.0, extent_z * 0.5))
        .friction(1.2)
        .restitution(0.0)
        .build()
}
