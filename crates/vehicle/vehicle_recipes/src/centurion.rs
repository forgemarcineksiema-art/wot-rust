//! The Centurion Mk 3: blueprint-born, the British line's first vehicle. The character is the
//! SKIRT and the bogie — full-length bazooka plates (built by the shared `blueprint_skirts` on
//! the SAME plane the armor volumes bake as a spaced HEAT screen) hiding three Horstmann bogie
//! pairs — under the fleet's steepest glacis, a cast Mk 3 dome whose bustle is closed by the
//! signature rear stowage bin, and the clean unbraked 20-pounder.

use game_core::{HitboxProfile, MountFrames, VehicleKind};
use glam::Vec3;

use super::soviet::{CastRoof, soviet_cast_turret_for};
use super::{
    GunPlan, SG_HARD, assemble, blueprint_prism_hull, blueprint_skirts, gun_group, shade_hull,
};
use vehicle_geometry::{BakedVehicle, GeometryMesh, MaterialRole, MeshBuilder};

pub(crate) fn centurion(hitbox: &HitboxProfile, mounts: &MountFrames) -> BakedVehicle {
    let pieces = centurion_pieces(hitbox, mounts, super::deck_details::DeckOmit::default());
    let concat = |pieces: Vec<(&'static str, GeometryMesh)>| {
        revolve::merge(&pieces.into_iter().map(|(_, mesh)| mesh).collect::<Vec<_>>())
    };
    assemble(
        VehicleKind::Centurion,
        concat(pieces.hull),
        concat(pieces.turret),
        concat(pieces.gun),
        pieces.mounts,
    )
}

/// The Centurion as the pieces its recipe is made of (Forge 2.0 K3, 2026-09-06): the prism
/// hull, the British deck, the bazooka plates, the Mk 3 dome with its bustle bin, and the gun
/// group. `centurion` is these concatenated in this order and welded; each piece stays out when
/// the part library builds its class (`DeckOmit`).
pub(crate) fn centurion_pieces(
    _hitbox: &HitboxProfile,
    mounts: &MountFrames,
    omit: super::deck_details::DeckOmit,
) -> super::RecipePieces {
    let bp = super::active_blueprint(VehicleKind::Centurion).expect("Centurion has a blueprint");
    let mut hull = Vec::with_capacity(3);
    if !omit.slab {
        hull.push((
            "recipe_hull_prism",
            shade_hull(blueprint_prism_hull(&bp.hull, bp.armor.hull_side.0).build()),
        ));
    }
    if !omit.deck {
        hull.push(("recipe_hull_deck", shade_hull(super::deck_details::centurion_deck(&bp, omit))));
    }
    if !omit.skirts {
        hull.push(("recipe_hull_skirts", shade_hull(blueprint_skirts(&bp.hull, &bp.track))));
    }

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

    let gun = gun_group(
        VehicleKind::Centurion,
        &GunPlan {
            axis_y: bp.gun.trunnion_y,
            breech_z: bp.gun.trunnion_z - 0.25,
            muzzle_z: bp.gun.muzzle_z,
            radius: bp.gun.barrel_radius,
            segments: bp.gun.segments,
            mantlet,
            evacuator: bp.gun.evacuator,
            muzzle_brake: bp.gun.muzzle_brake,
        },
    );

    super::RecipePieces {
        hull,
        turret: if omit.turret { Vec::new() } else { vec![("recipe_turret", turret)] },
        gun: if omit.gun { Vec::new() } else { vec![("recipe_gun", gun)] },
        mounts: *mounts,
    }
}
