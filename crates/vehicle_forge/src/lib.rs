//! Armored Vehicle Forge foundation.
//!
//! This crate owns the source-of-truth layer above low-level procedural geometry: reference packs,
//! semantic targets, bake reports, and eventually Forge artifacts. It may consume
//! `vehicle_geometry`, but it must stay renderer-backend free.

mod packs;
mod part_data;
mod part_graph;
mod reference;
mod report;

pub use packs::t54_t55_reference_pack;
pub use part_graph::{ForgePart, ForgePartGraph, ForgePartKind, PartAnchor};
pub use reference::{RatioKind, RatioTarget, ReferencePack, ReferenceSource};
pub use report::{MeasuredRatio, RatioReport};
