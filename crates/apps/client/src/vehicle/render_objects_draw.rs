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

pub fn tank_render_objects(
    catalog: &mut VehicleMeshCatalog,
    snapshot: &TankSnapshot,
    hull_color: [f32; 3],
) -> Vec<RenderObject> {
    tank_render_objects_from_eye(catalog, snapshot, hull_color, None)
}

/// The same assembly, with the camera the live battle uses to pick a gear detail tier.
///
/// `None` keeps the authored construction (the garage, the studio and the look probes exist to
/// LOOK at a vehicle and must never be shown the distance mesh). `Some(eye)` applies the SAME
/// rule the battle path applies — `gear_detail_for_distance` — so a cost measurement made
/// through this path measures the gear the game actually draws at that range, not the near tier
/// on every tank regardless of how far away it is.
pub fn tank_render_objects_from_eye(
    catalog: &mut VehicleMeshCatalog,
    snapshot: &TankSnapshot,
    hull_color: [f32; 3],
    eye: Option<[f32; 3]>,
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
            // With no camera this path exists to LOOK at the vehicle (garage, studio, lineups),
            // so it keeps the authored construction; with one it obeys the battle's own rule.
            eye.map_or(vehicle_geometry::GearDetail::Near, |eye| {
                let distance = glam::Vec3::from_array(eye)
                    .distance(glam::Vec3::from_array(pose.hull_translation().into()));
                vehicle_geometry::gear_detail_for_distance(distance)
            }),
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
