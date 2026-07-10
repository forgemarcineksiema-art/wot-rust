//! The IS-3: the era's Soviet heavy, blueprint-born from day one. The hull is bespoke — the
//! pike bow ("shchuchy nos") is authored face by face on the EXACT planes the armor volumes
//! shoot against (same fold ridge, same slopes, same ±sweep), so the visible bow and the
//! penetration model are one geometry. The dome, running gear, and the braked 122 mm ride the
//! shared blueprint family machinery.

use game_core::{HitboxProfile, HullShape, MountFrames, TrackShape, VehicleKind};
use glam::{Vec2, Vec3};

use super::is3_hull::is3_pike_hull;
use super::soviet::soviet_cast_turret_for;
use super::{
    GunPlan, SG_HARD, assemble, blueprint_deck_details, blueprint_running_gear, blueprint_skirts,
    build_gun, shade_hull,
};
use crate::{Axis, BakedVehicle, ExtrudeSpec, GeometryMesh, MaterialRole, MeshBuilder};

pub(crate) fn is3(_hitbox: &HitboxProfile, mounts: &MountFrames) -> BakedVehicle {
    let bp =
        game_core::VehicleBlueprint::for_vehicle(VehicleKind::IS3).expect("IS-3 has a blueprint");
    let hull = shade_hull(
        is3_pike_hull(&bp.hull)
            .append(&blueprint_running_gear(&bp.track))
            .append(&blueprint_deck_details(&bp.hull))
            .append(&blueprint_skirts(&bp.hull, &bp.track))
            .append(&is3_fenders(&bp.hull, &bp.track))
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

/// The IS-3's fender line: a thin full-length shelf over each track band, the signature front
/// mudguards sloping down off the pike cheeks over the idlers, and short rear flaps over the
/// sprockets — the reference look that visually ties the running gear to the hull.
fn is3_fenders(hull: &HullShape, track: &TrackShape) -> GeometryMesh {
    // The fender covers exactly the track band, a hair inside it on both edges: its tin must
    // not reach inboard of the belt (the pike lock rightly treats that as hull armor) nor poke
    // past the hitbox width the track's outer face already sits flush against.
    let center_x = (track.inner_x + track.outer_x) * 0.5;
    let half_x = (track.outer_x - track.inner_x) * 0.5 - 0.005;
    let shelf_y = hull.sponson_y + 0.015;
    let belt_top = track.end_y + track.end_radius + 0.04;

    // Flat shelf over the track from the hull rear to where the front mudguard takes over.
    let shelf_front_z = hull.half_len - 1.35;
    let shelf = ExtrudeSpec {
        section: vec![
            Vec2::new(-hull.half_len + 0.15, shelf_y),
            Vec2::new(shelf_front_z, shelf_y),
            Vec2::new(shelf_front_z, shelf_y + 0.035),
            Vec2::new(-hull.half_len + 0.15, shelf_y + 0.035),
        ],
        axis: Axis::X,
        half_depth: half_x,
        material: MaterialRole::RolledArmor,
        smoothing: SG_HARD,
    };
    // Front mudguard: slopes from the shelf line down over the idler to just above the belt,
    // stopping short of the hitbox nose so every LOD stays inside the collision envelope.
    let guard_tip_z = track.end_z + track.end_radius + 0.20;
    let front = ExtrudeSpec {
        section: vec![
            Vec2::new(shelf_front_z, shelf_y),
            Vec2::new(guard_tip_z, belt_top),
            Vec2::new(guard_tip_z, belt_top + 0.035),
            Vec2::new(shelf_front_z, shelf_y + 0.035),
        ],
        axis: Axis::X,
        half_depth: half_x,
        material: MaterialRole::RolledArmor,
        smoothing: SG_HARD,
    };
    // Rear flap: a short drop over the sprocket, inside the hitbox tail.
    let rear = ExtrudeSpec {
        section: vec![
            Vec2::new(-(track.end_z + track.end_radius + 0.20), belt_top),
            Vec2::new(-hull.half_len + 0.15, shelf_y),
            Vec2::new(-hull.half_len + 0.15, shelf_y + 0.035),
            Vec2::new(-(track.end_z + track.end_radius + 0.20), belt_top + 0.035),
        ],
        axis: Axis::X,
        half_depth: half_x,
        material: MaterialRole::RolledArmor,
        smoothing: SG_HARD,
    };

    let mut builder = MeshBuilder::new();
    for spec in [shelf, front, rear] {
        builder = builder.extrude(Vec3::new(center_x, 0.0, 0.0), spec);
    }
    builder.mirror(Axis::X).build()
}
