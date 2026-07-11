//! Armored Vehicle Forge foundation.
//!
//! This crate owns the source-of-truth layer above low-level procedural geometry: reference packs,
//! semantic targets, bake reports, and eventually Forge artifacts. It may consume
//! `vehicle_geometry`, but it must stay renderer-backend free.

mod artifact;
mod compiler;
mod mesh_source;
mod packs;
mod packs_british;
mod packs_german;
mod packs_is3;
mod part_data;
mod part_graph;
mod part_manifest;
mod production_bake;
mod reference;
mod reference_measure;
mod registry;
mod report;

pub use artifact::{
    ArtifactError, BakeProfile, ForgeArtifact, ForgeArtifactManifest, ForgeSubmeshManifest,
    ForgeTextureManifest, MaterialFamily, ReviewCamera, ReviewCameraSet, ReviewCameraSpec,
    StudioBundle, SurfaceBakeManifest, bake_studio_bundle, forge_vehicle_slug,
};
pub use compiler::{
    CompiledTank, TankCompileError, TankCompileRequest, TankValidationError, compile_tank,
};
pub use mesh_source::authoritative_baked_vehicle;
pub use packs::t54_reference_pack;
pub use packs_british::centurion_reference_pack;
pub use packs_german::{
    jagdtiger_reference_pack, panther_ii_reference_pack, tiger_i_reference_pack,
    tiger_ii_reference_pack,
};
pub use packs_is3::is3_reference_pack;
pub use part_graph::{
    ForgePart, ForgePartGraph, ForgePartKind, GameplayRole, LodPolicy, PartAnchor, PartGroup,
};
pub use part_manifest::{
    part_manifest_report, production_part_manifest, validate_production_manifest,
};
pub use production_bake::bake_production_vehicle;
pub use reference::{RatioKind, RatioTarget, ReferencePack, ReferenceSource};
pub use report::{MeasuredRatio, RatioReport};
