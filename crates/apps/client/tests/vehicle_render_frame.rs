use client::{
    VehicleAssetCatalog, VehicleMeshCatalog, split_pbr_vehicle_render_frame,
    split_vehicle_render_frame,
};
use engine::PresentationTank;
use game_core::{TankId, VehicleKind};
use vehicle_geometry::RunningGearKinematics;

/// Base submeshes (hull, turret, gun) plus the animated running-gear instances every blueprint
/// vehicle adds (road wheels both sides, two end wheels per side, and the belt links).
fn expected_object_count() -> usize {
    VehicleKind::ALL
        .iter()
        .map(|kind| {
            3 + RunningGearKinematics::for_vehicle(*kind)
                .map_or(0, |kin| kin.wheel_zs.len() * 2 + 4 + kin.link_count() * 2)
        })
        .sum()
}

/// Cached meshes: hull/turret/gun for every vehicle plus three unit gear meshes per blueprint one.
fn expected_mesh_count() -> usize {
    VehicleKind::ALL
        .iter()
        .map(|kind| 3 + if RunningGearKinematics::for_vehicle(*kind).is_some() { 3 } else { 0 })
        .sum()
}

#[test]
fn vehicle_render_frame_uses_baked_objects_for_every_vehicle() {
    let mut catalog = VehicleMeshCatalog::default();
    let tanks: Vec<_> = VehicleKind::ALL
        .iter()
        .enumerate()
        .map(|(index, vehicle)| PresentationTank {
            id: TankId(index as u64 + 1),
            team: game_core::TeamId(1),
            vehicle: *vehicle,
            translation: [index as f32, 0.0, 0.0],
            hull_yaw_rad: 0.0,
            turret_yaw_rad: 0.0,
            gun_pitch_rad: 0.0,
            hit_points: vehicle.spec().hit_points,
            destroyed_modules_mask: 0,
            track_left_m: 0.0,
            track_right_m: 0.0,
        })
        .collect();

    let frame = split_vehicle_render_frame(&mut catalog, tanks, TankId(1), 1.0);

    assert_eq!(frame.objects.len(), expected_object_count());
    assert_eq!(catalog.take_pending_meshes().len(), expected_mesh_count());
}

#[test]
fn pbr_vehicle_render_frame_uses_vehicle_assets_for_every_vehicle() {
    let mut catalog = VehicleAssetCatalog::default();
    let tanks = presentation_tanks();

    let frame = split_pbr_vehicle_render_frame(&mut catalog, tanks, TankId(1), 1.2);

    assert_eq!(frame.objects.len(), expected_object_count());
    assert_eq!(catalog.take_pending_vehicle_meshes().len(), expected_mesh_count());
    assert_eq!(catalog.material_count(), VehicleKind::ALL.len());
    let player_gun = &frame.objects[2];
    let gun_forward = glam::Mat4::from_cols_array_2d(&player_gun.transform)
        .transform_vector3(glam::Vec3::Z)
        .length();
    assert!((gun_forward - 1.2).abs() < 1.0e-4, "player gun keeps barrel scale");
}

fn presentation_tanks() -> Vec<PresentationTank> {
    VehicleKind::ALL
        .iter()
        .enumerate()
        .map(|(index, vehicle)| PresentationTank {
            id: TankId(index as u64 + 1),
            team: game_core::TeamId(1),
            vehicle: *vehicle,
            translation: [index as f32, 0.0, 0.0],
            hull_yaw_rad: 0.0,
            turret_yaw_rad: 0.0,
            gun_pitch_rad: 0.0,
            hit_points: vehicle.spec().hit_points,
            destroyed_modules_mask: 0,
            track_left_m: 0.0,
            track_right_m: 0.0,
        })
        .collect()
}
