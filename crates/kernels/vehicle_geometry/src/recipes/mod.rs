//! Per-vehicle procedural recipes plus the shared family components they are built from.
//!
//! Each recipe authors its submeshes in a single **local space**: the origin sits on the ground
//! plane under the hull centre, `+Y` is up, and `+Z` is forward (the gun points `+Z`). The hull,
//! turret, and gun all use this same space; the renderer later pivots the turret about the
//! turret-ring frame and the gun about the trunnion frame, so authoring everything in one space
//! keeps the mount maths trivial and the fit tests honest.
//!
//! Shared shapes live in [`chassis`] (hulls, tracks, wheels), [`armament`] (guns), and
//! [`turret_fittings`] (cupolas, mantlet sockets, cast domes); the family modules ([`soviet`],
//! [`german`], [`panther`], [`casemate`]) tune those into distinct vehicles.
//!
//! Recipes receive [`MountFrames`] and [`HitboxProfile`] from [`game_core`] as authoritative
//! inputs — hull dimensions are derived as explicit fractions of the hitbox, and mount points
//! (turret ring, trunnion, muzzle) drive turret and gun placement. This keeps the fit test
//! nearly tautological and prevents drift between the visual geometry, the collision box, and
//! the simulation shell spawn.

use game_core::{HitboxProfile, MountFrames, VehicleKind};

use crate::{BakeError, BakedVehicle, GeometryMesh, SmoothingGroup, Submesh, SubmeshKind};

mod armament;
mod casemate;
mod chassis;
mod chassis_blueprint;
mod german;
mod is3;
mod is3_hull;
mod panther;
mod soviet;
mod t54;
mod turret_fittings;

pub(crate) use armament::{GunPlan, build_gun, build_gun_with_mantlet_scale};
pub(crate) use chassis::{HullPlan, RunningGear, add_running_gear, hull_body, shade_hull};
pub(crate) use chassis_blueprint::{
    blueprint_deck_details, blueprint_hull, blueprint_running_gear,
};
pub(crate) use t54::{t54_hull, t54_turret_front};

/// The static belt band for a legacy-animated vehicle: the same wrapped band the blueprint fleet
/// bakes (top/bottom runs + end wraps), built from the authored [`crate::legacy_tracks`] table.
/// The moving parts — wheels, sprocket, idler, shoe links — are instanced at render time, exactly
/// like the blueprint fleet; the fused wheel/box gear this replaces is gone.
pub(crate) fn legacy_track_band(kind: VehicleKind) -> GeometryMesh {
    let track = crate::legacy_tracks::legacy_track_shape(kind)
        .expect("legacy_track_band is only called for vehicles with an authored legacy track");
    blueprint_running_gear(&track)
}
pub(crate) use turret_fittings::{
    add_broad_mantlet_socket, add_cupola, add_mantlet_socket, add_t54_mantlet_socket,
    add_turret_ring, cast_turret_shell,
};

/// Bake the procedural geometry for `kind`.
///
/// The result is fallible for API stability and forward-compatibility: [`recipe`] matches every
/// current [`VehicleKind`] exhaustively (so adding a wire variant is a compile error until a
/// recipe exists), and the [`BakeError::MissingRecipe`] path stays wired up for that future gap.
pub fn bake_vehicle(kind: VehicleKind) -> Result<BakedVehicle, BakeError> {
    let hitbox = HitboxProfile::for_vehicle(kind);
    let mounts = MountFrames::for_vehicle(kind);
    recipe(kind, &hitbox, &mounts).ok_or(BakeError::MissingRecipe(kind))
}

fn recipe(kind: VehicleKind, hitbox: &HitboxProfile, mounts: &MountFrames) -> Option<BakedVehicle> {
    Some(match kind {
        VehicleKind::PrototypeMedium => soviet::prototype_medium(hitbox, mounts),
        VehicleKind::T54_1951 => soviet::t54_1951(hitbox, mounts),
        VehicleKind::T55A => soviet::t55a(hitbox, mounts),
        VehicleKind::TigerI => german::tiger_i(hitbox, mounts),
        VehicleKind::TigerII => german::tiger_ii(hitbox, mounts),
        VehicleKind::Jagdtiger => casemate::jagdtiger(hitbox, mounts),
        VehicleKind::PantherII => panther::panther_ii(hitbox, mounts),
        VehicleKind::IS3 => is3::is3(hitbox, mounts),
    })
}

// Smoothing groups shared across families. Group 0 (`hard_edges`) keeps welded plates crisp;
// higher groups mark cast/round surfaces that should read as smooth.
pub(crate) const SG_HARD: SmoothingGroup = SmoothingGroup::hard_edges();
pub(crate) const SG_CAST: SmoothingGroup = SmoothingGroup(2);
pub(crate) const SG_CUPOLA: SmoothingGroup = SmoothingGroup(3);
pub(crate) const SG_BARREL: SmoothingGroup = SmoothingGroup(4);
pub(crate) const SG_WHEEL: SmoothingGroup = SmoothingGroup(5);
pub(crate) const SG_MANTLET: SmoothingGroup = SmoothingGroup(6);
pub(crate) const SG_RING: SmoothingGroup = SmoothingGroup(7);

/// Assemble the three submeshes and mount frames into a baked vehicle. `turret_ring` doubles as
/// the casemate frame for fixed-superstructure tank destroyers (their `turret` submesh simply
/// never traverses, since the sim holds casemate turret yaw at zero).
pub(crate) fn assemble(
    kind: VehicleKind,
    hull: GeometryMesh,
    turret: GeometryMesh,
    gun: GeometryMesh,
    mounts: MountFrames,
) -> BakedVehicle {
    BakedVehicle::new(
        kind,
        vec![
            Submesh { kind: SubmeshKind::Hull, mesh: hull.weld_and_smooth() },
            Submesh { kind: SubmeshKind::Turret, mesh: turret.weld_and_smooth() },
            Submesh { kind: SubmeshKind::Gun, mesh: gun.weld_and_smooth() },
        ],
        mounts,
    )
}
