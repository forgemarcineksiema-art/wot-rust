//! The IS-3: the era's Soviet heavy, blueprint-born from day one. First pass rides the shared
//! family machinery — hull plates, six-wheel running gear, the flattened cast dome, and the
//! braked 122 mm all read the IS-3 blueprint, so dimensions, dome, wheels, and the gun already
//! read true. The visual pike bow and the heavy's deck furniture are the follow-up detail
//! package (the ARMOR pike is already real — the gameplay volumes carry both bow planes).

use game_core::{HitboxProfile, MountFrames, VehicleKind};

use super::soviet::soviet_cast_turret_for;
use super::{
    GunPlan, assemble, blueprint_deck_details, blueprint_hull, blueprint_running_gear, build_gun,
    shade_hull,
};
use crate::{BakedVehicle, MaterialRole};

pub(crate) fn is3(_hitbox: &HitboxProfile, mounts: &MountFrames) -> BakedVehicle {
    let bp =
        game_core::VehicleBlueprint::for_vehicle(VehicleKind::IS3).expect("IS-3 has a blueprint");
    let hull = shade_hull(
        blueprint_hull(&bp.hull, MaterialRole::RolledArmor)
            .append(&blueprint_running_gear(&bp.track))
            .append(&blueprint_deck_details(&bp.hull))
            .build(),
    );

    let t = &bp.turret;
    let mantlet = Some((t.mantlet_radius, t.mantlet_back_z, t.mantlet_front_z));
    let turret = soviet_cast_turret_for(t, bp.gun.trunnion_y, mantlet, 20);

    let gun = build_gun(&GunPlan {
        axis_y: bp.gun.trunnion_y,
        breech_z: bp.gun.trunnion_z - 0.22,
        muzzle_z: bp.gun.muzzle_z,
        radius: bp.gun.barrel_radius,
        segments: bp.gun.segments,
        mantlet,
        evacuator: bp.gun.evacuator,
        muzzle_brake: bp.gun.muzzle_brake,
    });

    assemble(VehicleKind::IS3, hull, turret, gun, *mounts)
}
