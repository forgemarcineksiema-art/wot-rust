//! One test binary for `physics`: every integration test file is a module here, so a change
//! links ONE program instead of one per file (2026-09-06: 314 test binaries across the
//! workspace were ~180 CPU-seconds of linking per heavy crate on every gate). Goldens and
//! env-gated tests (`gate_completeness.rs`) keep their own binaries: they re-record under an
//! env var and must not share a process with a value test of the same file.

mod climb_envelope;
mod cover_footprint;
mod height_in_contact;
mod hull_attitude;
mod mobility_baseline;
mod movement_model;
mod physics_policy;
mod pivot_mechanism;
mod rigid_body_movement;
mod rollover_unreachable;
mod rubble_support;
mod step_over_low_solids;
mod tank_controller;
mod track_contact;
mod vertical_flight;
mod water_wading;
mod yaw_collision;
