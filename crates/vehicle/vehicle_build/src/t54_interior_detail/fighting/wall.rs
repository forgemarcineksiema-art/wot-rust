use game_core::{DamageComponentKind, DamageLayout};
use glam::Vec3;
use vehicle_geometry::{MaterialRole, SubmeshKind};

use crate::part::{PartKey, PartLod, VehiclePart};
use crate::t54_interior::{box_part, drum_part};

pub(super) fn add_wall_equipment(
    parts: &mut Vec<VehiclePart>,
    damage_layout: &DamageLayout,
    cy: f32,
) {
    let radio = super::super::obb_anchor(damage_layout, DamageComponentKind::Radio, cy);
    parts.push(box_part(
        PartKey::new("radio_control_face"),
        SubmeshKind::Turret,
        radio.center + Vec3::Y * radio.half.y * 0.55 + Vec3::Z * (radio.half.z * 0.45),
        Vec3::new(radio.half.x, radio.half.y * 0.72, 0.025),
        MaterialRole::InteriorPrimer,
        PartLod::Detail,
    ));
    for (index, x_offset) in [-0.52_f32, 0.0, 0.52].into_iter().enumerate() {
        parts.push(drum_part(
            PartKey::indexed("radio_control_dial", index as u16),
            SubmeshKind::Turret,
            Vec3::new(
                radio.center.x + x_offset * radio.half.x,
                radio.center.y + radio.half.y * 0.68,
                radio.center.z + radio.half.z * 0.45 + 0.032,
            ),
            Vec3::Z,
            (0.012, 0.035),
            MaterialRole::InteriorMachinery,
            PartLod::Detail,
        ));
    }

    let turret_drive =
        super::super::cylinder_anchors(damage_layout, DamageComponentKind::TurretDrive, cy)
            .into_iter()
            .next()
            .expect("T-54 turret drive anchor");
    parts.push(drum_part(
        PartKey::new("turret_drive_housing"),
        SubmeshKind::Turret,
        turret_drive.center,
        turret_drive.axis,
        (turret_drive.half_length, turret_drive.radius),
        MaterialRole::InteriorMachinery,
        PartLod::Detail,
    ));
    parts.push(box_part(
        PartKey::new("turret_drive_gearbox"),
        SubmeshKind::Turret,
        turret_drive.center + Vec3::Z * (turret_drive.radius + 0.10),
        Vec3::new(0.15, 0.16, 0.10),
        MaterialRole::InteriorMachinery,
        PartLod::Detail,
    ));

    for (index, x) in [0.49_f32, 0.68].into_iter().enumerate() {
        parts.push(drum_part(
            PartKey::indexed("turret_extinguisher", index as u16),
            SubmeshKind::Turret,
            Vec3::new(x, cy + 0.48, -0.57),
            Vec3::Y,
            (0.20, 0.07),
            MaterialRole::InteriorPrimer,
            PartLod::Detail,
        ));
    }
}
