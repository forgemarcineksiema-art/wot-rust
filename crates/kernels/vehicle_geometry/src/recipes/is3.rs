//! The IS-3: the era's Soviet heavy, blueprint-born from day one. The hull is bespoke — the
//! pike bow ("shchuchy nos") is authored face by face on the EXACT planes the armor volumes
//! shoot against (same fold ridge, same slopes, same ±sweep), so the visible bow and the
//! penetration model are one geometry. The dome, running gear, and the braked 122 mm ride the
//! shared blueprint family machinery.

use game_core::{HitboxProfile, MountFrames, VehicleKind};

use super::is3_hull::is3_pike_hull;
use super::soviet::soviet_cast_turret_for;
use super::{
    GunPlan, assemble, blueprint_deck_details, blueprint_running_gear, build_gun, shade_hull,
};
use crate::BakedVehicle;

pub(crate) fn is3(_hitbox: &HitboxProfile, mounts: &MountFrames) -> BakedVehicle {
    let bp =
        game_core::VehicleBlueprint::for_vehicle(VehicleKind::IS3).expect("IS-3 has a blueprint");
    let hull = shade_hull(
        is3_pike_hull(&bp.hull)
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
