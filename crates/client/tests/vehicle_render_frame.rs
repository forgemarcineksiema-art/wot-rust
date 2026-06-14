use client::{
    VehicleAssetCatalog, VehicleMeshCatalog, split_pbr_vehicle_render_frame,
    split_vehicle_render_frame,
};
use engine::PresentationTank;
use game_core::{TankId, VehicleKind};

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
        })
        .collect();

    let frame = split_vehicle_render_frame(&mut catalog, tanks, TankId(1), 1.0);

    assert_eq!(frame.objects.len(), VehicleKind::ALL.len() * 3);
    assert_eq!(catalog.take_pending_meshes().len(), VehicleKind::ALL.len() * 3);
}

#[test]
fn pbr_vehicle_render_frame_uses_vehicle_assets_for_every_vehicle() {
    let mut catalog = VehicleAssetCatalog::default();
    let tanks = presentation_tanks();

    let frame = split_pbr_vehicle_render_frame(&mut catalog, tanks, TankId(1), 1.2);

    assert_eq!(frame.objects.len(), VehicleKind::ALL.len() * 3);
    assert_eq!(catalog.take_pending_vehicle_meshes().len(), VehicleKind::ALL.len() * 3);
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
        })
        .collect()
}
