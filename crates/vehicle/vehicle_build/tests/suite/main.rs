//! One test binary for `vehicle_build`: every integration test file is a module here, so a change
//! links ONE program instead of one per file (2026-09-06: 314 test binaries across the
//! workspace were ~180 CPU-seconds of linking per heavy crate on every gate). Goldens and
//! env-gated tests (`gate_completeness.rs`) keep their own binaries: they re-record under an
//! env var and must not share a process with a value test of the same file.

#[path = "../common/mod.rs"]
mod common;

mod casemate;
mod construction_floor;
mod henschel_turret;
mod hitbox_fit;
mod part_library;
mod part_lod;
mod quality;
mod soviet_library;
mod surface_bake;
mod t54_exterior_mechanics;
mod t54_hatch_visibility;
mod t54_hybrid;
mod t54_hybrid_details;
mod t54_interior_containment;
mod t54_interior_detail;
mod t54_kernel_contract;
mod t54_muzzle;
mod t54_nose_honesty;
mod t54_reference_quality;
mod t54_reference_spec;
mod t54_silhouette;
mod t54_stern_honesty;
mod t54_stowage_straps;
mod t54_suspension_band;
mod t54_suspension_single_source;
mod t54_turret_armor_lock;
mod t54_turret_containment;
