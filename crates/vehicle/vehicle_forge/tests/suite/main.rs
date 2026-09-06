//! One test binary for `vehicle_forge`: every integration test file is a module here, so a change
//! links ONE program instead of one per file (2026-09-06: 314 test binaries across the
//! workspace were ~180 CPU-seconds of linking per heavy crate on every gate). Goldens and
//! env-gated tests (`gate_completeness.rs`) keep their own binaries: they re-record under an
//! env var and must not share a process with a value test of the same file.

mod artifact;
mod compiler;
mod dimension_gate;
mod dossier_claims;
mod fastener_rule;
mod fit_tool;
mod fleet_draw_cost;
mod material_floor;
mod material_law;
mod obj_export;
mod outline_gate;
mod part_graph;
mod part_inventory;
mod reference_pack;
mod reference_provenance;
mod seam_lock;
mod shipped_cost;
mod shipped_mesh_quality;
mod shoe_pitch;
mod studio;
mod studio_live_loop;
mod tile_chirality;
