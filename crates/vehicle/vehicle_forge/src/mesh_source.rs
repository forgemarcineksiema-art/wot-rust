//! The single authoritative mesh source, keyed by [`VehicleKind`].
//!
//! Everything that renders or forges a vehicle — the client's live bake, the client's artifact
//! validation, and [`crate::ForgeArtifact::bake`] — resolves its geometry here, so the garage, the
//! battle, and the baked artifact can never describe different tanks. Migrated vehicles return their
//! denser hybrid mesh (CAD plates + SDF castings + revolved parts, via [`vehicle_build`]); the rest
//! pass through to the lean procedural recipe ([`vehicle_geometry::bake_vehicle`]).
//!
//! This is the seam that lets a vehicle move onto the hybrid pipeline without touching the renderer:
//! the hybrid [`vehicle_build::VehicleDescription::build`] yields the same [`BakedVehicle`] the
//! procedural path does, so the consumers do not know which source produced it.

use game_core::VehicleKind;
use vehicle_geometry::{BakeError, BakedVehicle, bake_vehicle};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MeshSourceKind {
    Procedural,
    Hybrid,
}

pub(crate) fn mesh_source_kind(kind: VehicleKind) -> MeshSourceKind {
    match kind {
        VehicleKind::T54_1951 => MeshSourceKind::Hybrid,
        _ => MeshSourceKind::Procedural,
    }
}

/// The full-detail (LOD0) baked geometry for `kind`, from whichever source owns that vehicle.
///
/// The T-54 is the hybrid benchmark and bakes through [`vehicle_build`] at its default loadout
/// (matching the loadout-independent procedural path); every other vehicle bakes through the
/// procedural recipe unchanged.
pub fn authoritative_baked_vehicle(kind: VehicleKind) -> Result<BakedVehicle, BakeError> {
    match mesh_source_kind(kind) {
        MeshSourceKind::Hybrid => Ok(vehicle_build::t54_description().build()),
        MeshSourceKind::Procedural => bake_vehicle(kind),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn triangles(vehicle: &BakedVehicle) -> usize {
        vehicle.submeshes().iter().map(|submesh| submesh.mesh.triangle_count()).sum()
    }

    /// The T-54 must resolve to the dense hybrid, not the lean procedural mesh — the whole point of
    /// the seam. The hybrid carries multiples more triangles (multi-slope hull, running gear, cast
    /// turret, fittings) than the procedural recipe, so a generous factor distinguishes them.
    #[test]
    fn t54_routes_to_the_dense_hybrid_not_the_lean_procedural_mesh() {
        let seam = authoritative_baked_vehicle(VehicleKind::T54_1951).expect("T-54 bakes");
        let procedural = bake_vehicle(VehicleKind::T54_1951).expect("T-54 procedural bakes");
        assert_ne!(seam.deterministic_hash(), procedural.deterministic_hash());
        assert!(
            triangles(&seam) > triangles(&procedural) * 3 / 2,
            "T-54 seam {} tris must be the dense hybrid, not the procedural {} tris",
            triangles(&seam),
            triangles(&procedural)
        );
    }

    /// Every unmigrated vehicle passes straight through to the procedural mesh, byte for byte.
    #[test]
    fn other_vehicles_pass_through_to_the_procedural_mesh() {
        for kind in [VehicleKind::T34_85, VehicleKind::TigerI, VehicleKind::PantherII] {
            let seam = authoritative_baked_vehicle(kind).expect("vehicle bakes");
            let procedural = bake_vehicle(kind).expect("vehicle procedural bakes");
            assert_eq!(
                seam.deterministic_hash(),
                procedural.deterministic_hash(),
                "{kind:?} must pass through the seam unchanged"
            );
        }
    }
}
