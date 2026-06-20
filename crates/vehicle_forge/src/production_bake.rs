//! The one production bake contract shared by Forge artifacts and runtime consumers.

use game_core::VehicleKind;
use vehicle_geometry::{BakeError, BakedVehicle, bake_vehicle, reduce_vehicle};

use crate::BakeProfile;

/// Bake a vehicle through the production-selected source for `profile`.
///
/// T-54 is the Forge benchmark and uses its hybrid description. Other vehicles intentionally stay
/// on the legacy recipe path until their own Forge migration is complete.
pub fn bake_production_vehicle(
    vehicle: VehicleKind,
    profile: BakeProfile,
) -> Result<BakedVehicle, BakeError> {
    let authored = match vehicle {
        VehicleKind::T54_1951 => vehicle_build::t54_description().build(),
        _ => bake_vehicle(vehicle)?,
    };
    Ok(reduce_vehicle(&authored, profile.lod_level()))
}
