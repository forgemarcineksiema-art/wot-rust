//! Armored Vehicle Forge foundation.
//!
//! This crate owns the source-of-truth layer above low-level procedural geometry: reference packs,
//! semantic targets, bake reports, and eventually Forge artifacts. It may consume
//! `vehicle_geometry`, but it must stay renderer-backend free.

mod artifact;
mod packs;
mod packs_german;
mod part_data;
mod part_graph;
mod reference;
mod report;

pub use artifact::{
    ArtifactError, BakeProfile, ForgeArtifact, ForgeArtifactManifest, ForgeSubmeshManifest,
    ForgeTextureManifest, ReviewCamera, ReviewCameraSet, ReviewCameraSpec, forge_vehicle_slug,
};
pub use packs::t54_reference_pack;
pub use packs_german::{
    jagdtiger_reference_pack, panther_ii_reference_pack, tiger_i_reference_pack,
    tiger_ii_reference_pack,
};
pub use part_graph::{ForgePart, ForgePartGraph, ForgePartKind, PartAnchor};
pub use reference::{RatioKind, RatioTarget, ReferencePack, ReferenceSource};
pub use report::{MeasuredRatio, RatioReport};
