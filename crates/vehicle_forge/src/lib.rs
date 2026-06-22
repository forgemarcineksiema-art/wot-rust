//! Armored Vehicle Forge foundation.
//!
//! This crate owns the source-of-truth layer above low-level procedural geometry: reference packs,
//! semantic targets, bake reports, and eventually Forge artifacts. It may consume
//! `vehicle_geometry`, but it must stay renderer-backend free.

mod artifact;
mod mesh_source;
mod packs;
mod packs_german;
mod part_data;
mod part_graph;
mod part_manifest;
mod production_bake;
mod reference;
mod registry;
mod report;

pub use artifact::{
    ArtifactError, BakeProfile, ForgeArtifact, ForgeArtifactManifest, ForgeSubmeshManifest,
    ForgeTextureManifest, MaterialFamily, ReviewCamera, ReviewCameraSet, ReviewCameraSpec,
    forge_vehicle_slug,
};
pub use mesh_source::authoritative_baked_vehicle;
pub use packs::t54_reference_pack;
pub use packs_german::{
    jagdtiger_reference_pack, panther_ii_reference_pack, tiger_i_reference_pack,
    tiger_ii_reference_pack,
};
pub use part_graph::{
    ForgePart, ForgePartGraph, ForgePartKind, GameplayRole, LodPolicy, PartAnchor, PartGroup,
};
pub use part_manifest::{
    part_manifest_report, production_part_manifest, validate_production_manifest,
};
pub use production_bake::bake_production_vehicle;
pub use reference::{RatioKind, RatioTarget, ReferencePack, ReferenceSource};
pub use report::{MeasuredRatio, RatioReport};
