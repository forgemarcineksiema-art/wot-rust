//! Soviet low-hull mediums and the test-only prototype.
//!
//! Family signature: a low, flat hull with a raked glacis and a rounded cast turret carried low on
//! the deck. The 100 mm guns are slim and carry a mid-barrel bore evacuator rather than a muzzle
//! brake. The cast-turret T-54 rides the historical five road wheels per side; the
//! prototype medium is the odd one out — six wheels and a plain welded box turret — so it stays
//! visually distinct.
//!
//! The T-54 is blueprint-driven (hull, running gear, turret, and gun all read one
//! [`game_core::VehicleBlueprint`]); the prototype still derives its hull from the hitbox fractions
//! and mount frames.

use game_core::{HitboxProfile, MountFrames, TurretShape, VehicleKind};
use glam::Vec3;

use super::{
    GunPlan, HullPlan, RunningGear, SG_CAST, add_broad_mantlet_socket, add_cupola,
    add_mantlet_socket, add_running_gear, add_t54_mantlet_socket, add_turret_ring, assemble,
    blueprint_running_gear, blueprint_skirts, build_gun, build_gun_with_mantlet_scale,
    cast_turret_shell, hull_body, shade_hull, t54_hull, t54_turret_front,
};
use crate::{BakedVehicle, GeometryMesh, MaterialRole, MeshBuilder};

/// The shared Soviet cast turret: a low wide dome carrying a commander's cupola drum, a loader's
/// hatch, the ring collar, and the mantlet seat. One helper keeps the family (T-54, IS-3)
/// reading consistently and keeps the recipes short.
fn soviet_cast_turret(
    t: &TurretShape,
    trunnion_y: f32,
    mantlet: Option<(f32, f32, f32)>,
    t54_socket: bool,
    shell_segments: usize,
) -> GeometryMesh {
    let mut shell = cast_turret_shell(
        t.ring_z,
        t.base_radius,
        t.plan_half_length,
        t.roof_radius,
        t.ring_y,
        t.roof_y,
        shell_segments,
    );
    if t54_socket {
        shell = shell.append(&t54_turret_front(t, trunnion_y, mantlet));
    }
    // Commander's cupola: a real drum standing proud of the roof (not a pimple), capped so its
    // lid stays inside the hitbox top (the 2.40 m silhouette apex).
    let with_cupola =
        add_cupola(shell, t.cupola_x, t.cupola_z, t.roof_y - 0.08, t.cupola_radius, 0.21, true);
    // Loader's hatch: a low flat round hatch on the opposite side of the roof.
    let with_hatch = add_cupola(
        with_cupola,
        -t.cupola_x * 0.85,
        t.cupola_z + 0.46,
        t.roof_y - 0.05,
        t.cupola_radius * 0.78,
        0.09,
        false,
    );
    let with_ring = add_turret_ring(with_hatch, t.ring_z, t.ring_y, t.ring_radius, 0.12, 16);
    if t54_socket {
        add_t54_mantlet_socket(with_ring, trunnion_y, mantlet, 12)
    } else {
        add_broad_mantlet_socket(with_ring, trunnion_y, mantlet, 12)
    }
    .build()
}

/// The shared Soviet cast-turret builder, exposed to the IS-3 recipe (`recipes::is3`) — the
/// heavy rides the same family machinery with its own blueprint numbers.
pub(super) fn soviet_cast_turret_for(
    t: &TurretShape,
    trunnion_y: f32,
    mantlet: Option<(f32, f32, f32)>,
    shell_segments: usize,
) -> GeometryMesh {
    soviet_cast_turret(t, trunnion_y, mantlet, false, shell_segments)
}

pub(crate) fn t54_1951(_hitbox: &HitboxProfile, mounts: &MountFrames) -> BakedVehicle {
    // Migrated to the blueprint: hull, tracks, turret, and gun all read one shape source, so the
    // visible glacis is the armour angle and the running gear wraps the wheels.
    let bp = super::active_blueprint(VehicleKind::T54_1951).expect("T-54 has a blueprint");
    let hull = shade_hull(
        t54_hull(&bp.hull, &bp.track)
            .append(&blueprint_running_gear(&bp.track))
            .append(&super::deck_details::t54_family_deck(&bp))
            .append(&blueprint_skirts(&bp.hull, &bp.track))
            .build(),
    );

    let t = &bp.turret;
    let mantlet = Some((t.mantlet_radius, t.mantlet_back_z, t.mantlet_front_z));
    let turret = soviet_cast_turret(t, bp.gun.trunnion_y, mantlet, true, 28);

    let gun = build_gun_with_mantlet_scale(
        &GunPlan {
            axis_y: bp.gun.trunnion_y,
            breech_z: bp.gun.trunnion_z - 0.18,
            muzzle_z: bp.gun.muzzle_z,
            radius: bp.gun.barrel_radius,
            segments: bp.gun.segments,
            mantlet,
            evacuator: bp.gun.evacuator,
            muzzle_brake: bp.gun.muzzle_brake,
        },
        1.55,
        0.62,
    );

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
