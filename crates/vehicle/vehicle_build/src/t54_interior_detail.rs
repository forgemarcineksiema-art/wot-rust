//! Close-view T-54-3 interior dressing. Major transforms remain owned by `DamageLayout`; these
//! smaller assemblies make those silhouettes read as an actual V-54 power pack, D-10T fighting
//! compartment and torsion-bar hull when seen through ingress and egress.

use glam::Vec3;
use vehicle_geometry::{MaterialRole, SubmeshKind};

use crate::part::{PartKey, PartLod, VehiclePart};
use crate::t54_interior::{box_part, drum_part};

mod ammunition;
mod fighting;

pub(crate) fn t54_museum_detail_parts(center_y: f32) -> Vec<VehiclePart> {
    let mut parts = Vec::new();
    add_v54(&mut parts, center_y);
    add_driveline_and_cooling(&mut parts, center_y);
    add_torsion_bars(&mut parts, center_y);
    ammunition::add_ammunition_parts(&mut parts, center_y);
    fighting::add_fighting_parts(&mut parts, center_y);
    parts
}

fn add_v54(parts: &mut Vec<VehiclePart>, cy: f32) {
    // V-54: crankcase, two canted banks and the central intake/exhaust spine.
    parts.push(box_part(
        PartKey::new("v54_crankcase"),
        SubmeshKind::Hull,
        Vec3::new(0.0, cy - 0.22, -1.90),
        Vec3::new(0.42, 0.20, 0.52),
        MaterialRole::InteriorMachinery,
        PartLod::Detail,
    ));
    for bank in [-1.0_f32, 1.0] {
        for cylinder in 0..6 {
            let z = -2.32 + cylinder as f32 * 0.17;
            parts.push(drum_part(
                PartKey::indexed("v54_cylinder", ((bank > 0.0) as u16) * 6 + cylinder),
                SubmeshKind::Hull,
                Vec3::new(bank * 0.31, cy + 0.05, z),
                Vec3::new(bank * 0.72, 0.69, 0.0),
                (0.16, 0.105),
                MaterialRole::InteriorMachinery,
                PartLod::Detail,
            ));
        }
    }
    parts.push(box_part(
        PartKey::new("v54_intake_spine"),
        SubmeshKind::Hull,
        Vec3::new(0.0, cy + 0.38, -1.90),
        Vec3::new(0.13, 0.10, 0.52),
        MaterialRole::InteriorPrimer,
        PartLod::Detail,
    ));
}

fn add_driveline_and_cooling(parts: &mut Vec<VehiclePart>, cy: f32) {
    for (index, x) in [-0.72_f32, 0.72].into_iter().enumerate() {
        parts.push(box_part(
            PartKey::indexed("radiator_core", index as u16),
            SubmeshKind::Hull,
            Vec3::new(x, cy + 0.27, -2.05),
            Vec3::new(0.18, 0.24, 0.56),
            MaterialRole::InteriorMachinery,
            PartLod::Detail,
        ));
        for fin in 0..7 {
            parts.push(box_part(
                PartKey::indexed("radiator_fin", index as u16 * 7 + fin),
                SubmeshKind::Hull,
                Vec3::new(x, cy + 0.27, -2.48 + fin as f32 * 0.14),
                Vec3::new(0.195, 0.255, 0.012),
                MaterialRole::InteriorPrimer,
                PartLod::Detail,
            ));
        }
        parts.push(drum_part(
            PartKey::indexed("final_drive_housing", index as u16),
            SubmeshKind::Hull,
            Vec3::new(x.signum() * 0.92, cy - 0.18, -2.69),
            Vec3::X,
            (0.18, 0.27),
            MaterialRole::InteriorMachinery,
            PartLod::Detail,
        ));
    }
    for (index, x) in [-0.32_f32, 0.32].into_iter().enumerate() {
        parts.push(drum_part(
            PartKey::indexed("transmission_drum", index as u16),
            SubmeshKind::Hull,
            Vec3::new(x, cy, -2.66),
            Vec3::Z,
            (0.21, 0.24),
            MaterialRole::InteriorMachinery,
            PartLod::Detail,
        ));
    }
}

fn add_torsion_bars(parts: &mut Vec<VehiclePart>, cy: f32) {
    for index in 0..10 {
        let z = -2.45 + index as f32 * 0.54;
        parts.push(drum_part(
            PartKey::indexed("torsion_bar", index),
            SubmeshKind::Hull,
            Vec3::new(0.0, cy - 0.58, z),
            Vec3::X,
            (1.12, 0.035),
            MaterialRole::InteriorMachinery,
            PartLod::Detail,
        ));
    }
}
