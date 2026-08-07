//! Geometry-path render-object assembly: turns a cached [`VehicleMeshCatalog`] entry plus a pose
//! into hull/turret/gun objects and the rest-pose running gear. Split from `render_objects` to keep
//! each module small; the live battle uses the PBR path (`asset_render`).

use game_core::{ModuleSlot, TankId};
use glam::Mat4;
use net::TankSnapshot;
use renderer_api::{MaterialHandle, MeshHandle, RenderObject};
use vehicle_geometry::RunningGearKinematics;

use super::pose::VehiclePose;
use super::render_objects::VehicleMeshCatalog;
use super::running_gear_objects::gear_render_objects;

/// Assemble one tank's hull, turret, gun and rest-pose running gear.
///
/// This path exists to LOOK at a vehicle — the garage, the studio, the offscreen lineups — so it
/// always builds the AUTHORED construction and never a distance mesh. The live battle takes the
/// PBR path (`split_pbr_vehicle_render_frame_on_terrain`), which picks a gear tier from the
/// camera and scrolls the gear from accumulated track distance.
///
/// It used to carry a `GearTier` parameter so the frame probe could bracket the detail tier from
/// here. That probe now measures through the battle's own builder, which is where the tier rule
/// lives — so the parameter, the enum and the two wrapper functions around them went with it
/// rather than staying as a second place the tier could be decided.
pub fn tank_render_objects(
    catalog: &mut VehicleMeshCatalog,
    snapshot: &TankSnapshot,
    hull_color: [f32; 3],
) -> Vec<RenderObject> {
    let entry = catalog.vehicle_entry(snapshot.vehicle).expect("vehicle must have baked geometry");
    let pose = VehiclePose::from_snapshot(snapshot);

    let hull_transform =
        Mat4::from_translation(pose.hull_translation()) * Mat4::from_mat3(pose.hull_basis());
    let turret_transform =
        Mat4::from_translation(pose.turret_translation()) * Mat4::from_mat3(pose.turret_basis());
    let gun_transform =
        Mat4::from_translation(pose.gun_translation()) * Mat4::from_mat3(pose.gun_basis());

    let hull_tint = damage_tint(
        hull_color,
        snapshot.destroyed_modules_mask,
        &[ModuleSlot::Engine, ModuleSlot::Suspension],
    );
    let turret_tint = damage_tint(
        hull_color,
        snapshot.destroyed_modules_mask,
        &[ModuleSlot::Turret, ModuleSlot::AmmoRack],
    );
    let gun_tint = damage_tint(hull_color, snapshot.destroyed_modules_mask, &[ModuleSlot::Gun]);

    let mut objects = vec![
        object(snapshot.tank_id, entry.hull, entry.material, hull_transform, hull_tint),
        object(snapshot.tank_id, entry.turret, entry.material, turret_transform, turret_tint),
        object(snapshot.tank_id, entry.gun, entry.material, gun_transform, gun_tint),
    ];

    // Running gear at rest (phase 0): this static path feeds the offscreen lineup examples; the
    // live battle uses the PBR path, which scrolls the gear from the accumulated track distance.
    if let Some(handles) = entry.running_gear
        && let Some(kin) = RunningGearKinematics::for_vehicle(snapshot.vehicle)
    {
        objects.extend(gear_render_objects(
            snapshot.tank_id,
            handles,
            entry.material,
            hull_transform,
            &kin,
            0.0,
            0.0,
            vehicle_geometry::GearDynamics::default(),
            hull_tint,
            vehicle_geometry::GearDetail::Near,
        ));
    }

    objects
}

fn object(
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

fn damage_tint(base: [f32; 3], mask: u8, slots: &[ModuleSlot]) -> [f32; 3] {
    if slots.iter().any(|slot| mask & slot.destroyed_mask_bit() != 0) {
        [base[0] * 0.42, base[1] * 0.40, base[2] * 0.36]
    } else {
        base
    }
}
