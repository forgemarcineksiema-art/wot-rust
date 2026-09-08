//! One test binary for `map_forge`: every integration test file is a module here, so a change
//! links ONE program instead of one per file (2026-09-06: 314 test binaries across the
//! workspace were ~180 CPU-seconds of linking per heavy crate on every gate). Goldens and
//! env-gated tests (`gate_completeness.rs`) keep their own binaries: they re-record under an
//! env var and must not share a process with a value test of the same file.

#[path = "../common/mod.rs"]
mod common;

mod bystra_map;
mod flora_integration;
mod goldens;
mod grid_creases;
mod historical_map;
mod horizon;
mod mazurski_przesmyk;
mod migration;
mod orliny_pereval;
mod ostrogorsk;
mod prokhorovka_balkas;
mod prokhorovka_dressing;
mod prokhorovka_field;
mod prokhorovka_hull_down;
mod report_contracts;
mod rock_boxes;
mod rot180_symmetry;
mod scenery_variety;
mod scratch;
mod standing_water;
mod stroke_ops;
mod topology;
mod tree_trunk_boxes;
