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
    let f = &v.fittings;
    let mut parts = Vec::new();
    // Loader-side DShK: a pedestal rooted deep into the curved dome, a receiver box with its ammo
    // can on the left, and a slim stepped barrel with a muzzle collar. A real turret part: it
    // traverses with the vehicle.
    // The RING. A DShK on a T-54 turns on the loader's hatch ring — that is the whole point of
    // the mount: the loader stands in his own hatch and swings the gun round it. Ours stood on a
    // pedestal BESIDE the hatch, which is a gun the loader cannot reach from inside the tank.
    let ring_center =
        Vec3::new(f.loader_hatch_center.x, f.loader_hatch_center.y, f.loader_hatch_center.z);
    let ring_radius = f.loader_hatch_radius * 1.24;
    parts.push(VehiclePart {
        key: PartKey::new("dshk_ring"),
        submesh: SubmeshKind::Turret,
        material: MaterialRole::TrackMetal,
        smoothing: SmoothingGroup(3),
        // Welded to the ROOF, round the opening — not to the lid. The lid opens; the ring stays,
        // which is what lets the loader swing the gun with his hatch up.
        //
        // The height is the ROOF's, and it has to be read off the hatch rather than off the
        // hatch's centre: the loader's hatch is rooted DEEP in the casting (its drum starts at
        // ~2.04) so that its cover lands on the 2.40 m roof. Seating the ring at the drum's base
        // buries it in the dome; seating it on top of the cover stands the gun 0.24 m above the
        // turret it is bolted to. It straddles the roof plane — welded steel overlaps the plate
        // it is welded to, so its base bites into the casting and its collar stands proud.
        shape: PartShape::Mesh(detail::coaming(
            Vec3::new(
                ring_center.x,
                ring_center.y + f.loader_hatch_half_height - 0.055,
                ring_center.z,
            ),
            Vec3::Y,
            ring_radius,
            0.05,
            0.035,
            MaterialRole::TrackMetal,
        )),
        lod: PartLod::Detail,
        generator: GeneratorKind::Revolve,
    });
    // The pintle rides the ring, on its loader-side quarter.
    // The gun sits LOW on its ring. A real DShK stands well above a T-54's roof; this vehicle's
    // collision box does not, and the doctrine is that the box is the footprint — so the mount is
    // modelled to fit inside it, exactly as it was before it moved onto the ring.
    let pintle = Vec3::new(
        ring_center.x + ring_radius * 0.72,
        ring_center.y + f.loader_hatch_half_height - 0.005,
        ring_center.z - ring_radius * 0.30,
    );
    parts.push(VehiclePart {
        key: PartKey::new("dshk_mount"),
        submesh: SubmeshKind::Turret,
        material: MaterialRole::TrackMetal,
        smoothing: SmoothingGroup::hard_edges(),
        shape: PartShape::Mesh(revolve::drum(
            pintle + Vec3::Y * 0.035,
            0.038,
            0.035,
            10,
            MaterialRole::TrackMetal,
            SmoothingGroup::hard_edges(),
        )),
        lod: PartLod::Detail,
        generator: GeneratorKind::Revolve,
    });
    let receiver = pintle + Vec3::new(0.0, 0.058, 0.08);
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
        // 12.7 mm, so a 6.35 mm bore. This inherited `..v.gun` — the D-10T's 0.050 — which is not
        // "a bore 2.2x too wide", it is a bore WIDER THAN THE TUBE AROUND IT: the profile put the
        // muzzle ring at 0.050 outside a 0.026 barrel, so the front of the gun turned itself
        // inside out. The one number a gun must own is its calibre, and this one was borrowed.
        bore_radius: 0.00635,
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
