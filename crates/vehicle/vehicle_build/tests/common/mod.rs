//! Shared helpers for the `vehicle_build` test suites.
//!
//! Small, but shared on purpose: `is_interior` decides what the construction floor may ignore AND
//! what the turret containment lock must judge. Two copies of that predicate would let a new
//! material role join one list and not the other, and the part carrying it would answer to
//! neither test.
#![allow(dead_code)]

use vehicle_build::t54_description;
use vehicle_geometry::{GeometryMesh, MaterialRole, SubmeshKind};

/// Roles that are only ever seen through a breach.
///
/// Derived from the MATERIAL rather than from a name list, so a new interior part classifies
/// itself instead of waiting for someone to remember to add it.
pub fn is_interior(role: MaterialRole) -> bool {
    matches!(
        role,
        MaterialRole::InteriorPrimer | MaterialRole::InteriorMachinery | MaterialRole::Ammunition
    )
}

/// The benchmark turret, built and cloned out of the bake.
pub fn turret() -> GeometryMesh {
    t54_description().build().submesh(SubmeshKind::Turret).expect("turret").mesh.clone()
}
