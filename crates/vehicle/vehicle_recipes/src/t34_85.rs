//! The T-34-85: the first vehicle AUTHORED THROUGH THE STUDIO LOOP — its shape lives entirely
//! in `blueprints/t34_85.blueprint.ron`, and this recipe is pure delegation to the shared
//! blueprint components. The character is SLOPE: the 60-degree glacis and the raked sides
//! overhanging the tracks (the same 45 mm plate everywhere, angled), five big bare Christie
//! wheels with the signature open gap behind the first station, and the wide low three-man
//! cast dome of the 85 refit sitting forward on the hull.

use game_core::{HitboxProfile, MountFrames, VehicleKind};

use super::soviet::{CastRoof, soviet_cast_turret_for};
use super::{GunPlan, assemble, blueprint_prism_hull, gun_group, shade_hull};
use vehicle_geometry::{BakedVehicle, GeometryMesh};

pub(crate) fn t34_85(hitbox: &HitboxProfile, mounts: &MountFrames) -> BakedVehicle {
    let pieces = t34_85_pieces(hitbox, mounts, super::deck_details::DeckOmit::default());
    let concat = |pieces: Vec<(&'static str, GeometryMesh)>| {
        revolve::merge(&pieces.into_iter().map(|(_, mesh)| mesh).collect::<Vec<_>>())
    };
    assemble(
        VehicleKind::T34_85,
        concat(pieces.hull),
        concat(pieces.turret),
        concat(pieces.gun),
        pieces.mounts,
    )
}

/// The T-34-85 as the pieces its recipe is made of (Forge 2.0 K3, 2026-09-06): the leaned prism
/// hull, the Soviet deck with the glacis furniture, the cast dome with its roof, and the gun
/// group. `t34_85` is these concatenated in this order and welded; each piece stays out when the
/// part library builds its class (`DeckOmit`).
pub(crate) fn t34_85_pieces(
    _hitbox: &HitboxProfile,
    mounts: &MountFrames,
    omit: super::deck_details::DeckOmit,
) -> super::RecipePieces {
    let bp = super::active_blueprint(VehicleKind::T34_85).expect("T-34-85 has a blueprint");
    let mut hull = Vec::with_capacity(2);
    if !omit.slab {
        hull.push((
            "recipe_hull_prism",
            shade_hull(blueprint_prism_hull(&bp.hull, bp.armor.hull_side.0).build()),
        ));
    }
    if !omit.deck {
        hull.push(("recipe_hull_deck", shade_hull(super::deck_details::t34_85_deck(&bp, omit))));
    }

    let t = &bp.turret;
    let mantlet = Some((t.mantlet_radius, t.mantlet_back_z, t.mantlet_front_z));
    let turret = soviet_cast_turret_for(t, bp.gun.trunnion_y, mantlet, CastRoof::T3485, 20);

    let gun = gun_group(
        VehicleKind::T34_85,
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
