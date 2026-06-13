//! Soviet low-hull mediums and the test-only prototype.
//!
//! Family signature: a low, flat hull with a raked glacis and a rounded cast turret carried low on
//! the deck. The 100 mm guns are slim and carry a mid-barrel bore evacuator rather than a muzzle
//! brake. The cast-turret pair (T-54 and T-55A) ride the historical five road wheels per side; the
//! prototype medium is the odd one out — six wheels and a plain welded box turret — so it stays
//! visually distinct from the pair.
//!
//! The T-54 and T-55A are blueprint-driven (hull, running gear, turret, and gun all read one
//! [`game_core::VehicleBlueprint`]); the prototype still derives its hull from the hitbox fractions
//! and mount frames.

use game_core::{HitboxProfile, MountFrames, VehicleKind};
use glam::Vec3;

use super::{
    GunPlan, HullPlan, RunningGear, SG_CAST, add_broad_mantlet_socket, add_cupola,
    add_mantlet_socket, add_running_gear, add_turret_ring, assemble, blueprint_deck_details,
    blueprint_hull, blueprint_running_gear, build_gun, cast_turret_shell, hull_body, shade_hull,
};
use crate::{BakedVehicle, MaterialRole, MeshBuilder};

pub(crate) fn t55a(_hitbox: &HitboxProfile, mounts: &MountFrames) -> BakedVehicle {
    // Migrated to the blueprint, reusing the same machinery as the T-54: hull, wrapped five-wheel
    // running gear, cast turret, and the longer D-10 gun all read one shape source.
    let bp =
        game_core::VehicleBlueprint::for_vehicle(VehicleKind::T55A).expect("T-55A has a blueprint");
    let hull = shade_hull(
        blueprint_hull(&bp.hull, MaterialRole::RolledArmor)
            .append(&blueprint_running_gear(&bp.track))
            .append(&blueprint_deck_details(&bp.hull))
            .build(),
    );

    let t = &bp.turret;
    let mantlet = Some((t.mantlet_radius, t.mantlet_back_z, t.mantlet_front_z));
    let turret = add_broad_mantlet_socket(
        add_turret_ring(
            add_cupola(
                cast_turret_shell(
                    t.ring_z,
                    t.base_radius,
                    t.plan_half_length,
                    t.roof_radius,
                    t.ring_y,
                    t.roof_y,
                    16,
                ),
                t.cupola_x,
                t.cupola_z,
                t.roof_y - 0.13,
                t.cupola_radius,
                0.18,
                true,
            ),
            t.ring_z,
            t.ring_y,
            t.ring_radius,
            0.12,
            16,
        ),
        bp.gun.trunnion_y,
        mantlet,
        12,
    )
    .build();

    let gun = build_gun(&GunPlan {
        axis_y: bp.gun.trunnion_y,
        breech_z: bp.gun.trunnion_z - 0.18,
        muzzle_z: bp.gun.muzzle_z,
        radius: bp.gun.barrel_radius,
        segments: bp.gun.segments,
        mantlet,
        evacuator: bp.gun.evacuator,
        muzzle_brake: bp.gun.muzzle_brake,
    });

    assemble(VehicleKind::T55A, hull, turret, gun, *mounts)
}

pub(crate) fn t54_1951(_hitbox: &HitboxProfile, mounts: &MountFrames) -> BakedVehicle {
    // Migrated to the blueprint: hull, tracks, turret, and gun all read one shape source, so the
    // visible glacis is the armour angle and the running gear wraps the wheels.
    let bp = game_core::VehicleBlueprint::for_vehicle(VehicleKind::T54_1951)
        .expect("T-54 has a blueprint");
    let hull = shade_hull(
        blueprint_hull(&bp.hull, MaterialRole::RolledArmor)
            .append(&blueprint_running_gear(&bp.track))
            .append(&blueprint_deck_details(&bp.hull))
            .build(),
    );

    let t = &bp.turret;
    let mantlet = Some((t.mantlet_radius, t.mantlet_back_z, t.mantlet_front_z));
    let turret = add_broad_mantlet_socket(
        add_turret_ring(
            add_cupola(
                cast_turret_shell(
                    t.ring_z,
                    t.base_radius,
                    t.plan_half_length,
                    t.roof_radius,
                    t.ring_y,
                    t.roof_y,
                    16,
                ),
                t.cupola_x,
                t.cupola_z,
                t.roof_y - 0.13,
                t.cupola_radius,
                0.18,
                true,
            ),
            t.ring_z,
            t.ring_y,
            t.ring_radius,
            0.12,
            16,
        ),
        bp.gun.trunnion_y,
        mantlet,
        12,
    )
    .build();

    let gun = build_gun(&GunPlan {
        axis_y: bp.gun.trunnion_y,
        breech_z: bp.gun.trunnion_z - 0.18,
        muzzle_z: bp.gun.muzzle_z,
        radius: bp.gun.barrel_radius,
        segments: bp.gun.segments,
        mantlet,
        evacuator: bp.gun.evacuator,
        muzzle_brake: bp.gun.muzzle_brake,
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
