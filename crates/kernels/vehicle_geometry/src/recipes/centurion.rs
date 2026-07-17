//! The Centurion Mk 3: blueprint-born, the British line's first vehicle. The character is the
//! SKIRT and the bogie — full-length bazooka plates (built by the shared `blueprint_skirts` on
//! the SAME plane the armor volumes bake as a spaced HEAT screen) hiding three Horstmann bogie
//! pairs — under the fleet's steepest glacis, a cast Mk 3 dome whose bustle is closed by the
//! signature rear stowage bin, and the clean unbraked 20-pounder.

use game_core::{HitboxProfile, MountFrames, VehicleKind};
use glam::Vec3;

use super::soviet::{CastRoof, soviet_cast_turret_for};
use super::{
    GunPlan, SG_HARD, assemble, blueprint_prism_hull, blueprint_running_gear, blueprint_skirts,
    build_gun, shade_hull,
};
use crate::{BakedVehicle, MaterialRole, MeshBuilder};

pub(crate) fn centurion(_hitbox: &HitboxProfile, mounts: &MountFrames) -> BakedVehicle {
    let bp = super::active_blueprint(VehicleKind::Centurion).expect("Centurion has a blueprint");
    let hull = shade_hull(
        blueprint_prism_hull(&bp.hull, bp.armor.hull_side.0)
            .append(&blueprint_running_gear(&bp.track))
            .append(&super::deck_details::centurion_deck(&bp))
            .append(&blueprint_skirts(&bp.hull, &bp.track))
            .build(),
    );

    let t = &bp.turret;
    let mantlet = Some((t.mantlet_radius, t.mantlet_back_z, t.mantlet_front_z));
    // The Mk 3 casting with the bustle stowage bin closing the rear of the turret plan.
    let turret = MeshBuilder::new()
        .chamfered_prism(
            Vec3::new(0.0, t.ring_y + 0.44, t.ring_z - t.plan_half_length + 0.16),
            Vec3::new(0.75, 0.22, 0.15),
            0.04,
            MaterialRole::RolledArmor,
            SG_HARD,
        )
        .append(&soviet_cast_turret_for(t, bp.gun.trunnion_y, mantlet, CastRoof::Centurion, 20))
        .build();

    let gun = build_gun(&GunPlan {
        axis_y: bp.gun.trunnion_y,
        breech_z: bp.gun.trunnion_z - 0.25,
        muzzle_z: bp.gun.muzzle_z,
        radius: bp.gun.barrel_radius,
        segments: bp.gun.segments,
        mantlet,
        evacuator: bp.gun.evacuator,
        muzzle_brake: bp.gun.muzzle_brake,
    });

    assemble(VehicleKind::Centurion, hull, turret, gun, *mounts)
}
