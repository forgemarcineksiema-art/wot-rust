//! Close-view equipment in the T-54 fighting compartment. Large hit volumes still come from
//! `DamageLayout`; this module adds the recognizable D-10T, SG-43 and crew-station construction
//! around those volumes.

use glam::Vec3;
use vehicle_geometry::{MaterialRole, SubmeshKind};

use crate::part::{PartKey, PartLod, VehiclePart};
use crate::t54_interior::{box_part, drum_part};

pub(super) fn add_fighting_parts(parts: &mut Vec<VehiclePart>, cy: f32) {
    add_d10t(parts, cy);
    add_coaxial_sg43(parts, cy);
    add_crew_stations(parts, cy);
    add_wall_equipment(parts, cy);
}

fn add_d10t(parts: &mut Vec<VehiclePart>, cy: f32) {
    parts.push(box_part(
        PartKey::new("d10_breech_block"),
        SubmeshKind::Turret,
        Vec3::new(0.0, cy + 0.82, 0.34),
        Vec3::new(0.24, 0.18, 0.10),
        MaterialRole::InteriorMachinery,
        PartLod::Detail,
    ));
    parts.push(drum_part(
        PartKey::new("d10_breech_ring"),
        SubmeshKind::Turret,
        Vec3::new(0.0, cy + 0.83, 0.54),
        Vec3::Z,
        (0.11, 0.245),
        MaterialRole::InteriorMachinery,
        PartLod::Detail,
    ));
    parts.push(box_part(
        PartKey::new("d10_cradle_bridge"),
        SubmeshKind::Turret,
        Vec3::new(0.0, cy + 1.01, 0.37),
        Vec3::new(0.37, 0.055, 0.045),
        MaterialRole::InteriorMachinery,
        PartLod::Detail,
    ));
    for (index, x) in [-0.30_f32, 0.30].into_iter().enumerate() {
        parts.push(box_part(
            PartKey::indexed("d10_cradle_rail", index as u16),
            SubmeshKind::Turret,
            Vec3::new(x, cy + 1.01, 0.72),
            Vec3::new(0.055, 0.055, 0.36),
            MaterialRole::InteriorMachinery,
            PartLod::Detail,
        ));
    }
    for (index, x) in [-0.25_f32, 0.25].into_iter().enumerate() {
        parts.push(drum_part(
            PartKey::indexed("d10_recoil_cylinder", index as u16),
            SubmeshKind::Turret,
            Vec3::new(x, cy + 0.98, 0.42),
            Vec3::Z,
            (0.42, 0.095),
            MaterialRole::InteriorMachinery,
            PartLod::Detail,
        ));
    }
    for (index, x) in [-0.42_f32, 0.42].into_iter().enumerate() {
        parts.push(box_part(
            PartKey::indexed("breech_guard_rail", index as u16),
            SubmeshKind::Turret,
            Vec3::new(x, cy + 0.57, 0.48),
            Vec3::new(0.018, 0.018, 0.56),
            MaterialRole::InteriorPrimer,
            PartLod::Detail,
        ));
        parts.push(box_part(
            PartKey::indexed("breech_guard_stanchion", index as u16),
            SubmeshKind::Turret,
            Vec3::new(x, cy + 0.77, 0.39),
            Vec3::new(0.018, 0.22, 0.018),
            MaterialRole::InteriorPrimer,
            PartLod::Detail,
        ));
    }
    for (index, z) in [-0.08_f32, 1.04].into_iter().enumerate() {
        parts.push(box_part(
            PartKey::indexed("breech_guard_crossbar", index as u16),
            SubmeshKind::Turret,
            Vec3::new(0.0, cy + 0.57, z),
            Vec3::new(0.44, 0.018, 0.018),
            MaterialRole::InteriorPrimer,
            PartLod::Detail,
        ));
    }
    parts.push(drum_part(
        PartKey::new("d10_breech_handle"),
        SubmeshKind::Turret,
        Vec3::new(0.34, cy + 0.78, 0.42),
        Vec3::Y,
        (0.17, 0.025),
        MaterialRole::InteriorMachinery,
        PartLod::Detail,
    ));
}

fn add_coaxial_sg43(parts: &mut Vec<VehiclePart>, cy: f32) {
    parts.push(box_part(
        PartKey::new("sg43_coax_receiver"),
        SubmeshKind::Turret,
        Vec3::new(0.39, cy + 0.91, 0.52),
        Vec3::new(0.095, 0.10, 0.17),
        MaterialRole::InteriorMachinery,
        PartLod::Detail,
    ));
    parts.push(drum_part(
        PartKey::new("sg43_coax_barrel"),
        SubmeshKind::Turret,
        Vec3::new(0.39, cy + 0.93, 0.87),
        Vec3::Z,
        (0.24, 0.018),
        MaterialRole::InteriorMachinery,
        PartLod::Detail,
    ));
    parts.push(box_part(
        PartKey::new("sg43_ammo_tray"),
        SubmeshKind::Turret,
        Vec3::new(0.53, cy + 0.77, 0.48),
        Vec3::new(0.13, 0.08, 0.18),
        MaterialRole::InteriorPrimer,
        PartLod::Detail,
    ));
}

fn add_crew_stations(parts: &mut Vec<VehiclePart>, cy: f32) {
    for (index, x) in [-0.54_f32, 0.54].into_iter().enumerate() {
        parts.push(box_part(
            PartKey::indexed("turret_seat", index as u16),
            SubmeshKind::Turret,
            Vec3::new(x, cy + 0.25, -0.18),
            Vec3::new(0.20, 0.065, 0.22),
            MaterialRole::InteriorMachinery,
            PartLod::Detail,
        ));
        parts.push(box_part(
            PartKey::indexed("turret_seat_back", index as u16),
            SubmeshKind::Turret,
            Vec3::new(x, cy + 0.52, -0.36),
            Vec3::new(0.20, 0.25, 0.045),
            MaterialRole::InteriorMachinery,
            PartLod::Detail,
        ));
        parts.push(drum_part(
            PartKey::indexed("turret_handwheel", index as u16),
            SubmeshKind::Turret,
            Vec3::new(x * 0.82, cy + 0.66, 0.20),
            Vec3::X,
            (0.035, 0.15),
            MaterialRole::InteriorMachinery,
            PartLod::Detail,
        ));
    }

    parts.push(box_part(
        PartKey::new("tsh2_sight_body"),
        SubmeshKind::Turret,
        Vec3::new(-0.42, cy + 0.92, 0.69),
        Vec3::new(0.075, 0.10, 0.25),
        MaterialRole::InteriorMachinery,
        PartLod::Detail,
    ));
    parts.push(drum_part(
        PartKey::new("tsh2_eyepiece"),
        SubmeshKind::Turret,
        Vec3::new(-0.42, cy + 0.92, 0.41),
        Vec3::Z,
        (0.055, 0.052),
        MaterialRole::InteriorMachinery,
        PartLod::Detail,
    ));
}

fn add_wall_equipment(parts: &mut Vec<VehiclePart>, cy: f32) {
    parts.push(box_part(
        PartKey::new("radio_control_face"),
        SubmeshKind::Turret,
        Vec3::new(-0.66, cy + 0.50, 0.98),
        Vec3::new(0.29, 0.16, 0.025),
        MaterialRole::InteriorPrimer,
        PartLod::Detail,
    ));
    for (index, x) in [-0.80_f32, -0.66, -0.52].into_iter().enumerate() {
        parts.push(drum_part(
            PartKey::indexed("radio_control_dial", index as u16),
            SubmeshKind::Turret,
            Vec3::new(x, cy + 0.53, 1.012),
            Vec3::Z,
            (0.012, 0.035),
            MaterialRole::InteriorMachinery,
            PartLod::Detail,
        ));
    }

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
