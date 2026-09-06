//! One test binary for `renderer_api`: every integration test file is a module here, so a change
//! links ONE program instead of one per file (2026-09-06: 314 test binaries across the
//! workspace were ~180 CPU-seconds of linking per heavy crate on every gate). Goldens and
//! env-gated tests (`gate_completeness.rs`) keep their own binaries: they re-record under an
//! env var and must not share a process with a value test of the same file.

mod capability_tiers;
mod culling;
mod depth_convention;
mod hud_vertex_lanes;
mod look_locks;
mod projection_matrix;
mod render_settings;
mod resource_registry;
mod scene_lighting;
mod scene_vertex_lanes;
