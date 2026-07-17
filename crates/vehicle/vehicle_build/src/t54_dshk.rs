//! The loader-side DShK anti-aircraft machine gun on the turret roof: a pedestal rooted into the
//! cast dome, the receiver with its ammo can, and a slim stepped barrel. Split from `t54_details`
//! to keep each module within the reviewability budget.

use game_core::{GunVisual, HybridVisual};
use glam::Vec3;
use vehicle_geometry::{MaterialRole, SmoothingGroup, SubmeshKind};

use crate::part::{GeneratorKind, PartKey, PartLod, PartShape, VehiclePart};
use crate::t54_details::detail_plate;

/// Every DShK part, all at the `Detail` tier, riding the turret so the gun traverses.
pub(crate) fn t54_dshk_parts(v: &HybridVisual) -> Vec<VehiclePart> {
    let d = &v.detail;
    let mut parts = Vec::new();
    // Loader-side DShK: a pedestal rooted deep into the curved dome, a receiver box with its ammo
    // can on the left, and a slim stepped barrel with a muzzle collar. A real turret part: it
    // traverses with the vehicle.
    parts.push(VehiclePart {
        key: PartKey::new("dshk_mount"),
        submesh: SubmeshKind::Turret,
        material: MaterialRole::TrackMetal,
        smoothing: SmoothingGroup::hard_edges(),
        shape: PartShape::Mesh(revolve::drum(
            d.dshk_mount_center,
            0.065,
            0.16,
            10,
            MaterialRole::TrackMetal,
            SmoothingGroup::hard_edges(),
        )),
        lod: PartLod::Detail,
        generator: GeneratorKind::Revolve,
    });
    let receiver = d.dshk_mount_center + Vec3::new(0.0, 0.175, 0.08);
    parts.push(detail_plate(
        PartKey::new("dshk_receiver"),
        SubmeshKind::Turret,
        MaterialRole::TrackMetal,
        solid::chamfered_box(receiver, Vec3::new(0.04, 0.035, 0.15), 0.012),
    ));
    parts.push(detail_plate(
        PartKey::new("dshk_ammo_box"),
        SubmeshKind::Turret,
        MaterialRole::TrackMetal,
        // The can HANGS on the receiver's left wall (audit #10: it used to float 7 mm off
        // it in plain air over the dome cheek).
        solid::chamfered_box(
            receiver + Vec3::new(-0.071, -0.005, -0.02),
            Vec3::new(0.032, 0.03, 0.07),
            0.010,
        ),
    ));
    let dshk = GunVisual {
        barrel_radius: 0.020,
        muzzle_radius: 0.026,
        muzzle_taper: 0.04,
        barrel_segments: 10,
        ..v.gun
    };
    let dshk_breech = receiver + Vec3::Z * 0.13;
    parts.push(VehiclePart {
        key: PartKey::new("dshk_barrel"),
        submesh: SubmeshKind::Turret,
        material: MaterialRole::BarrelSteel,
        smoothing: SmoothingGroup(4),
        shape: PartShape::Mesh(revolve::gun_barrel_between(
            dshk_breech,
            dshk_breech + Vec3::Z * d.dshk_barrel_length,
            &dshk,
        )),
        lod: PartLod::Detail,
        generator: GeneratorKind::Revolve,
    });

    parts
}
