//! The single authoritative mesh source, keyed by [`VehicleKind`] — through ONE rule.
//!
//! Everything that renders or forges a vehicle — the client's live bake, the client's artifact
//! validation, and [`crate::ForgeArtifact::bake`] — resolves its geometry here, so the garage, the
//! battle, and the baked artifact can never describe different tanks.
//!
//! Until Forge 2.0 K1 this file was a `match` with one arm: `T54_1951 => Hybrid, _ =>
//! Procedural`, and three other files (`cost.rs`, `production_bake.rs`, the studio) repeated the
//! same fork. Now every vehicle is a [`vehicle_build::VehicleDescription`]: the part library
//! builds the vehicles whose blueprint carries a complete visual, and the lean recipes are wrapped
//! as descriptions for the rest ([`vehicle_recipes::describe`]). What a vehicle may cost and how
//! it reduces are properties the DESCRIPTION declares (`fidelity`, `lod`), so nothing in this crate
//! needs to know which vehicle it is holding. A vehicle migrates by gaining a complete visual in
//! its blueprint — data — not by a new arm here.

use game_core::VehicleKind;
use vehicle_build::{Fidelity, VehicleDescription};
use vehicle_geometry::{BakeError, BakedVehicle};

/// The description the game ships for `kind`.
pub fn authoritative_description(kind: VehicleKind) -> Result<VehicleDescription, BakeError> {
    vehicle_recipes::describe(kind).ok_or(BakeError::MissingRecipe(kind))
}

/// Which cost envelope and golden regime `kind` lives under — read from its description, not
/// from its name.
pub fn shipped_fidelity(kind: VehicleKind) -> Fidelity {
    vehicle_recipes::describe_fidelity(kind)
}

/// The full-detail (LOD0) baked geometry for `kind`, from whichever source owns that vehicle.
pub fn authoritative_baked_vehicle(kind: VehicleKind) -> Result<BakedVehicle, BakeError> {
    Ok(authoritative_description(kind)?.build())
}

#[cfg(test)]
mod tests {
    use super::*;
    use vehicle_recipes::bake_vehicle;

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
        assert_eq!(shipped_fidelity(VehicleKind::T54_1951), Fidelity::Benchmark);
    }

    /// Every vehicle with no library part yet passes straight through to the procedural mesh,
    /// byte for byte. (The Tiger I carries library fittings since K3 and is pinned by its own
    /// mixed golden in `seam_lock`.)
    #[test]
    fn other_vehicles_pass_through_to_the_procedural_mesh() {
        for kind in [VehicleKind::T34_85, VehicleKind::TigerII, VehicleKind::PantherII] {
            let seam = authoritative_baked_vehicle(kind).expect("vehicle bakes");
            let procedural = bake_vehicle(kind).expect("vehicle procedural bakes");
            assert_eq!(
                seam.deterministic_hash(),
                procedural.deterministic_hash(),
                "{kind:?} must pass through the seam unchanged"
            );
            assert_eq!(shipped_fidelity(kind), Fidelity::Sketch);
        }
    }
}
