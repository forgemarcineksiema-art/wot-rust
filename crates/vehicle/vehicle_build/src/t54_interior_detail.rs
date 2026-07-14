//! Close-view T-54-3 interior dressing. Major transforms remain owned by `DamageLayout`; these
//! smaller assemblies make those silhouettes read as an actual V-54 power pack, D-10T fighting
//! compartment and torsion-bar hull when seen through ingress and egress.

use game_core::{DamageComponentKind, DamageLayout};
use glam::Vec3;
use vehicle_geometry::{MaterialRole, SubmeshKind};

use crate::part::{PartKey, PartLod, VehiclePart};
use crate::t54_interior::{box_part, drum_part};

mod ammunition;
mod anchors;
mod driver;
mod fighting;

pub(super) use anchors::{cylinder_anchors, obb_anchor};

pub(crate) fn t54_museum_detail_parts(
    damage_layout: &DamageLayout,
    center_y: f32,
) -> Vec<VehiclePart> {
    let mut parts = Vec::new();
    add_v54(&mut parts, damage_layout, center_y);
    add_v54_ancillaries(&mut parts, damage_layout, center_y);
    add_driveline_and_cooling(&mut parts, damage_layout, center_y);
    add_torsion_bars(&mut parts, damage_layout, center_y);
    ammunition::add_ammunition_parts(&mut parts, damage_layout, center_y);
    fighting::add_fighting_parts(&mut parts, damage_layout, center_y);
    driver::add_driver_parts(&mut parts, center_y);
    parts
}

/// The V-54's ancillaries the museum eye looks for first: the flywheel fan drum on the gearbox
/// side of the block and the air-cleaner canister on the right sponson.
fn add_v54_ancillaries(parts: &mut Vec<VehiclePart>, damage_layout: &DamageLayout, cy: f32) {
    let engine = obb_anchor(damage_layout, DamageComponentKind::Engine, cy);
    parts.push(drum_part(
        PartKey::new("v54_flywheel_fan"),
        SubmeshKind::Hull,
        Vec3::new(0.0, engine.center.y - 0.04, engine.center.z - engine.half.z - 0.10),
        Vec3::Z,
        (0.07, 0.34),
        MaterialRole::InteriorMachinery,
        PartLod::Detail,
    ));
    parts.push(drum_part(
        PartKey::new("v54_air_cleaner"),
        SubmeshKind::Hull,
        engine.center + Vec3::new(engine.half.x + 0.14, 0.10, 0.12),
        Vec3::Y,
        (0.14, 0.115),
        MaterialRole::InteriorMachinery,
        PartLod::Detail,
    ));
}

fn add_v54(parts: &mut Vec<VehiclePart>, damage_layout: &DamageLayout, cy: f32) {
    let engine = obb_anchor(damage_layout, DamageComponentKind::Engine, cy);
    // V-54: crankcase, two canted banks and the central intake/exhaust spine.
    parts.push(box_part(
        PartKey::new("v54_crankcase"),
        SubmeshKind::Hull,
        engine.center - Vec3::Y * engine.half.y * 0.5,
        Vec3::new(engine.half.x * 0.67, engine.half.y * 0.42, engine.half.z * 0.90),
        MaterialRole::InteriorMachinery,
        PartLod::Detail,
    ));
    for bank in [-1.0_f32, 1.0] {
        for cylinder in 0..6 {
            let z = engine.center.z - engine.half.z * 0.72 + cylinder as f32 * engine.half.z * 0.29;
            parts.push(drum_part(
                PartKey::indexed("v54_cylinder", ((bank > 0.0) as u16) * 6 + cylinder),
                SubmeshKind::Hull,
                Vec3::new(
                    engine.center.x + bank * engine.half.x * 0.49,
                    engine.center.y + engine.half.y * 0.06,
                    z,
                ),
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
        engine.center + Vec3::Y * engine.half.y * 0.75,
        Vec3::new(0.13, 0.10, engine.half.z * 0.90),
        MaterialRole::InteriorPrimer,
        PartLod::Detail,
    ));
}

fn add_driveline_and_cooling(parts: &mut Vec<VehiclePart>, damage_layout: &DamageLayout, cy: f32) {
    let engine = obb_anchor(damage_layout, DamageComponentKind::Engine, cy);
    let transmission = obb_anchor(damage_layout, DamageComponentKind::Transmission, cy);
    let final_drives = cylinder_anchors(damage_layout, DamageComponentKind::FinalDrive, cy);
    for (index, x) in [-0.72_f32, 0.72].into_iter().enumerate() {
        parts.push(box_part(
            PartKey::indexed("radiator_core", index as u16),
            SubmeshKind::Hull,
            Vec3::new(x, engine.center.y + engine.half.y * 0.52, engine.center.z - 0.15),
            Vec3::new(0.18, 0.24, engine.half.z * 0.97),
            MaterialRole::InteriorMachinery,
            PartLod::Detail,
        ));
        for fin in 0..7 {
            parts.push(box_part(
                PartKey::indexed("radiator_fin", index as u16 * 7 + fin),
                SubmeshKind::Hull,
                Vec3::new(
                    x,
                    engine.center.y + engine.half.y * 0.52,
                    engine.center.z - engine.half.z + fin as f32 * engine.half.z * 0.24,
                ),
                Vec3::new(0.195, 0.255, 0.012),
                MaterialRole::InteriorPrimer,
                PartLod::Detail,
            ));
        }
        let drive = final_drives[index];
        parts.push(drum_part(
            PartKey::indexed("final_drive_housing", index as u16),
            SubmeshKind::Hull,
            drive.center,
            drive.axis,
            (drive.half_length, drive.radius),
            MaterialRole::InteriorMachinery,
            PartLod::Detail,
        ));
    }
    for (index, x) in [-0.32_f32, 0.32].into_iter().enumerate() {
        parts.push(drum_part(
            PartKey::indexed("transmission_drum", index as u16),
            SubmeshKind::Hull,
            Vec3::new(transmission.center.x + x, transmission.center.y, transmission.center.z),
            Vec3::Z,
            (0.21, 0.24),
            MaterialRole::InteriorMachinery,
            PartLod::Detail,
        ));
    }
}

fn add_torsion_bars(parts: &mut Vec<VehiclePart>, damage_layout: &DamageLayout, cy: f32) {
    let suspension = anchors::capsule_anchors(damage_layout, DamageComponentKind::Suspension, cy);
    let z_min =
        suspension.iter().map(|anchor| anchor.a.z.min(anchor.b.z)).fold(f32::INFINITY, f32::min);
    let z_max = suspension
        .iter()
        .map(|anchor| anchor.a.z.max(anchor.b.z))
        .fold(f32::NEG_INFINITY, f32::max);
    let floor_y =
        suspension.iter().map(|anchor| anchor.a.y).sum::<f32>() / suspension.len() as f32 + 0.12;
    for index in 0..10 {
        let z = z_min + 0.20 + index as f32 * (z_max - z_min - 0.40) / 9.0;
        parts.push(drum_part(
            PartKey::indexed("torsion_bar", index),
            SubmeshKind::Hull,
            Vec3::new(0.0, floor_y, z),
            Vec3::X,
            (1.12, 0.035),
            MaterialRole::InteriorMachinery,
            PartLod::Detail,
        ));
    }
}
