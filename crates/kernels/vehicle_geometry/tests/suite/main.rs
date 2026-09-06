//! One test binary for `vehicle_geometry`: every integration test file is a module here, so a change
//! links ONE program instead of one per file (2026-09-06: 314 test binaries across the
//! workspace were ~180 CPU-seconds of linking per heavy crate on every gate). Goldens and
//! env-gated tests (`gate_completeness.rs`) keep their own binaries: they re-record under an
//! env var and must not share a process with a value test of the same file.

#[path = "../common/mod.rs"]
mod common;

mod contact_cavity;
mod damage_remesh;
mod fleet_running_gear;
mod gear_mesh_quality;
mod kernel;
mod mesh_quality;
mod part_aware_lod;
mod road_wheel_faces;
mod running_gear;
mod running_gear_dynamics;
mod surface_mapping;
