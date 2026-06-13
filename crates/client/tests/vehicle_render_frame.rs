use client::{VehicleMeshCatalog, split_vehicle_render_frame};
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
