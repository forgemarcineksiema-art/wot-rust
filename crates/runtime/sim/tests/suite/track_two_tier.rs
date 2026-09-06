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
        footprint: None,
        water: terrain::WaterView::DRY,
        rubble: &[],
        ground: None,
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
fn a_thrown_track_cannot_be_counter_steered_away() {
    // THE PROMISE THIS REPLACES, AND WHY. The old one was
    // `one_thrown_track_drift_is_counter_steerable`, and it asked for exactly what it said: a full
    // counter-steer had to overpower the drift and turn the hull the other way. It got it, because
    // the drift was a fixed 0.18 ADDED TO THE PLAYER'S STEER. Measured against the real drive over
    // ten seconds, holding 0.18 of counter-steer took a T-54 with a dead left track down a dead
    // straight line at 7.67 m/s — the identical speed it made with no steer at all, and 2 m further
    // for not having to arc. The track was thrown and it cost nothing but throttle.
    //
    // A tracked hull steers by running its belts at different speeds; that difference IS the yaw.
    // A dead belt cannot be sped up to match its partner, so the turn is not a bias to be argued
    // with — it is the geometry of having one belt. The only way to straighten out is to slow the
    // live belt down to nothing, which is stopping.
    let drift = driven(health_with(0, 100), TankCommand::drive(1.0, 0.0), 45);
    let countered = driven(health_with(0, 100), TankCommand::drive(1.0, 1.0), 45);

    assert!(drift.kinematic.yaw_rad < -0.01, "uncorrected, it swings toward the dead side");
    assert!(
        countered.kinematic.yaw_rad < 0.0,
        "a full counter-steer must not turn the hull the other way: it can only ask the dead belt \
         to outrun the live one, and a dead belt outruns nothing (countered {})",
        countered.kinematic.yaw_rad
    );
}

/// Stopping is the one thing that DOES straighten a crippled hull, because the forced turn is a
/// belt-speed difference and a stopped hull has no belt speed to differ by. This is what keeps the
/// damage from removing the player: plant it and the turret still fights.
#[test]
fn a_stopped_hull_on_one_track_is_not_forced_to_turn() {
    let rolling = driven(health_with(0, 100), TankCommand::drive(1.0, 0.0), 45);
    let planted = driven(health_with(0, 100), TankCommand::drive(0.0, 0.0), 45);

    assert!(rolling.kinematic.yaw_rad.abs() > 0.05, "under power it is forced round");
    assert!(
        planted.kinematic.yaw_rad.abs() < 0.01,
        "stopped, nothing forces it round (yaw {})",
        planted.kinematic.yaw_rad
    );
}

/// An unevenly drained (but unthrown) pair pulls toward the worse belt — the same rule as the
/// thrown case, applied to a difference that is merely small. An EVEN pair, however damaged, does
/// not pull at all: equal belts do not turn a hull.
#[test]
fn uneven_belts_pull_but_even_ones_do_not() {
    let uneven = driven(health_with(40, 100), TankCommand::drive(1.0, 0.0), 60);
    let even = driven(health_with(40, 40), TankCommand::drive(1.0, 0.0), 60);

    assert!(
        uneven.kinematic.yaw_rad.abs() > even.kinematic.yaw_rad.abs(),
        "the worse belt pulls the hull toward itself (uneven {}, even {})",
        uneven.kinematic.yaw_rad,
        even.kinematic.yaw_rad
    );
    assert!(
        even.kinematic.yaw_rad.abs() < 0.005,
        "an evenly damaged pair still drives where it is pointed (yaw {})",
        even.kinematic.yaw_rad
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
