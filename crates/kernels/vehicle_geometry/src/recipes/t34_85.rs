//! The T-34-85: the first vehicle AUTHORED THROUGH THE STUDIO LOOP — its shape lives entirely
//! in `blueprints/t34_85.blueprint.ron`, and this recipe is pure delegation to the shared
//! blueprint components. The character is SLOPE: the 60-degree glacis and the raked sides
//! overhanging the tracks (the same 45 mm plate everywhere, angled), five big bare Christie
//! wheels with the signature open gap behind the first station, and the wide low three-man
//! cast dome of the 85 refit sitting forward on the hull.

use game_core::{HitboxProfile, MountFrames, VehicleKind};

use super::soviet::soviet_cast_turret_for;
use super::{
    GunPlan, assemble, blueprint_deck_details, blueprint_prism_hull, blueprint_running_gear,
    build_gun, shade_hull,
};
use crate::BakedVehicle;

pub(crate) fn t34_85(_hitbox: &HitboxProfile, mounts: &MountFrames) -> BakedVehicle {
    let bp = game_core::VehicleBlueprint::for_vehicle(VehicleKind::T34_85)
        .expect("T-34-85 has a blueprint");
    let hull = shade_hull(
        blueprint_prism_hull(&bp.hull, bp.armor.hull_side.0)
            .append(&blueprint_running_gear(&bp.track))
            .append(&blueprint_deck_details(&bp.hull))
            .build(),
    );

    let t = &bp.turret;
    let mantlet = Some((t.mantlet_radius, t.mantlet_back_z, t.mantlet_front_z));
    let turret = soviet_cast_turret_for(t, bp.gun.trunnion_y, mantlet, 20);

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

    assemble(VehicleKind::T34_85, hull, turret, gun, *mounts)
}
