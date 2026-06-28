use engine::PresentationWorld;
use game_core::{TankId, TeamId, VehicleKind};
use net::TankSnapshot;

fn snapshot(id: u64, position: [f32; 3], hit_points: u32) -> TankSnapshot {
    TankSnapshot {
        tank_id: TankId(id),
        team: TeamId(1),
        vehicle: VehicleKind::T55A,
        position,
        yaw_rad: 0.1,
        turret_yaw_rad: 0.2,
        turret_yaw_velocity_rad_s: 0.0,
        gun_pitch_rad: 0.3,
        hit_points,
        reload_remaining_s: 0.0,
        aim_dispersion_mrad: 2.9,
        module_hit_points: VehicleKind::T55A.spec().module_health.hit_points_by_slot(),
        destroyed_modules_mask: 0,
    }
}

#[test]
fn sync_spawns_an_entity_per_tank_and_extracts_its_pose() {
    let mut world = PresentationWorld::default();

    world.sync_tanks(&[snapshot(1, [1.0, 0.0, 2.0], 900), snapshot(2, [3.0, 0.0, 4.0], 500)]);

    assert_eq!(world.tank_count(), 2);
    let tanks = world.presentation_tanks();
    assert_eq!(tanks.len(), 2);
    // Ordered by id for deterministic rendering.
    assert_eq!(tanks[0].id, TankId(1));
    assert_eq!(tanks[0].translation, [1.0, 0.0, 2.0]);
    assert_eq!(tanks[0].hit_points, 900);
    assert_eq!(tanks[0].turret_yaw_rad, 0.2);
    assert_eq!(tanks[1].id, TankId(2));
    assert_eq!(tanks[1].hit_points, 500);
}

#[test]
fn re_syncing_a_tank_updates_in_place_without_respawning() {
    let mut world = PresentationWorld::default();
    world.sync_tanks(&[snapshot(1, [0.0, 0.0, 0.0], 900)]);
    let first = world.presentation_tanks();

    world.sync_tanks(&[snapshot(1, [10.0, 0.0, 0.0], 300)]);
    let second = world.presentation_tanks();

    // Same single entity, moved and damaged — not a duplicate.
    assert_eq!(world.tank_count(), 1);
    assert_eq!(first.len(), 1);
    assert_eq!(second.len(), 1);
    assert_eq!(second[0].translation, [10.0, 0.0, 0.0]);
    assert_eq!(second[0].hit_points, 300);
}

#[test]
fn a_tank_absent_from_the_next_sync_is_despawned() {
    let mut world = PresentationWorld::default();
    world.sync_tanks(&[snapshot(1, [0.0; 3], 900), snapshot(2, [0.0; 3], 900)]);
    assert_eq!(world.tank_count(), 2);

    world.sync_tanks(&[snapshot(1, [0.0; 3], 900)]);

    assert_eq!(world.tank_count(), 1);
    let tanks = world.presentation_tanks();
    assert_eq!(tanks.len(), 1);
    assert_eq!(tanks[0].id, TankId(1));
}

fn posed(position: [f32; 3], yaw_rad: f32) -> TankSnapshot {
    TankSnapshot { yaw_rad, ..snapshot(1, position, 900) }
}

fn half_gauge() -> f32 {
    game_core::HitboxProfile::for_vehicle(VehicleKind::T55A).half_width_m
}

#[test]
fn driving_forward_advances_both_tracks_equally() {
    let mut world = PresentationWorld::default();
    world.sync_tanks(&[posed([0.0, 0.0, 0.0], 0.0)]); // seed
    world.sync_tanks(&[posed([0.0, 0.0, 5.0], 0.0)]); // +Z is forward at yaw 0

    let tank = world.presentation_tanks().remove(0);
    assert!((tank.track_left_m - 5.0).abs() < 1.0e-4, "left {}", tank.track_left_m);
    assert!((tank.track_right_m - 5.0).abs() < 1.0e-4, "right {}", tank.track_right_m);
}

#[test]
fn pivoting_in_place_runs_the_tracks_in_opposite_directions() {
    let mut world = PresentationWorld::default();
    world.sync_tanks(&[posed([0.0, 0.0, 0.0], 0.0)]); // seed
    world.sync_tanks(&[posed([0.0, 0.0, 0.0], 0.5)]); // rotate, no translation

    let tank = world.presentation_tanks().remove(0);
    assert!(tank.track_left_m < -1.0e-3, "left should run back, got {}", tank.track_left_m);
    assert!(tank.track_right_m > 1.0e-3, "right should run forward, got {}", tank.track_right_m);
    assert!((tank.track_left_m + tank.track_right_m).abs() < 1.0e-4, "a pure pivot is symmetric");
    assert!((tank.track_right_m - 0.5 * half_gauge()).abs() < 1.0e-4);
}

#[test]
fn reversing_runs_both_tracks_backward() {
    let mut world = PresentationWorld::default();
    world.sync_tanks(&[posed([0.0, 0.0, 0.0], 0.0)]); // seed
    world.sync_tanks(&[posed([0.0, 0.0, -3.0], 0.0)]); // backward

    let tank = world.presentation_tanks().remove(0);
    assert!((tank.track_left_m + 3.0).abs() < 1.0e-4, "left {}", tank.track_left_m);
    assert!((tank.track_right_m + 3.0).abs() < 1.0e-4, "right {}", tank.track_right_m);
}

#[test]
fn advance_time_accumulates_ticks_and_elapsed_seconds() {
    let mut world = PresentationWorld::default();

    world.advance_time(0.5);
    world.advance_time(0.25);

    let time = world.time();
    assert_eq!(time.tick, 2);
    assert!((time.delta_seconds - 0.25).abs() < 1.0e-6);
    assert!((time.elapsed_seconds - 0.75).abs() < 1.0e-6);
}
