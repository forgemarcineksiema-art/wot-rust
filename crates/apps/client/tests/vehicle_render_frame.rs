use client::{
    VehicleAssetCatalog, VehicleMeshCatalog, VehicleRenderFrame, split_pbr_vehicle_render_frame,
    split_vehicle_render_frame,
};
use engine::PresentationTank;
use game_core::{ModuleSlot, TankId, TrackDamageMask, VehicleKind};
use vehicle_geometry::RunningGearKinematics;

/// Base submeshes plus the authoritative placement pass. Counting the placements themselves keeps
/// this contract honest when a suspension family changes cardinality (for example, Centurion's
/// three paired Horstmann bogies instead of six independent arms per side).
fn expected_object_count() -> usize {
    VehicleKind::ALL
        .iter()
        .map(|kind| {
            3 + RunningGearKinematics::for_vehicle(*kind)
                .map_or(0, |kin| vehicle_geometry::running_gear_placements(&kin, 0.0, 0.0).len())
        })
        .sum()
}

/// Cached meshes: hull/turret/gun for every vehicle plus six unit gear meshes per blueprint one
/// (road wheel, swing arm, sprocket, idler, track link, return roller).
fn expected_mesh_count() -> usize {
    VehicleKind::ALL
        .iter()
        .map(|kind| 3 + if RunningGearKinematics::for_vehicle(*kind).is_some() { 6 } else { 0 })
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
            spotted_by_teams_mask: 0,
            module_hit_points: vehicle.spec().module_health.hit_points_by_slot(),
            track_damage_mask: 0,
            track_break_t: [None, None],
            engine_fire: false,
            fuel_fire: false,
            armor_breaches: Default::default(),
            track_left_m: 0.0,
            track_right_m: 0.0,
            attitude_pitch_rad: 0.0,
            attitude_roll_rad: 0.0,
            attitude_heave_m: 0.0,
            accel_long_mps2: 0.0,
            gun_recoil_m: 0.0,
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

#[test]
fn pbr_render_stops_track_scroll_when_tracks_are_broken() {
    let moving = frame_for_t54_tracks(1.25, 1.25, 0);
    let rest = frame_for_t54_tracks(0.0, 0.0, 0);
    let broken_moving =
        frame_for_t54_tracks_with_masks(1.25, 1.25, ModuleSlot::Suspension.destroyed_mask_bit(), 0);
    let broken_rest =
        frame_for_t54_tracks_with_masks(0.0, 0.0, ModuleSlot::Suspension.destroyed_mask_bit(), 0);

    assert!(
        moving.objects[3..].iter().zip(&rest.objects[3..]).any(|(a, b)| a.transform != b.transform),
        "healthy running gear should animate when track distance changes"
    );
    assert_eq!(
        broken_moving.objects[3..],
        broken_rest.objects[3..],
        "broken tracks should render at the stopped phase instead of continuing to scroll"
    );
}

#[test]
fn pbr_render_stops_only_the_broken_track_side() {
    let left_broken_moving =
        frame_for_t54_tracks_with_masks(1.25, 1.25, 0, TrackDamageMask::LEFT.bits());
    let left_broken_rest =
        frame_for_t54_tracks_with_masks(0.0, 0.0, 0, TrackDamageMask::LEFT.bits());

    let kin = RunningGearKinematics::for_vehicle(VehicleKind::T54_1951).expect("T-54 gear");
    let right_count = kin.wheel_zs.len() * 2 + 2 + kin.link_count();
    let right_range = 3..3 + right_count;
    let left_range = 3 + right_count..3 + right_count * 2;

    assert!(
        left_broken_moving.objects[right_range.clone()]
            .iter()
            .zip(&left_broken_rest.objects[right_range])
            .any(|(a, b)| a.transform != b.transform),
        "healthy right side should keep scrolling"
    );
    assert_eq!(
        left_broken_moving.objects[left_range.clone()],
        left_broken_rest.objects[left_range],
        "broken left side should stay stopped"
    );
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
            spotted_by_teams_mask: 0,
            module_hit_points: vehicle.spec().module_health.hit_points_by_slot(),
            track_damage_mask: 0,
            track_break_t: [None, None],
            engine_fire: false,
            fuel_fire: false,
            armor_breaches: Default::default(),
            track_left_m: 0.0,
            track_right_m: 0.0,
            attitude_pitch_rad: 0.0,
            attitude_roll_rad: 0.0,
            attitude_heave_m: 0.0,
            accel_long_mps2: 0.0,
            gun_recoil_m: 0.0,
        })
        .collect()
}

fn frame_for_t54_tracks(left: f32, right: f32, destroyed_modules_mask: u8) -> VehicleRenderFrame {
    frame_for_t54_tracks_with_masks(left, right, destroyed_modules_mask, 0)
}

fn frame_for_t54_tracks_with_masks(
    left: f32,
    right: f32,
    destroyed_modules_mask: u8,
    track_damage_mask: u8,
) -> VehicleRenderFrame {
    let mut catalog = VehicleAssetCatalog::default();
    split_pbr_vehicle_render_frame(
        &mut catalog,
        vec![PresentationTank {
            id: TankId(1),
            team: game_core::TeamId(1),
            vehicle: VehicleKind::T54_1951,
            translation: [0.0, 0.0, 0.0],
            hull_yaw_rad: 0.0,
            turret_yaw_rad: 0.0,
            gun_pitch_rad: 0.0,
            hit_points: VehicleKind::T54_1951.spec().hit_points,
            destroyed_modules_mask,
            spotted_by_teams_mask: 0,
            module_hit_points: VehicleKind::T54_1951.spec().module_health.hit_points_by_slot(),
            track_damage_mask,
            track_break_t: [None, None],
            engine_fire: false,
            fuel_fire: false,
            armor_breaches: Default::default(),
            track_left_m: left,
            track_right_m: right,
            attitude_pitch_rad: 0.0,
            attitude_roll_rad: 0.0,
            attitude_heave_m: 0.0,
            accel_long_mps2: 0.0,
            gun_recoil_m: 0.0,
        }],
        TankId(1),
        1.0,
    )
}
