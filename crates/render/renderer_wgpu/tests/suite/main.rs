//! One test binary for `renderer_wgpu`: every integration test file is a module here, so a change
//! links ONE program instead of one per file (2026-09-06: 314 test binaries across the
//! workspace were ~180 CPU-seconds of linking per heavy crate on every gate). Goldens and
//! env-gated tests (`gate_completeness.rs`) keep their own binaries: they re-record under an
//! env var and must not share a process with a value test of the same file.

#[path = "../common/mod.rs"]
mod common;

mod armor_contour_parity;
mod bloom_energy;
mod capability_report;
mod foliage_mips;
mod frame_graph;
mod frame_profiler;
mod god_rays;
mod gpu_diagnostics;
mod grass_crushers;
mod ground_material;
mod hdr_formation;
mod image_formation;
mod local_lights;
mod msaa_targets;
mod pass_counts;
mod scene_render_frame;
mod shadow_render_frame;
mod sky_formation;
mod ssao_coupling;
mod surface_config;
mod terrain_chunk_culling;
mod vehicle_render_frame;
mod vehicle_resources;
mod wgsl_layout;
