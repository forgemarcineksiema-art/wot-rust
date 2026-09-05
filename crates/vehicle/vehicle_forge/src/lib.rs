//! Armored Vehicle Forge foundation.
//!
//! This crate owns the source-of-truth layer above low-level procedural geometry: reference packs,
//! semantic targets, bake reports, and eventually Forge artifacts. It may consume
//! `vehicle_geometry`, but it must stay renderer-backend free.

mod artifact;
mod compiler;
mod cost;
mod mesh_source;
mod outline;
mod part_data;
mod part_graph;
mod part_manifest;
mod production_bake;
mod reference;
mod reference_measure;
mod registry;
mod report;

pub use artifact::{
    ArtifactError, BakeProfile, DEFAULT_MATERIAL_MAP_SIZE, DefaultMaterialFamily,
    DefaultMaterialMap, ForgeArtifact, ForgeArtifactManifest, ForgeSubmeshManifest,
    ForgeTextureManifest, MaterialFamily, ObjExport, ReviewCamera, ReviewCameraSet,
    ReviewCameraSpec, ReviewFocus, StudioBundle, SurfaceBakeManifest, bake_studio_bundle,
    bake_studio_bundle_from_blueprint, default_material_families, export_obj, forge_vehicle_slug,
};
pub use compiler::{
    CompiledTank, TankCompileError, TankCompileRequest, TankValidationError, compile_tank,
};
pub use cost::{CostEnvelope, ShippedCostCeiling, shipped_cost_ceiling};
pub use mesh_source::{authoritative_baked_vehicle, authoritative_description, shipped_fidelity};
pub use outline::{
    OUTLINE_CELL_M, OutlineMeasurement, OutlineSet, OutlineSpec, OutlineView, SilhouetteGrid,
    composed_triangles, composed_triangles_for, measure as measure_outline,
    overlay_png as outline_overlay_png, rasterise as rasterise_outline,
};
pub use part_graph::{
    ForgePart, ForgePartGraph, ForgePartKind, GameplayRole, LodPolicy, PartAnchor, PartGroup,
};
pub use part_manifest::{
    part_manifest_report, production_part_manifest, validate_production_manifest,
};
pub use production_bake::bake_production_vehicle;
pub use reference::{
    AnchorStatus, DimensionKind, DimensionTarget, MeasurementBasis, RatioKind, RatioTarget,
    ReferencePack, ReferenceSource, outline_set,
};
pub use reference_measure::composed_visual_bounds;
pub use report::{DimensionReport, MeasuredDimension, MeasuredRatio, RatioReport};
