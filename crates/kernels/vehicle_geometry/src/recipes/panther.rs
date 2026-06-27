//! Panther II — the most extreme glacis in the lineup and a compact, heavily rounded turret with
//! a fat Saukopf mantlet, riding large overlapping wheels.
//!
//! Hull width and length derive from the hitbox as explicit fractions; deck height and gun axis
//! come from the authoritative mount frames in [`game_core`].

use game_core::{HitboxProfile, MountFrames, VehicleKind};
use glam::Vec3;

use super::{
    GunPlan, HullPlan, RunningGear, SG_CAST, add_cupola, add_mantlet_socket, add_running_gear,
    add_turret_ring, assemble, build_gun, hull_body, shade_hull,
};
use crate::{BakedVehicle, MaterialRole, MeshBuilder};

pub(crate) fn panther_ii(hitbox: &HitboxProfile, mounts: &MountFrames) -> BakedVehicle {
    let hull = shade_hull(
        add_running_gear(
            hull_body(
                &HullPlan {
                    half_len: hitbox.half_length_m * 0.959,
                    belly_y: 0.34,
                    deck_y: mounts.turret_ring.translation.y,
                    glacis_run: 1.18,
                    nose_rise: 0.05,
                    half_width: hitbox.half_width_m * 0.757,
                },
                MaterialRole::RolledArmor,
            ),
            &PANTHER_GEAR,
        )
        .build(),
    );

    let mantlet = Some((0.34, 1.05, 1.40));
    let turret = add_mantlet_socket(
        add_turret_ring(
            add_cupola(
                MeshBuilder::new().chamfered_prism(
                    Vec3::new(0.0, 2.02, -0.05),
                    Vec3::new(0.86, 0.56, 1.24),
                    0.34,
                    MaterialRole::CastArmor,
                    SG_CAST,
                ),
                -0.46,
                -0.55,
                2.54,
                0.20,
                0.20,
                true,
            ),
            mounts.turret_ring.translation.z,
            mounts.turret_ring.translation.y,
            0.78,
            0.13,
            14,
        ),
        mounts.gun_trunnion.translation.y,
        mantlet,
        12,
    )
    .build();

    let gun = build_gun(&GunPlan {
        axis_y: mounts.gun_trunnion.translation.y,
        breech_z: mounts.gun_trunnion.translation.z - 0.20,
        muzzle_z: mounts.muzzle.translation.z,
        radius: 0.085,
        segments: 12,
        mantlet,
        evacuator: None,
        muzzle_brake: Some(0.15),
    });

    assemble(VehicleKind::PantherII, hull, turret, gun, *mounts)
}

const PANTHER_GEAR: RunningGear = RunningGear {
    track_half: Vec3::new(0.25, 0.52, 3.52),
    track_center_x: 1.60,
    track_center_y: 0.40,
    track_center_z: 0.0,
    wheel_radius: 0.46,
    wheel_count: 8,
    wheel_y: 0.44,
    wheel_first_z: -3.00,
    wheel_last_z: 3.00,
    wheel_inner_x: 1.38,
    wheel_outer_x: 1.82,
    wheel_segments: 10,
};
