//! One test binary for `game_core`: every integration test file is a module here, so a change
//! links ONE program instead of one per file (2026-09-06: 314 test binaries across the
//! workspace were ~180 CPU-seconds of linking per heavy crate on every gate). Goldens and
//! env-gated tests (`gate_completeness.rs`) keep their own binaries: they re-record under an
//! env var and must not share a process with a value test of the same file.

mod ammo_data;
mod ammo_identity;
mod ammunition;
mod armor_coverage;
mod armor_model;
mod blueprint_source;
mod centurion;
mod damage_layout;
mod damage_layout_fleet;
mod flank_armour;
mod glancing_band;
mod gun_arcs;
mod handedness;
mod is3;
mod jagdtiger;
mod modules;
mod panther_ii;
mod spaced_armor;
mod tiger_i;
mod tiger_ii;
