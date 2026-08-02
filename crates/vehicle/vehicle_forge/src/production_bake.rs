//! The one production bake contract shared by Forge artifacts and runtime consumers.

use game_core::VehicleKind;
use vehicle_geometry::{BakeError, BakedVehicle, reduce_vehicle};
use vehicle_recipes::bake_vehicle;

use crate::BakeProfile;

/// Bake a vehicle through the production-selected source for `profile`.
///
/// T-54 is the Forge benchmark and uses its hybrid description. Other vehicles intentionally stay
/// on the legacy recipe path until their own Forge migration is complete.
pub fn bake_production_vehicle(
    vehicle: VehicleKind,
    profile: BakeProfile,
) -> Result<BakedVehicle, BakeError> {
    let level = profile.lod_level();
    match vehicle {
        // Part-aware LOD: the hybrid T-54 drops the parts excluded at this tier and clusters each
        // surviving part by its own importance *before* the merge — never a reduction of a flattened
        // LOD0 blob. The result is audited against the full bake in `part_aware_lod` tests.
        VehicleKind::T54_1951 => Ok(vehicle_build::t54_description().build_reduced_lod(level)),
        // Legacy vehicles still flatten then globally cluster until their own Forge migration lands.
        _ => Ok(reduce_vehicle(&bake_vehicle(vehicle)?, level)),
    }
}
