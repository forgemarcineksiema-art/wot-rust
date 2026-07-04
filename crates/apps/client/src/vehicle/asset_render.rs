//! PBR vehicle render-object assembly: turning a cached [`VehicleAssetCatalog`] entry plus a pose
//! into the per-submesh and running-gear render objects. Split from `asset_catalog` to keep each
//! module small.

use game_core::TankId;
use glam::Mat4;
use net::TankSnapshot;
use renderer_api::{MaterialHandle, MeshHandle, RenderObject};
use vehicle_geometry::{GearDynamics, RunningGearKinematics};

use super::asset_catalog::VehicleAssetCatalog;
use super::pose::VehiclePose;
use super::running_gear_objects::gear_render_objects;
use super::variation::VehicleVariation;

pub fn tank_vehicle_render_objects(
    catalog: &mut VehicleAssetCatalog,
    snapshot: &TankSnapshot,
    hull_color: [f32; 3],
) -> Vec<RenderObject> {
    tank_vehicle_render_objects_with_variation(
        catalog,
        snapshot,
        hull_color,
        &VehicleVariation::from_snapshot(snapshot),
    )
}

/// As [`tank_vehicle_render_objects`], but with an explicit runtime variation state (camo, dirt,
/// snow, broken tracks, destroyed modules). The base render path uses the snapshot-derived
/// variation; callers carrying richer per-tank cosmetic state pass it here.
pub fn tank_vehicle_render_objects_with_variation(
    catalog: &mut VehicleAssetCatalog,
    snapshot: &TankSnapshot,
    hull_color: [f32; 3],
    variation: &VehicleVariation,
) -> Vec<RenderObject> {
    tank_vehicle_render_objects_with_tracks(catalog, snapshot, hull_color, variation, 0.0, 0.0)
}

/// As [`tank_vehicle_render_objects_with_variation`], but also instances the animatable running gear
/// (spinning wheels, scrolling track links) from the per-side track distance the presentation world
/// accumulates. Vehicles without blueprint gear ignore the distances and render unchanged.
pub fn tank_vehicle_render_objects_with_tracks(
    catalog: &mut VehicleAssetCatalog,
    snapshot: &TankSnapshot,
    hull_color: [f32; 3],
    variation: &VehicleVariation,
    track_left_m: f32,
    track_right_m: f32,
) -> Vec<RenderObject> {
    tank_vehicle_render_objects_posed(
        catalog,
        snapshot,
        hull_color,
        variation,
        track_left_m,
        track_right_m,
        [0.0; 3],
        GearDynamics::default(),
    )
}

/// As [`tank_vehicle_render_objects_with_tracks`], with the sprung-hull attitude
/// `[pitch, roll, heave]` from the presentation world folded into the hull frame.
#[allow(clippy::too_many_arguments)]
pub fn tank_vehicle_render_objects_posed(
    catalog: &mut VehicleAssetCatalog,
    snapshot: &TankSnapshot,
    hull_color: [f32; 3],
    variation: &VehicleVariation,
    track_left_m: f32,
    track_right_m: f32,
    attitude: [f32; 3],
    gear_dynamics: GearDynamics<'_>,
) -> Vec<RenderObject> {
    let entry = catalog.vehicle_entry(snapshot.vehicle).expect("vehicle must have baked geometry");
    let pose = VehiclePose::new_with_attitude(
        snapshot.vehicle,
        glam::Vec3::from_array(snapshot.position),
        snapshot.yaw_rad,
        snapshot.turret_yaw_rad,
        snapshot.gun_pitch_rad,
        attitude,
    );
    let hull_transform =
        Mat4::from_translation(pose.hull_translation()) * Mat4::from_mat3(pose.hull_basis());
    let turret_transform =
        Mat4::from_translation(pose.turret_translation()) * Mat4::from_mat3(pose.turret_basis());
    let gun_transform =
        Mat4::from_translation(pose.gun_translation()) * Mat4::from_mat3(pose.gun_basis());

    // The cosmetic overlays (camo/dirt/snow) recolour the whole vehicle. A LIVE tank never
    // recolours with damage — wounds speak locally (hit decals, smoke), not through a palette
    // swap. Only a knocked-out hull takes the whole-vehicle burnt-out tint.
    let surface = variation.surface_tint(hull_color);
    let tint =
        if snapshot.hit_points == 0 { VehicleVariation::wreck_tint(surface) } else { surface };

    let mut objects = vec![
        vehicle_render_object(snapshot.tank_id, entry.hull, entry.material, hull_transform, tint),
        vehicle_render_object(
            snapshot.tank_id,
            entry.turret,
            entry.material,
            turret_transform,
            tint,
        ),
        vehicle_render_object(snapshot.tank_id, entry.gun, entry.material, gun_transform, tint),
    ];

    // Running gear rides the hull and tints with the suspension; the wheels spin and the links
    // scroll by the per-side track distance.
    if let Some(handles) = entry.running_gear
        && let Some(kin) = RunningGearKinematics::for_vehicle(snapshot.vehicle)
    {
        let track_left_m = if variation.left_track_broken() { 0.0 } else { track_left_m };
        let track_right_m = if variation.right_track_broken() { 0.0 } else { track_right_m };
        objects.extend(gear_render_objects(
            snapshot.tank_id,
            handles,
            entry.material,
            hull_transform,
            &kin,
            track_left_m,
            track_right_m,
            gear_dynamics,
            tint,
        ));
    }

    objects
}

fn vehicle_render_object(
    tank_id: TankId,
    mesh: MeshHandle,
    material: MaterialHandle,
    transform: Mat4,
    tint: [f32; 3],
) -> RenderObject {
    RenderObject {
        tank_id: Some(tank_id),
        mesh,
        material,
        transform: transform.to_cols_array_2d(),
        tint,
    }
}
