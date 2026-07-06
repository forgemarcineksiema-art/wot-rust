//! Fixed-casemate tank destroyer (Jagdtiger).
//!
//! There is no rotating turret: a tall, slab-sided superstructure with a strongly raked front
//! plate is welded to the hull and carries the gun. It is still emitted in the `turret` submesh
//! slot, but the sim holds a casemate's turret yaw at zero, so it never traverses — only the gun
//! elevates. The 128 mm is the longest, fattest barrel in the lineup.
//!
//! Hull width and length derive from the hitbox as explicit fractions; deck height and gun axis
//! come from the authoritative mount frames in [`game_core`].

use game_core::{HitboxProfile, MountFrames, VehicleKind};
use glam::{Vec2, Vec3};

use super::{
    GunPlan, HullPlan, SG_HARD, add_mantlet_socket, assemble, build_gun, hull_body,
    legacy_track_band, shade_hull,
};
use crate::{Axis, BakedVehicle, ExtrudeSpec, MaterialRole, MeshBuilder};

pub(crate) fn jagdtiger(hitbox: &HitboxProfile, mounts: &MountFrames) -> BakedVehicle {
    let hull = shade_hull(
        hull_body(
            &HullPlan {
                half_len: hitbox.half_length_m * 0.963,
                belly_y: 0.34,
                deck_y: mounts.turret_ring.translation.y,
                glacis_run: 0.85,
                nose_rise: 0.05,
                half_width: hitbox.half_width_m * 0.775,
            },
            MaterialRole::RolledArmor,
        )
        .append(&legacy_track_band(VehicleKind::Jagdtiger))
        .build(),
    );

    let mantlet = Some((0.42, 1.66, 2.05));
    let casemate = add_mantlet_socket(
        MeshBuilder::new().extrude(
            Vec3::ZERO,
            ExtrudeSpec {
                section: vec![
                    Vec2::new(-1.95, 1.05),
                    Vec2::new(1.55, 1.05),
                    Vec2::new(1.92, 1.92),
                    Vec2::new(1.45, 2.84),
                    Vec2::new(-1.60, 2.84),
                    Vec2::new(-1.95, 2.05),
                ],
                axis: Axis::X,
                half_depth: 1.48,
                material: MaterialRole::RolledArmor,
                smoothing: SG_HARD,
            },
        ),
        mounts.gun_trunnion.translation.y,
        mantlet,
        14,
    )
    .build();

    let gun = build_gun(&GunPlan {
        axis_y: mounts.gun_trunnion.translation.y,
        breech_z: mounts.gun_trunnion.translation.z - 0.18,
        muzzle_z: mounts.muzzle.translation.z,
        radius: 0.135,
        segments: 14,
        mantlet,
        evacuator: None,
        muzzle_brake: Some(0.23),
    });

    assemble(VehicleKind::Jagdtiger, hull, casemate, gun, *mounts)
}
