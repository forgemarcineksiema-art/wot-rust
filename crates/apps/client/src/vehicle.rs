//! Vehicle rendering: turning baked/forged vehicle geometry into render objects, meshes, poses,
//! material roles, equipment anchors, and per-vehicle visual variation. Grouped from the former
//! top-level `vehicle_*` modules so the crate root stays scannable.

pub(crate) mod aperture_rim;
pub(crate) mod asset_catalog;
pub(crate) mod asset_catalog_loader;
pub(crate) mod asset_render;
pub mod damage_budget;
#[cfg(test)]
mod damage_skin_tests;
pub(crate) mod damage_worker;
pub(crate) mod display;
pub(crate) mod equipment;
pub(crate) mod geometry_mesh;
pub(crate) mod mesh;
pub(crate) mod pbr_mesh;
pub(crate) mod pose;
pub(crate) mod render_frame;
pub(crate) mod render_objects;
pub(crate) mod render_objects_draw;
pub(crate) mod running_gear_objects;
pub(crate) mod track_ribbon;
pub(crate) mod turret_popoff;
pub(crate) mod variation;
pub(crate) mod wreck_deform;
