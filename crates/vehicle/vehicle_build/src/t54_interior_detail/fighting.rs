//! Close-view equipment in the T-54 fighting compartment. Large hit volumes still come from
//! `DamageLayout`; this module adds the recognizable D-10T, SG-43 and crew-station construction
//! around those volumes.

use game_core::{DamageComponentKind, DamageLayout};
use glam::Vec3;
use vehicle_geometry::{MaterialRole, SubmeshKind};

use crate::part::{PartKey, PartLod, VehiclePart};
use crate::t54_interior::{box_part, drum_part};

mod wall;

pub(super) fn add_fighting_parts(
    parts: &mut Vec<VehiclePart>,
    damage_layout: &DamageLayout,
    cy: f32,
) {
    // One bore height for everything mounted on the gun, taken from the breech volume so the
    // whole train follows the barrel (model-logic audit #17).
    let bore_y = super::obb_anchor(damage_layout, DamageComponentKind::Breech, cy).center.y;
    add_d10t(parts, damage_layout, cy);
    add_coaxial_sg43(parts, bore_y);
    add_crew_stations(parts, cy, bore_y);
    wall::add_wall_equipment(parts, damage_layout, cy);
}

fn add_d10t(parts: &mut Vec<VehiclePart>, damage_layout: &DamageLayout, cy: f32) {
    let breech = super::obb_anchor(damage_layout, DamageComponentKind::Breech, cy);
    let recoil = super::cylinder_anchors(damage_layout, DamageComponentKind::RecoilMechanism, cy);
    parts.push(box_part(
        PartKey::new("d10_breech_block"),
        SubmeshKind::Turret,
        breech.center - Vec3::Z * breech.half.z * 0.56,
        Vec3::new(0.24, 0.18, 0.10),
        MaterialRole::InteriorMachinery,
        PartLod::Detail,
    ));
    parts.push(drum_part(
        PartKey::new("d10_breech_ring"),
        SubmeshKind::Turret,
        breech.center - Vec3::Z * breech.half.z * 0.09,
        Vec3::Z,
        (0.11, 0.245),
        MaterialRole::InteriorMachinery,
        PartLod::Detail,
    ));
    // The gun line hangs off the BREECH VOLUME, not off the hull centre (model-logic audit #17).
    // While these carried absolute `cy + …` heights they stayed put when the breech was moved
    // onto the bore axis, and the cradle rails ended up 33 cm through the casting roof. Anchored
    // here, the whole train follows the barrel wherever the damage layout puts it.
    let bore_y = breech.center.y;
    parts.push(box_part(
        PartKey::new("d10_cradle_bridge"),
        SubmeshKind::Turret,
        Vec3::new(0.0, bore_y + 0.19, 0.37),
        Vec3::new(0.37, 0.055, 0.045),
        MaterialRole::InteriorMachinery,
        PartLod::Detail,
    ));
    for (index, x) in [-0.26_f32, 0.26].into_iter().enumerate() {
        parts.push(box_part(
            PartKey::indexed("d10_cradle_rail", index as u16),
            SubmeshKind::Turret,
            // Z anchored to the breech as well: the rails run forward from it toward the
            // mantlet seat and STOP short of the casting front, instead of reaching a fixed
            // 1.08 that the front wall closes in on as the casting narrows.
            Vec3::new(x, bore_y + 0.15, breech.center.z + 0.02),
            Vec3::new(0.055, 0.055, 0.22),
            MaterialRole::InteriorMachinery,
            PartLod::Detail,
        ));
    }
    for (index, anchor) in recoil.into_iter().enumerate() {
        parts.push(drum_part(
            PartKey::indexed("d10_recoil_cylinder", index as u16),
            SubmeshKind::Turret,
            anchor.center,
            anchor.axis,
            (anchor.half_length, anchor.radius),
            MaterialRole::InteriorMachinery,
            PartLod::Detail,
        ));
    }
    for (index, x) in [-0.42_f32, 0.42].into_iter().enumerate() {
        parts.push(box_part(
            PartKey::indexed("breech_guard_rail", index as u16),
            SubmeshKind::Turret,
            Vec3::new(x, bore_y - 0.25, 0.48),
            Vec3::new(0.018, 0.018, 0.56),
            MaterialRole::InteriorPrimer,
            PartLod::Detail,
        ));
        parts.push(box_part(
            PartKey::indexed("breech_guard_stanchion", index as u16),
            SubmeshKind::Turret,
            Vec3::new(x, bore_y - 0.05, 0.39),
            Vec3::new(0.018, 0.22, 0.018),
            MaterialRole::InteriorPrimer,
            PartLod::Detail,
        ));
    }
    // The forward crossbar was at z 1.04 with a 0.44 half-span, so its corners cleared the
    // casting front. The guard frames the breech; it does not reach the mantlet.
    for (index, z) in [-0.08_f32, 0.88].into_iter().enumerate() {
        parts.push(box_part(
            PartKey::indexed("breech_guard_crossbar", index as u16),
            SubmeshKind::Turret,
            Vec3::new(0.0, bore_y - 0.25, z),
            Vec3::new(0.40, 0.018, 0.018),
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

/// The coaxial SG-43 is bolted to the gun's cradle, so it rides the BORE height, not the hull
/// centre — at an absolute `cy + 0.93` its barrel stood 26 cm through the casting front.
fn add_coaxial_sg43(parts: &mut Vec<VehiclePart>, bore_y: f32) {
    parts.push(box_part(
        PartKey::new("sg43_coax_receiver"),
        SubmeshKind::Turret,
        Vec3::new(0.39, bore_y + 0.09, 0.52),
        Vec3::new(0.095, 0.10, 0.17),
        MaterialRole::InteriorMachinery,
        PartLod::Detail,
    ));
    parts.push(drum_part(
        PartKey::new("sg43_coax_barrel"),
        SubmeshKind::Turret,
        // Shortened and pulled back: the muzzle end reached z 1.11, through the casting front.
        // The SG-43's barrel fires through the mantlet aperture, it does not stand outside it.
        Vec3::new(0.39, bore_y + 0.11, 0.74),
        Vec3::Z,
        (0.20, 0.018),
        MaterialRole::InteriorMachinery,
        PartLod::Detail,
    ));
    parts.push(box_part(
        PartKey::new("sg43_ammo_tray"),
        SubmeshKind::Turret,
        Vec3::new(0.53, bore_y - 0.05, 0.48),
        Vec3::new(0.13, 0.08, 0.18),
        MaterialRole::InteriorPrimer,
        PartLod::Detail,
    ));
}

fn add_crew_stations(parts: &mut Vec<VehiclePart>, cy: f32, bore_y: f32) {
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

    // The TSh-2 is linked to the gun and articulates with it, so it rides the bore height too —
    // left at an absolute `cy + 0.92` its body stood 22 cm through the casting (audit #17).
    parts.push(box_part(
        PartKey::new("tsh2_sight_body"),
        SubmeshKind::Turret,
        Vec3::new(-0.42, bore_y + 0.10, 0.62),
        Vec3::new(0.075, 0.10, 0.22),
        MaterialRole::InteriorMachinery,
        PartLod::Detail,
    ));
    parts.push(drum_part(
        PartKey::new("tsh2_eyepiece"),
        SubmeshKind::Turret,
        Vec3::new(-0.42, bore_y + 0.10, 0.41),
        Vec3::Z,
        (0.055, 0.052),
        MaterialRole::InteriorMachinery,
        PartLod::Detail,
    ));
}
