use game_core::{TankSpec, TeamId, TrackDamageMask, TrackSide};
use glam::Vec3;
use sim::{FixedTimestep, SimulationState, TankCommand};

#[test]
fn side_specific_track_damage_changes_only_that_side() {
    let mut state = SimulationState::new();
    let tank = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::ZERO);

    state.damage_track(tank, TrackSide::Left);

    let tank = state.tank(tank).expect("tank");
    assert!(tank.tracks.is_broken(TrackSide::Left));
    assert!(!tank.tracks.is_broken(TrackSide::Right));
    assert_eq!(tank.tracks.broken_mask(), TrackDamageMask::LEFT);
}

#[test]
fn one_broken_track_allows_asymmetric_crawl_but_two_stop_drive() {
    let mut left_state = SimulationState::new();
    let left = left_state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::ZERO);
    left_state.damage_track(left, TrackSide::Left);
    drive_for_one_second(&mut left_state, left);

    let mut both_state = SimulationState::new();
    let both = both_state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::ZERO);
    both_state.damage_track(both, TrackSide::Left);
    both_state.damage_track(both, TrackSide::Right);
    drive_for_one_second(&mut both_state, both);

    let left_tank = left_state.tank(left).expect("left damaged tank");
    let both_tank = both_state.tank(both).expect("both damaged tank");
    assert!(left_tank.position.length() > 0.03);
    assert!(left_tank.yaw_rad.abs() > 0.01);
    assert_eq!(both_tank.position, Vec3::ZERO);
    assert_eq!(both_tank.hull_yaw_velocity_rad_s, 0.0);
}

fn drive_for_one_second(state: &mut SimulationState, tank: game_core::TankId) {
    let step = FixedTimestep::from_hz(60);
    let command = TankCommand::drive(1.0, 0.0);
    for _ in 0..60 {
        state.apply_commands(&[(tank, command)], step);
    }
}
