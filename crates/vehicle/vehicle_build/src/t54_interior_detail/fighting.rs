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
    // ONE height for the whole gun group, taken from the authoritative breech volume — which the
    // layout anchors to the blueprint's trunnion. Every piece that belongs to the gun reads this
    // instead of carrying its own remembered offset, which is how the cradle, the coax and the
    // sight each drifted 330-420 mm above the barrel and out through the casting roof.
    let axis_y = super::obb_anchor(damage_layout, DamageComponentKind::Breech, cy).center.y;
    add_d10t(parts, damage_layout, cy, axis_y);
    add_coaxial_sg43(parts, axis_y);
    add_crew_stations(parts, cy, axis_y);
    wall::add_wall_equipment(parts, damage_layout, cy);
}

fn add_d10t(parts: &mut Vec<VehiclePart>, damage_layout: &DamageLayout, cy: f32, axis_y: f32) {
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
    // The cradle CARRIES the tube, so it rides at the tube's height. It used to sit at
    // `cy + 1.01` — 420 mm above the trunnion, which put the rails through the casting roof
    // (~250 mm proud, the boxes the user photographed on the turret crown).
    parts.push(box_part(
        PartKey::new("d10_cradle_bridge"),
        SubmeshKind::Turret,
        Vec3::new(0.0, axis_y, 0.37),
        Vec3::new(0.37, 0.055, 0.045),
        MaterialRole::InteriorMachinery,
        PartLod::Detail,
    ));
    for (index, x) in [-0.30_f32, 0.30].into_iter().enumerate() {
        parts.push(box_part(
            PartKey::indexed("d10_cradle_rail", index as u16),
            SubmeshKind::Turret,
            Vec3::new(x, axis_y, 0.72),
            Vec3::new(0.055, 0.055, 0.36),
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
        Vec3::new(0.34, axis_y - 0.04, 0.42),
        Vec3::Y,
        (0.17, 0.025),
        MaterialRole::InteriorMachinery,
        PartLod::Detail,
    ));
}

/// The coaxial machine gun, beside the gun in the SAME window the gun comes through.
///
/// It was authored at x 0.39 and 340 mm above the trunnion, which put its muzzle through BARE
/// CASTING 160 mm outside the embrasure — a gun firing through armour that has no port for it,
/// and a 106 mm stub of receiver standing on the turret crown. The T-54 mounts the SGMT to the
/// gunner's right, close enough to share the mantlet opening; `window_az_width` puts the
/// window's edge at x ≈ 0.23, so that is the wall this has to live inside.
fn add_coaxial_sg43(parts: &mut Vec<VehiclePart>, axis_y: f32) {
    parts.push(box_part(
        PartKey::new("sg43_coax_receiver"),
        SubmeshKind::Turret,
        Vec3::new(0.22, axis_y - 0.04, 0.52),
        Vec3::new(0.095, 0.10, 0.17),
        MaterialRole::InteriorMachinery,
        PartLod::Detail,
    ));
    parts.push(drum_part(
        PartKey::new("sg43_coax_barrel"),
        SubmeshKind::Turret,
        // Lengthened with the egg re-registration: the forward-registered waist carries the
        // window's floor out ~25 mm, and the old tube ended inside the casting — a gun with
        // no muzzle (`the_coaxial_muzzle_reaches_daylight_through_the_gun_window`).
        Vec3::new(0.22, axis_y - 0.04, 0.95),
        Vec3::Z,
        (0.28, 0.018),
        MaterialRole::InteriorMachinery,
        PartLod::Detail,
    ));
    parts.push(box_part(
        PartKey::new("sg43_ammo_tray"),
        SubmeshKind::Turret,
        Vec3::new(0.40, axis_y - 0.18, 0.48),
        Vec3::new(0.13, 0.08, 0.18),
        MaterialRole::InteriorPrimer,
        PartLod::Detail,
    ));
}

fn add_crew_stations(parts: &mut Vec<VehiclePart>, cy: f32, axis_y: f32) {
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

    // The TSh-2 is LINKED to the gun: it rides the same trunnion so the sight picture and the
    // bore stay together through elevation. Authored 330 mm above it and 420 mm out, it stood
    // 163 mm through the casting instead — a telescope looking out of solid armour.
    parts.push(box_part(
        PartKey::new("tsh2_sight_body"),
        SubmeshKind::Turret,
        Vec3::new(-0.25, axis_y + 0.03, 0.69),
        Vec3::new(0.075, 0.10, 0.25),
        MaterialRole::InteriorMachinery,
        PartLod::Detail,
    ));
    parts.push(drum_part(
        PartKey::new("tsh2_eyepiece"),
        SubmeshKind::Turret,
        Vec3::new(-0.25, axis_y + 0.03, 0.41),
        Vec3::Z,
        (0.055, 0.052),
        MaterialRole::InteriorMachinery,
        PartLod::Detail,
    ));
}
