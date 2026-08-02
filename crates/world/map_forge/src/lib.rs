//! The map forge: battlefields as DATA. A map is a RON blueprint — grid, terrain program,
//! river, water, objects, dressing, roads, gameplay layout — compiled deterministically into
//! the runtime `BattlefieldMap` both ends of the wire agree on. This crate owns the document
//! schema, the structural op vocabulary, the compiler, the contract report, the shipped-map
//! catalog and the golden review gate. Renderer-free by rule (like `world_forge`).

pub mod backdrop;
pub mod blueprint;
mod catalog;
mod compile;
mod golden;
mod ops;
mod report;

pub use backdrop::{backdrop_height, cached_blueprint_by_id};
pub use catalog::{battlefield, blueprint_for, cached_blueprint, set_scratch_source};
pub use compile::{ForgeError, compile};
pub use golden::{battlefield_hash, map_golden_hashes};
pub use report::{
    HullDownSpot, MapReport, ReportEntry, Severity, WaterThresholds, hull_down_positions,
};
