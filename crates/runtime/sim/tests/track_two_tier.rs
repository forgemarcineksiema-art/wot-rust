//! The two-tier track model at the drive layer: a Damaged pool barely dents mobility (two of them
//! compound), a single thrown track crawls but stays controllable and — the headline fix — sits
//! still at rest instead of pivoting forever, and its under-power drift is counter-steerable.

use game_core::{TankSpec, TrackHealth, TrackSide};
use physics::TankKinematicState;
use sim::{
    AimingState, DriveModuleStatus, TankCommand, TankDriveState, TankDriveWorld, TrackDriveStatus,
    step_tank_drive,
};

/// Drive a T-54 for `ticks` at 60 Hz with the given track state and command, from rest.
fn driven(tracks: TrackDriveStatus, command: TankCommand, ticks: u32) -> TankDriveState {
    let spec = TankSpec::t54_1951();
    let mut drive = TankDriveState {
        kinematic: TankKinematicState::default(),
        aiming: AimingState::default(),
        aim_dispersion_mrad: spec.gun.dispersion_mrad,
    };
    let modules = DriveModuleStatus { tracks, ..DriveModuleStatus::healthy(&spec) };
    let world = TankDriveWorld {
        heightmap: None,
        cover: &[],
        tank_obstacles: &[],
        footprint: None,
        water: None,
    };
    for _ in 0..ticks {
        step_tank_drive(&mut drive, &spec, modules, world, command, 1.0 / 60.0);
    }
    drive
}

/// A drive status with the two pools set to the given HP (built by damaging down from full).
fn health_with(left_hp: u8, right_hp: u8) -> TrackDriveStatus {
    let mut tracks = TrackHealth::healthy();
    tracks.damage(TrackSide::Left, game_core::TRACK_HP_MAX - left_hp);
    tracks.damage(TrackSide::Right, game_core::TRACK_HP_MAX - right_hp);
    TrackDriveStatus::from_track_health(&tracks)
}

#[test]
fn one_thrown_track_sits_still_when_idle() {
    // The bug this whole change exists to kill: a hull with one thrown track, given NO input,
    // used to pivot in place forever from a phantom steer bias. It must now stay put.
    let tracks = health_with(0, 100); // left thrown, right whole
    let parked = driven(tracks, TankCommand::idle(), 120);

    assert!(
        parked.kinematic.position.length() < 1.0e-4,
        "an idle one-track hull must not creep: {:?}",
        parked.kinematic.position
    );
    assert!(
        parked.kinematic.yaw_rad.abs() < 1.0e-4,
        "an idle one-track hull must not spin: {}",
        parked.kinematic.yaw_rad
    );
    assert_eq!(parked.kinematic.yaw_rate_rad_s, 0.0, "no residual yaw rate at rest");
}

#[test]
fn one_thrown_track_crawls_under_power_but_slower_than_healthy() {
    let healthy = driven(TrackDriveStatus::healthy(), TankCommand::drive(1.0, 0.0), 60);
    let thrown = driven(health_with(0, 100), TankCommand::drive(1.0, 0.0), 60);

    assert!(thrown.kinematic.position.length() > 0.03, "the live side still crawls the hull");
    assert!(
        thrown.kinematic.position.length() < healthy.kinematic.position.length() * 0.75,
        "one thrown track loses most of the forward reach"
    );
}

#[test]
fn one_thrown_track_drift_is_counter_steerable() {
    // Left thrown: under power the hull drifts toward the dead (left) side. A full counter-steer
    // must not just dent that — it must overpower it and turn the hull the other way.
    let drift = driven(health_with(0, 100), TankCommand::drive(1.0, 0.0), 45);
    let countered = driven(health_with(0, 100), TankCommand::drive(1.0, 1.0), 45);

    assert!(drift.kinematic.yaw_rad < -0.01, "uncorrected, it drifts toward the dead side");
    assert!(
        countered.kinematic.yaw_rad > drift.kinematic.yaw_rad + 0.05,
        "a counter-steer overrides the drift (drift {}, countered {})",
        drift.kinematic.yaw_rad,
        countered.kinematic.yaw_rad
    );
}

#[test]
fn one_damaged_side_barely_dents_turn_but_two_compound() {
    // Pure neutral-steer pivot isolates turn agility. A single damaged pool costs almost nothing;
    // two damaged pools cost noticeably more.
    let pivot = TankCommand::drive(0.0, 1.0);
    let healthy = driven(TrackDriveStatus::healthy(), pivot, 60).kinematic.yaw_rad.abs();
    let one_damaged = driven(health_with(50, 100), pivot, 60).kinematic.yaw_rad.abs();
    let two_damaged = driven(health_with(50, 50), pivot, 60).kinematic.yaw_rad.abs();

    assert!(healthy > 0.0, "the healthy hull pivots");
    assert!(one_damaged > healthy * 0.9, "one damaged side is nearly imperceptible: {one_damaged}");
    assert!(two_damaged < one_damaged, "a second damaged side compounds the loss");
    assert!(two_damaged < healthy * 0.92, "two damaged sides bite the turn");
}

#[test]
fn a_light_hit_leaves_the_track_rolling_only_a_solid_one_throws_it() {
    // Sanity at the state layer: a single small chunk damages without breaking; draining the pool
    // breaks. (The shell-dependent chunk sizing itself is locked in game_core::track.)
    let mut tracks = TrackHealth::healthy();
    tracks.damage(TrackSide::Left, 30);
    assert!(!tracks.is_broken(TrackSide::Left), "a light hit only degrades");
    assert_eq!(TrackDriveStatus::from_track_health(&tracks), health_with(70, 100));

    tracks.damage(TrackSide::Left, 100);
    assert!(tracks.is_broken(TrackSide::Left), "enough damage throws the track");
}
