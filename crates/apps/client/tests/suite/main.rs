//! One test binary for `client`: every integration test file is a module here, so a change
//! links ONE program instead of one per file (2026-09-06: 314 test binaries across the
//! workspace were ~180 CPU-seconds of linking per heavy crate on every gate). Goldens and
//! env-gated tests (`gate_completeness.rs`) keep their own binaries: they re-record under an
//! env var and must not share a process with a value test of the same file.

#[path = "../common/mod.rs"]
mod common;

mod armor_breach_replication;
mod audio_rt;
mod battle_camera;
mod camera_feel;
mod dressing_budget;
mod interpolated_render_state;
mod probe_foliage_atlas;
mod procedural_vehicle_geometry;
mod terrain_ground_maps;
mod vehicle_asset_catalog;
mod vehicle_material_ids;
mod vehicle_render_cache;
mod vehicle_render_frame;
mod vehicle_render_objects;
mod vehicle_variation;
mod winit_loop_policy;
