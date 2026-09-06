//! One test binary for `quality`: every integration test file is a module here, so a change
//! links ONE program instead of one per file (2026-09-06: 314 test binaries across the
//! workspace were ~180 CPU-seconds of linking per heavy crate on every gate). Goldens and
//! env-gated tests (`gate_completeness.rs`) keep their own binaries: they re-record under an
//! env var and must not share a process with a value test of the same file.

mod architecture_rules;
mod camera_rules;
mod combat_rules;
mod coverage_floor;
mod crate_hygiene;
mod data_contracts;
mod feature_unification;
mod gate_completeness;
mod honest_getters;
mod identity_enum_rules;
mod interface_program_rules;
mod kernel_purity;
mod layer_rules;
mod linker_config;
mod math_backend_parity;
mod movement_rules;
mod naming_rules;
mod no_duplicate_free_functions;
mod parry_feature_rules;
mod render_pass_recorder;
mod render_sample_count;
mod roadmap_claims;
mod roundness_rules;
mod simulation_render_boundary;
mod single_source_constants;
mod test_binaries;
mod ui_strings_rules;
mod ui_vertex_equality_ratchet;
mod vehicle_dispatch;
