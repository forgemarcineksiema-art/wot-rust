//! One test binary for `vehicle_recipes`: every integration test file is a module here, so a change
//! links ONE program instead of one per file (2026-09-06: 314 test binaries across the
//! workspace were ~180 CPU-seconds of linking per heavy crate on every gate). Goldens and
//! env-gated tests (`gate_completeness.rs`) keep their own binaries: they re-record under an
//! env var and must not share a process with a value test of the same file.

#[path = "../common/mod.rs"]
mod common;

mod all_vehicles;
mod centurion_benchmark;
mod contact_on_production_bakes;
mod fleet_construction;
mod fleet_mesh_quality;
mod is3_benchmark;
mod is3_pike;
mod jagdtiger_benchmark;
mod panther_ii_benchmark;
mod recipe_pieces;
mod silhouette_quality;
mod t34_85_benchmark;
mod t54_benchmark_reset;
mod t54_front_reset;
mod tiger_i_benchmark;
mod tiger_ii_benchmark;
mod vehicle_budgets;
mod vehicle_fittings;
mod vehicle_lod;
mod vehicle_recipe;
