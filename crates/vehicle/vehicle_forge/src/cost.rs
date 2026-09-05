//! What the mesh the GAME ships is allowed to cost, keyed by [`VehicleKind`].
//!
//! The fleet has two bake paths and therefore two cost envelopes, which is correct — a hybrid
//! carrying CAD plates and lofted castings has no business being held to the lean procedural recipe's
//! numbers. What was missing is a single place that says WHICH envelope applies to a given
//! vehicle, so the question "what may this tank cost?" has one answer instead of depending on the
//! reader already knowing which crate bakes it.
//!
//! Without that, the numbers looked like a contradiction — 26,000 triangles in `vehicle_build`
//! against a `vehicle_tri` maximum of 3,950 in `vehicle_recipes` — and the seam between them was
//! written down only inside Forge Studio's report text.
//!
//! This mirrors what `tests/shipped_mesh_quality.rs` already does for mesh QUALITY. That file
//! exists because the procedural audit "cannot see `vehicle_build`", leaving the hybrid T-54
//! outside every quality gate; the identical hole existed for COST, and on the vertex axis it was
//! not hypothetical (see [`vehicle_build::MEDIUM_LOD0_VERT_BUDGET`]).

use game_core::VehicleKind;
use vehicle_recipes::VEHICLE_BUDGETS;

use crate::mesh_source::{MeshSourceKind, mesh_source_kind};

/// The ceiling a shipped LOD0 mesh must stay under, and which envelope it came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShippedCostCeiling {
    /// Triangles across all submeshes.
    pub tri_max: usize,
    /// Vertices across all submeshes. Not a restatement of `tri_max`: smoothing-group, material
    /// and UV splits duplicate vertices without adding triangles.
    pub vert_max: usize,
    /// Which bake path set these numbers — carried so a failure names the envelope it broke.
    pub envelope: CostEnvelope,
}

/// The bake path a vehicle's ceiling comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CostEnvelope {
    /// The lean procedural recipe: `vehicle_recipes::VEHICLE_BUDGETS`.
    ProceduralFleet,
    /// The dense hybrid: `vehicle_build`'s per-class LOD0 budgets.
    HybridClass,
}

/// What `kind`'s shipped mesh may cost, from whichever source owns that vehicle.
///
/// Pairs with [`crate::authoritative_baked_vehicle`]: that resolves the mesh the game draws, this
/// resolves the budget it is judged against. Using one without the other is how a migrated vehicle
/// silently loses its cost gate.
pub fn shipped_cost_ceiling(kind: VehicleKind) -> ShippedCostCeiling {
    match mesh_source_kind(kind) {
        MeshSourceKind::Hybrid => ShippedCostCeiling {
            tri_max: vehicle_build::MEDIUM_LOD0_TRI_BUDGET,
            vert_max: vehicle_build::MEDIUM_LOD0_VERT_BUDGET,
            envelope: CostEnvelope::HybridClass,
        },
        MeshSourceKind::Procedural => ShippedCostCeiling {
            tri_max: VEHICLE_BUDGETS.vehicle_tri.1,
            vert_max: VEHICLE_BUDGETS.vehicle_vert_max,
            envelope: CostEnvelope::ProceduralFleet,
        },
    }
}
