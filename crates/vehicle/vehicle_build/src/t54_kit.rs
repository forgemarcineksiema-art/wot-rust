//! The T-54's external kit: the fender stowage line (fuel tanks and bins on the shelves over the
//! tracks), the sloping fender end sections, the glacis splash board, the turret handrails and the
//! stowed tow cables. All are `PartLod::Detail` visual parts derived from the blueprint's
//! [`HybridVisual`]; none adds a gameplay dimension. The kit is what makes the narrow-box hull read
//! as a T-54: the tracks stay exposed and the shelves above them carry the visual mass.

use game_core::{FenderVisual, HybridVisual};
use glam::Vec3;
use vehicle_geometry::{MaterialRole, SubmeshKind};

use crate::part::{GeneratorKind, PartKey, PartLod, PartShape, VehiclePart};
use crate::t54_details::detail_plate;

/// Every kit part: fender stowage, sloping fender ends, splash board, turret rails, tow cables.
pub fn t54_kit_parts(v: &HybridVisual, glacis_deg: f32) -> Vec<VehiclePart> {
    let mut parts = Vec::new();
    parts.extend(fender_stowage(&v.fender));
    for (i, side) in [v.fender.side_x, -v.fender.side_x].into_iter().enumerate() {
        for (j, sign) in [1.0_f32, -1.0].into_iter().enumerate() {
            parts.push(detail_plate(
                PartKey::indexed("fender_slope", (i * 2 + j) as u16),
                SubmeshKind::Hull,
                MaterialRole::RolledArmor,
                solid::t54_fender_slope(side, &v.fender, sign),
            ));
        }
    }
    // Line-work: splash board, turret rails, tow cables, the unditching beam, the travel lock.
    parts.extend(crate::t54_kit_lines::t54_line_kit_parts(v, glacis_deg));
    parts
}

/// The fender stowage line from the references' top view: a forward bin, the flat external fuel
/// tanks and a rear bin on the right shelf; toolboxes fore and aft of the exhaust cover (placed by
/// `t54_details`) on the left. Chamfered pressings, tops well below the roof, outer faces just
/// inside the track span.
fn fender_stowage(fender: &FenderVisual) -> Vec<VehiclePart> {
    let base = fender.center_y + fender.half.y + 0.005;
    // (side, key, z centre, half length, half height)
    let boxes: [(f32, &str, f32, f32, f32); 8] = [
        (1.0, "stowage_bin", 2.15, 0.28, 0.15),
        (1.0, "fuel_tank", 1.05, 0.42, 0.13),
        (1.0, "fuel_tank", -0.15, 0.42, 0.13),
        (1.0, "fuel_tank", -1.35, 0.40, 0.13),
        (1.0, "stowage_bin", -2.35, 0.28, 0.14),
        (-1.0, "stowage_bin", 1.95, 0.40, 0.15),
        (-1.0, "stowage_bin", 0.55, 0.35, 0.14),
        (-1.0, "stowage_bin", -2.30, 0.35, 0.14),
    ];
    let mut parts = Vec::new();
    for (i, &(side, key, z, half_z, half_y)) in boxes.iter().enumerate() {
        // Centred on the SHELF, not on a copy of where the shelf used to be. This was a
        // literal 1.34 beside a fender that has since moved with the documented track gauge.
        let center = Vec3::new(side * fender.side_x, base + half_y, z);
        let half = Vec3::new(0.27, half_y, half_z);
        parts.push(detail_plate(
            PartKey::indexed(key, i as u16),
            SubmeshKind::Hull,
            MaterialRole::RolledArmor,
            solid::chamfered_box(center, half, 0.035),
        ));
        // The flat external fuel tanks carry the references' pressed X-stiffened lids: two raised
        // diagonal ribs across the top face.
        if key == "fuel_tank" {
            parts.push(tank_lid_ribs(i as u16, center, half));
        }
    }
    parts
}

/// The pressed X on one fuel-tank lid: two thin raised ribs along the top-face diagonals.
fn tank_lid_ribs(instance: u16, center: Vec3, half: Vec3) -> VehiclePart {
    let top = center.y + half.y + 0.004;
    let (dx, dz) = (half.x - 0.07, half.z - 0.07);
    let a = detail::weld_bead(
        &[
            Vec3::new(center.x - dx, top, center.z - dz),
            Vec3::new(center.x + dx, top, center.z + dz),
        ],
        0.014,
    );
    let b = detail::weld_bead(
        &[
            Vec3::new(center.x - dx, top, center.z + dz),
            Vec3::new(center.x + dx, top, center.z - dz),
        ],
        0.014,
    );
    VehiclePart {
        key: PartKey::indexed("tank_lid_ribs", instance),
        submesh: SubmeshKind::Hull,
        material: MaterialRole::RolledArmor,
        smoothing: vehicle_geometry::SmoothingGroup(7),
        shape: PartShape::Mesh(revolve::merge(&[a, b])),
        lod: PartLod::Detail,
        generator: GeneratorKind::Sweep,
    }
}
