//! Chassis articulation detail for the hybrid T-54: the hull-plate weld seams, the rear transmission
//! access covers, and the swing-arm brackets that visibly mount each road wheel to the hull. All are
//! `PartLod::Detail` visual plates derived from the blueprint's [`HybridVisual`], built through the
//! shared `detail_plate` helper so they carry stable part keys and the `solid` generator provenance.

use game_core::{HybridVisual, VehicleKind};
use glam::Vec3;
use vehicle_geometry::{MaterialRole, RunningGearKinematics, SubmeshKind};

use crate::part::{PartKey, VehiclePart};
use crate::t54_details::detail_plate;

/// Hull-plate articulation: the glacis-to-roof weld seam and the rear transmission access covers, as
/// visual detail plates. `front_deg` is the glacis armour slope (the single source) used to place the
/// seam on the real plate join.
pub fn t54_hull_plate_parts(v: &HybridVisual, front_deg: f32) -> Vec<VehiclePart> {
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

/// A visual swing-arm bracket per road wheel, bridging the hull's lower tub side to the wheel hub at
/// axle height. Without it the road wheels read as floating on a bare axle line; this is the link
/// that mounts them to the hull. `lower_half_width` is the hull tub half-width (the pivot side).
///
/// The brackets read the **animated** running-gear kinematics (the same source the rendered road
/// wheels use), so each bracket lands on the axle and Z of an actual wheel instead of a divergent
/// second wheel layout.
pub fn t54_suspension_parts(lower_half_width: f32) -> Vec<VehiclePart> {
    let kin = RunningGearKinematics::for_vehicle(VehicleKind::T54_1951)
        .expect("T-54 has animated running gear");
    let wheel_inner = kin.wheel_x - kin.wheel_half_width;
    let arm_cx = 0.5 * (lower_half_width + wheel_inner);
    let arm_hx = (0.5 * (wheel_inner - lower_half_width)).max(0.04);
    let mut parts = Vec::new();
    let mut arm = 0u16;
    for &z in &kin.wheel_zs {
        for side in [1.0_f32, -1.0] {
            let center = Vec3::new(side * arm_cx, kin.cy, z);
            parts.push(detail_plate(
                PartKey::indexed("swing_arm", arm),
                SubmeshKind::Hull,
                MaterialRole::TrackMetal,
                solid::ConvexSolid::box_at(center, Vec3::new(arm_hx, 0.05, 0.10)),
            ));
            arm += 1;
        }
    }
    parts
}
