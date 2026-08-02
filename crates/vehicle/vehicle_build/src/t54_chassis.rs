//! Chassis articulation detail for the hybrid T-54: the hull-plate weld seams and the rear
//! transmission access covers. All are `PartLod::Detail` visual plates derived from the
//! blueprint's [`VisualDetail`], built through the shared `detail_plate` helper so they carry
//! stable part keys and the `solid` generator provenance.
//!
//! The swing arms are NOT here. They used to be — a static box per station, baked into the hull
//! at rest height — while the running gear ALSO instanced an animated trailing arm at the same
//! station. Two arms per wheel, one of them frozen at rest and the other rotating with live
//! suspension travel: over rough ground they visibly separated. The animated arm is the real
//! mechanism (`vehicle_geometry::swing_arm_unit_mesh`), so the baked copy is gone.

use game_core::VisualDetail;
use vehicle_geometry::{MaterialRole, SubmeshKind};

use crate::part::{PartKey, VehiclePart};
use crate::t54_details::detail_plate;

/// Hull-plate articulation: the glacis-to-roof weld seam and the rear transmission access covers, as
/// visual detail plates. `front_deg` is the glacis armour slope (the single source) used to place the
/// seam on the real plate join.
pub fn t54_hull_plate_parts(v: &VisualDetail, front_deg: f32) -> Vec<VehiclePart> {
    let mut parts = Vec::new();
    for (i, seam) in solid::t54_hull_plate_seams(&v.hull, front_deg).into_iter().enumerate() {
        parts.push(detail_plate(
            PartKey::indexed("hull_plate_seam", i as u16),
            SubmeshKind::Hull,
            MaterialRole::RolledArmor,
            seam,
        ));
    }
    for (i, cover) in solid::t54_transmission_covers(&v.deck).into_iter().enumerate() {
        parts.push(detail_plate(
            PartKey::indexed("transmission_cover", i as u16),
            SubmeshKind::Hull,
            MaterialRole::RolledArmor,
            cover,
        ));
    }
    parts
}
