//! The loader-side DShK-M anti-aircraft machine gun on its hatch-ring mount.
//!
//! At its FIGHTING height. The first ring-mounted build (PR-26b) laid the gun nearly flat on the
//! roof so the whole vehicle stayed inside its collision box — a recorded compromise, now
//! withdrawn: the AA gun stands on a real pintle with a cradle, an elevation arc, spade grips and
//! a sight, and it protrudes above the hitbox apex **by the same documented exception the main
//! gun holds** (2.6 m of barrel already reach past the box; a machine gun on a ring is the same
//! class of thin, honest protrusion).
//!
//! Documented dimensions: the DShK is 1625 mm long over grips-to-muzzle with a 1070 mm barrel —
//! and its own calibre, 12.7 mm, not the tank gun's (that inheritance bug is locked out by
//! `the_dshk_has_its_own_calibre`).

use game_core::{CompleteVisual, GunVisual};
use glam::Vec3;
use vehicle_geometry::{MaterialRole, SmoothingGroup, SubmeshKind};

use crate::part::{GeneratorKind, PartKey, PartLod, PartShape, VehiclePart};
use crate::t54_details::detail_plate;

/// Every DShK part, all at the `Detail` tier, riding the turret so the gun traverses.
pub(crate) fn t54_dshk_parts(v: CompleteVisual<'_>) -> Vec<VehiclePart> {
    let d = &v.detail;
    let f = &v.fittings;
    let mut parts = Vec::new();

    // The RING. A DShK on a T-54 turns on the loader's hatch ring — the loader stands in his own
    // hatch and swings the gun round himself. Welded to the ROOF round the opening, not to the
    // lid: the lid opens, the ring stays. It straddles the roof plane — welded steel overlaps
    // the plate it is welded to, so its base bites into the casting and its collar stands proud.
    let ring_center =
        Vec3::new(f.loader_hatch_center.x, f.loader_hatch_center.y, f.loader_hatch_center.z);
    let ring_radius = f.loader_hatch_radius * 1.24;
    let ring_base_y = ring_center.y + f.loader_hatch_half_height - 0.055;
    parts.push(VehiclePart {
        key: PartKey::new("dshk_ring"),
        submesh: SubmeshKind::Turret,
        material: MaterialRole::TrackMetal,
        smoothing: SmoothingGroup(3),
        shape: PartShape::Mesh(detail::coaming(
            Vec3::new(ring_center.x, ring_base_y, ring_center.z),
            Vec3::Y,
            ring_radius,
            0.05,
            0.035,
            MaterialRole::TrackMetal,
            20,
        )),
        lod: PartLod::Detail,
        generator: GeneratorKind::Revolve,
    });

    // The pintle POST, on the ring's loader-side quarter: a real column, tall enough that the
    // gunner behind it is standing in the hatch, not lying on the roof.
    let pintle_base = Vec3::new(
        ring_center.x + ring_radius * 0.72,
        ring_base_y + 0.045,
        ring_center.z - ring_radius * 0.30,
    );
    const POST_H: f32 = 0.185;
    let trunnion = pintle_base + Vec3::new(0.0, POST_H + 0.035, 0.0);
    parts.push(VehiclePart {
        key: PartKey::new("dshk_mount"),
        submesh: SubmeshKind::Turret,
        material: MaterialRole::TrackMetal,
        smoothing: SmoothingGroup(3),
        shape: PartShape::Mesh(revolve::translate(
            &revolve::revolve(
                Vec3::Y,
                &[
                    (0.0, 0.0),
                    (0.0, 0.046),
                    (0.03, 0.046),
                    (0.04, 0.030),
                    (POST_H, 0.030),
                    (POST_H, 0.0),
                ],
                12,
                MaterialRole::TrackMetal,
                SmoothingGroup(3),
            ),
            pintle_base,
        )),
        lod: PartLod::Detail,
        generator: GeneratorKind::Revolve,
    });

    // The CRADLE: a fork of two plates either side of the receiver, with the trunnion pin
    // through them — the joint the barrel group pitches about.
    for (i, side) in [-1.0_f32, 1.0].into_iter().enumerate() {
        parts.push(detail_plate(
            PartKey::indexed("dshk_cradle", i as u16),
            SubmeshKind::Turret,
            MaterialRole::TrackMetal,
            solid::chamfered_box(
                trunnion + Vec3::new(side * 0.062, 0.0, 0.0),
                Vec3::new(0.009, 0.052, 0.055),
                0.006,
            ),
        ));
    }
    parts.push(VehiclePart {
        key: PartKey::new("dshk_trunnion_pin"),
        submesh: SubmeshKind::Turret,
        material: MaterialRole::BarrelSteel,
        smoothing: SmoothingGroup(3),
        shape: PartShape::Mesh(revolve::translate(
            &revolve::revolve(
                Vec3::X,
                &[(-0.075, 0.0), (-0.075, 0.013), (0.075, 0.013), (0.075, 0.0)],
                10,
                MaterialRole::BarrelSteel,
                SmoothingGroup(3),
            ),
            trunnion,
        )),
        lod: PartLod::Detail,
        generator: GeneratorKind::Revolve,
    });

    // The ELEVATION ARC: the quadrant the cradle clamps against, hanging inboard of the pintle
    // in the pitch plane.
    let arc: Vec<Vec3> = (0..=8)
        .map(|k| {
            let a = (-25.0 + 75.0 * k as f32 / 8.0).to_radians();
            trunnion + Vec3::new(-0.075, a.sin() * 0.145, -a.cos() * 0.145)
        })
        .collect();
    parts.push(VehiclePart {
        key: PartKey::new("dshk_elevation_arc"),
        submesh: SubmeshKind::Turret,
        material: MaterialRole::BarrelSteel,
        smoothing: SmoothingGroup::hard_edges(),
        shape: PartShape::Mesh(detail::handle_rail(&arc, 0.011)),
        lod: PartLod::Detail,
        generator: GeneratorKind::Sweep,
    });

    // The RECEIVER on the cradle, at the documented proportions: 1625 mm over grips-to-muzzle
    // with a 1070 mm barrel leaves ~0.55 m of receiver and grips.
    let receiver = trunnion + Vec3::new(0.0, 0.028, 0.055);
    parts.push(detail_plate(
        PartKey::new("dshk_receiver"),
        SubmeshKind::Turret,
        MaterialRole::TrackMetal,
        solid::chamfered_box(receiver, Vec3::new(0.052, 0.046, 0.215), 0.012),
    ));
    parts.push(detail_plate(
        PartKey::new("dshk_ammo_box"),
        SubmeshKind::Turret,
        MaterialRole::TrackMetal,
        // The can HANGS on the receiver's left wall.
        solid::chamfered_box(
            receiver + Vec3::new(-0.083, -0.008, -0.04),
            Vec3::new(0.030, 0.038, 0.085),
            0.010,
        ),
    ));

    // SPADE GRIPS, angled down and back where the gunner's hands go: the rear of the documented
    // envelope. A gun with no grips is a prop.
    for (i, side) in [-1.0_f32, 1.0].into_iter().enumerate() {
        let root = receiver + Vec3::new(side * 0.030, -0.010, -0.215);
        parts.push(VehiclePart {
            key: PartKey::indexed("dshk_grip", i as u16),
            submesh: SubmeshKind::Turret,
            material: MaterialRole::BarrelSteel,
            smoothing: SmoothingGroup::hard_edges(),
            shape: PartShape::Mesh(detail::handle_rail(
                &[root, root + Vec3::new(side * 0.035, -0.075, -0.085)],
                0.011,
            )),
            lod: PartLod::Detail,
            generator: GeneratorKind::Sweep,
        });
    }

    // The SIGHT: post and ring above the receiver's front — the AA pattern.
    let sight_base = receiver + Vec3::new(0.0, 0.046, 0.145);
    parts.push(VehiclePart {
        key: PartKey::new("dshk_sight"),
        submesh: SubmeshKind::Turret,
        material: MaterialRole::BarrelSteel,
        smoothing: SmoothingGroup(3),
        shape: PartShape::Mesh(
            revolve::merge(&[
                revolve::translate(
                    &revolve::revolve(
                        Vec3::Y,
                        &[(0.0, 0.0), (0.0, 0.006), (0.05, 0.006), (0.05, 0.0)],
                        8,
                        MaterialRole::BarrelSteel,
                        SmoothingGroup(3),
                    ),
                    sight_base,
                ),
                detail::coaming(
                    sight_base + Vec3::new(0.0, 0.068, 0.0),
                    Vec3::Z,
                    0.022,
                    0.008,
                    0.006,
                    MaterialRole::BarrelSteel,
                    10,
                ),
            ])
            .weld_and_smooth(),
        ),
        lod: PartLod::Detail,
        generator: GeneratorKind::Revolve,
    });

    // The BARREL: the documented 1070 mm tube, 12.7 mm bore of its own.
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
        ..*v.gun
    };
    let dshk_breech = receiver + Vec3::Z * 0.20;
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
