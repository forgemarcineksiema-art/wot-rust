//! The T-54 driver's station: front-LEFT hull, per the period manuals (the bow ammunition rack
//! owns the right side). Fixed hull-frame construction — the station has no large hit volume of
//! its own, so nothing anchors to the damage layout; the geometry documents the workplace: seat,
//! the two steering levers, pedals, the left-wall instrument panel and the gear selector.

use glam::Vec3;
use vehicle_geometry::{MaterialRole, SubmeshKind};

use crate::part::{PartKey, PartLod, VehiclePart};
use crate::t54_interior::{box_part, drum_part};

pub(super) fn add_driver_parts(parts: &mut Vec<VehiclePart>, cy: f32) {
    let station = Vec3::new(-0.55, cy, 1.55);

    parts.push(box_part(
        PartKey::new("driver_station_seat"),
        SubmeshKind::Hull,
        station + Vec3::new(0.0, -0.30, 0.0),
        Vec3::new(0.20, 0.05, 0.21),
        MaterialRole::InteriorMachinery,
        PartLod::Detail,
    ));
    parts.push(box_part(
        PartKey::new("driver_station_seat_back"),
        SubmeshKind::Hull,
        station + Vec3::new(0.0, -0.05, -0.24),
        Vec3::new(0.20, 0.21, 0.04),
        MaterialRole::InteriorMachinery,
        PartLod::Detail,
    ));
    // The pair of steering levers rises between the driver's knees toward the glacis.
    for (index, x) in [-0.13_f32, 0.13].into_iter().enumerate() {
        parts.push(drum_part(
            PartKey::indexed("driver_steering_lever", index as u16),
            SubmeshKind::Hull,
            station + Vec3::new(x, 0.05, 0.38),
            Vec3::new(0.0, 1.0, 0.28).normalize(),
            (0.26, 0.016),
            MaterialRole::InteriorMachinery,
            PartLod::Detail,
        ));
    }
    // Clutch / brake / accelerator cluster on the sloping floor ahead of the seat.
    for (index, x) in [-0.14_f32, 0.0, 0.14].into_iter().enumerate() {
        parts.push(box_part(
            PartKey::indexed("driver_pedal", index as u16),
            SubmeshKind::Hull,
            station + Vec3::new(x, -0.38, 0.55),
            Vec3::new(0.045, 0.02, 0.06),
            MaterialRole::InteriorMachinery,
            PartLod::Detail,
        ));
    }
    // Instrument panel on the left sponson wall, angled toward the driver.
    parts.push(box_part(
        PartKey::new("driver_instrument_panel"),
        SubmeshKind::Hull,
        // Lowered 3 cm (model-logic audit #17): the panel's top edge reached 1.550 where the
        // foredeck skin is 1.543, so the dials stood through the plate.
        station + Vec3::new(-0.34, 0.19, 0.24),
        Vec3::new(0.035, 0.14, 0.22),
        MaterialRole::InteriorMachinery,
        PartLod::Detail,
    ));
    // Gear selector gate at the driver's right hand.
    parts.push(drum_part(
        PartKey::new("driver_gear_selector"),
        SubmeshKind::Hull,
        station + Vec3::new(0.33, -0.02, 0.10),
        Vec3::new(0.15, 1.0, 0.0).normalize(),
        (0.17, 0.014),
        MaterialRole::InteriorMachinery,
        PartLod::Detail,
    ));
    // TVN night-periscope housing over the station, under the driver's hatch.
    parts.push(box_part(
        PartKey::new("driver_periscope_housing"),
        SubmeshKind::Hull,
        // Reseated UNDER the deck (model-logic audit #17): at +0.52 the housing topped 1.650
        // where the glacis fold's skin is 1.520 — the night periscope floated over the plate
        // instead of hanging beneath the driver's hatch.
        station + Vec3::new(0.0, 0.24, 0.42),
        Vec3::new(0.11, 0.06, 0.08),
        MaterialRole::InteriorMachinery,
        PartLod::Detail,
    ));
    // The compressed-air start bottles stand against the glacis at the driver's right.
    for (index, x) in [0.62_f32, 0.80].into_iter().enumerate() {
        parts.push(drum_part(
            PartKey::indexed("driver_air_bottle", index as u16),
            SubmeshKind::Hull,
            Vec3::new(-station.x.abs() + x + 0.0, cy + 0.05, 2.02),
            Vec3::new(0.0, 1.0, 0.35).normalize(),
            (0.20, 0.065),
            MaterialRole::InteriorPrimer,
            PartLod::Detail,
        ));
    }
}
