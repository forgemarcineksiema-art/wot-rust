//! Soviet low-hull mediums and the test-only prototype.
//!
//! Family signature: a low, flat hull with a raked glacis, six small road wheels per side, and a
//! rounded cast turret carried low on the deck. The 100 mm guns are slim and carry a mid-barrel
//! bore evacuator rather than a muzzle brake. The prototype medium is the odd one out — a plain
//! welded box turret — so it stays visually distinct from the cast-turret pair.
//!
//! Hull width and length derive from the hitbox as explicit fractions; deck height and gun axis
//! come from the authoritative mount frames in [`game_core`].

use game_core::{HitboxProfile, MountFrames, VehicleKind};
use glam::Vec3;

use super::{
    GunPlan, HullPlan, RunningGear, SG_CAST, add_cupola, add_mantlet_socket, add_running_gear,
    add_turret_ring, assemble, build_gun, cast_dome_turret, hull_body, shade_hull,
};
use crate::{BakedVehicle, MaterialRole, MeshBuilder};

pub(crate) fn t55a(hitbox: &HitboxProfile, mounts: &MountFrames) -> BakedVehicle {
    let hull = shade_hull(
        add_running_gear(
            hull_body(
                &HullPlan {
                    half_len: hitbox.half_length_m * 0.953,
                    belly_y: 0.30,
                    deck_y: mounts.turret_ring.translation.y,
                    glacis_run: 0.78,
                    nose_rise: 0.10,
                    half_width: hitbox.half_width_m * 0.76,
                },
                MaterialRole::RolledArmor,
            ),
            &SOVIET_GEAR,
        )
        .build(),
    );

    let mantlet = Some((0.27, 0.85, 1.18));
    let turret = add_mantlet_socket(
        add_turret_ring(
            add_cupola(
                cast_dome_turret(0.05, 0.95, 0.30, mounts.turret_ring.translation.y, 2.16, 16),
                -0.30,
                -0.12,
                2.04,
                0.17,
                0.18,
                true,
            ),
            mounts.turret_ring.translation.z,
            mounts.turret_ring.translation.y,
            0.78,
            0.12,
            16,
        ),
        mounts.gun_trunnion.translation.y,
        mantlet,
        12,
    )
    .build();

    let gun = build_gun(&GunPlan {
        axis_y: mounts.gun_trunnion.translation.y,
        breech_z: mounts.gun_trunnion.translation.z - 0.15,
        muzzle_z: mounts.muzzle.translation.z,
        radius: 0.092,
        segments: 12,
        mantlet,
        // Mid-barrel evacuator, slightly forward of centre — the T-55A signature.
        evacuator: Some((0.57, 0.135)),
        muzzle_brake: None,
    });

    assemble(VehicleKind::T55A, hull, turret, gun, *mounts)
}

pub(crate) fn t54_1951(hitbox: &HitboxProfile, mounts: &MountFrames) -> BakedVehicle {
    let hull = shade_hull(
        add_running_gear(
            hull_body(
                &HullPlan {
                    half_len: hitbox.half_length_m * 0.952,
                    belly_y: 0.30,
                    deck_y: mounts.turret_ring.translation.y,
                    glacis_run: 0.70,
                    nose_rise: 0.08,
                    half_width: hitbox.half_width_m * 0.766,
                },
                MaterialRole::RolledArmor,
            ),
            &SOVIET_GEAR,
        )
        .build(),
    );

    let mantlet = Some((0.29, 0.88, 1.22));
    let turret = add_mantlet_socket(
        add_turret_ring(
            add_cupola(
                cast_dome_turret(0.0, 1.00, 0.34, mounts.turret_ring.translation.y, 2.18, 16),
                -0.34,
                -0.10,
                2.05,
                0.18,
                0.18,
                true,
            ),
            mounts.turret_ring.translation.z,
            mounts.turret_ring.translation.y,
            0.82,
            0.12,
            16,
        ),
        mounts.gun_trunnion.translation.y,
        mantlet,
        12,
    )
    .build();

    let gun = build_gun(&GunPlan {
        axis_y: mounts.gun_trunnion.translation.y,
        breech_z: mounts.gun_trunnion.translation.z - 0.18,
        muzzle_z: mounts.muzzle.translation.z,
        radius: 0.092,
        segments: 12,
        mantlet,
        // Evacuator close to the muzzle — keeps the T-54 readable next to the mid-barrel T-55A.
        evacuator: Some((0.79, 0.135)),
        muzzle_brake: None,
    });

    assemble(VehicleKind::T54_1951, hull, turret, gun, *mounts)
}

pub(crate) fn prototype_medium(hitbox: &HitboxProfile, mounts: &MountFrames) -> BakedVehicle {
    let hull = shade_hull(
        add_running_gear(
            hull_body(
                &HullPlan {
                    half_len: hitbox.half_length_m * 0.953,
                    belly_y: 0.30,
                    deck_y: mounts.turret_ring.translation.y,
                    glacis_run: 0.55,
                    nose_rise: 0.0,
                    half_width: hitbox.half_width_m * 0.765,
                },
                MaterialRole::RolledArmor,
            ),
            &PROTOTYPE_GEAR,
        )
        .build(),
    );

    let mantlet = Some((0.26, 0.85, 1.10));
    let turret = add_mantlet_socket(
        add_turret_ring(
            MeshBuilder::new().chamfered_prism(
                Vec3::new(0.0, 1.74, 0.0),
                Vec3::new(0.82, 0.46, 1.00),
                0.14,
                MaterialRole::CastArmor,
                SG_CAST,
            ),
            mounts.turret_ring.translation.z,
            mounts.turret_ring.translation.y,
            0.74,
            0.12,
            14,
        ),
        mounts.gun_trunnion.translation.y,
        mantlet,
        12,
    )
    .build();

    let gun = build_gun(&GunPlan {
        axis_y: mounts.gun_trunnion.translation.y,
        breech_z: mounts.gun_trunnion.translation.z - 0.10,
        muzzle_z: mounts.muzzle.translation.z,
        radius: 0.10,
        segments: 12,
        mantlet,
        evacuator: None,
        muzzle_brake: None,
    });

    assemble(VehicleKind::PrototypeMedium, hull, turret, gun, *mounts)
}

/// Six small road wheels per side on a narrow track run — the shared Soviet medium chassis.
const SOVIET_GEAR: RunningGear = RunningGear {
    track_half: Vec3::new(0.24, 0.43, 3.05),
    track_center_x: 1.51,
    track_center_y: 0.34,
    track_center_z: 0.0,
    wheel_radius: 0.32,
    wheel_count: 6,
    wheel_y: 0.34,
    wheel_first_z: -2.25,
    wheel_last_z: 2.25,
    wheel_inner_x: 1.66,
    wheel_outer_x: 1.74,
    wheel_segments: 12,
};

const PROTOTYPE_GEAR: RunningGear = RunningGear {
    track_half: Vec3::new(0.23, 0.42, 3.05),
    track_center_x: 1.47,
    track_center_y: 0.34,
    track_center_z: 0.0,
    wheel_radius: 0.31,
    wheel_count: 6,
    wheel_y: 0.34,
    wheel_first_z: -2.25,
    wheel_last_z: 2.25,
    wheel_inner_x: 1.62,
    wheel_outer_x: 1.69,
    wheel_segments: 12,
};
