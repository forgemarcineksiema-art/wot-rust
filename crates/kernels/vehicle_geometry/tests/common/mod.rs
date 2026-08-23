//! Shared running-gear fixtures.
#![allow(dead_code)]

use game_core::VehicleKind;
use vehicle_geometry::RunningGearKinematics;

/// The benchmark vehicle's blueprint running gear.
pub fn t54() -> RunningGearKinematics {
    RunningGearKinematics::for_vehicle(VehicleKind::T54_1951)
        .expect("T-54 has blueprint running gear")
}
