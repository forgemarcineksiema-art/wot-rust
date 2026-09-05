//! The one production bake contract shared by Forge artifacts and runtime consumers.

use game_core::VehicleKind;
use vehicle_geometry::{BakeError, BakedVehicle};

use crate::BakeProfile;
use crate::mesh_source::authoritative_description;

/// Bake a vehicle through the production-selected source for `profile`.
///
/// One rule for the fleet (Forge 2.0 K1): the vehicle's description knows how it reduces —
/// part-aware for the part library (each part clustered by its own importance before the merge,
/// audited in `part_aware_lod`), whole-mesh for a recipe sketch (flatten, then cluster) — so this
/// function does not.
pub fn bake_production_vehicle(
    vehicle: VehicleKind,
    profile: BakeProfile,
) -> Result<BakedVehicle, BakeError> {
    Ok(authoritative_description(vehicle)?.production_bake(profile.lod_level()))
}
