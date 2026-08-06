use game_core::TankSpec;
use glam::Vec3;
use physics::TankKinematicState;
use sim::{
    AimingState, DriveModuleStatus, TankCommand, TankDriveState, TankDriveWorld, TrackDriveStatus,
    TrackSideDrive, step_tank_drive,
};

#[test]
fn t54_per_side_track_damage_changes_drive_behavior() {
    let healthy = driven_for(TrackDriveStatus::healthy());
    let left_broken = driven_for(TrackDriveStatus {
        left: TrackSideDrive::Thrown,
        right: TrackSideDrive::Rolling(1.0),
    });
    let both_broken = driven_for(TrackDriveStatus::broken());

    assert!(
        healthy.kinematic.position.length() > 0.3,
        "healthy tracks should move the T-54 forward"
    );
    assert!(
        left_broken.kinematic.position.length() > 0.03,
        "one working track should still crawl or pivot instead of hard immobilizing"
    );
    assert!(
        left_broken.kinematic.position.length() < healthy.kinematic.position.length() * 0.75,
        "one broken track should lose most forward traction"
    );
    assert!(
        left_broken.kinematic.yaw_rad.abs() > healthy.kinematic.yaw_rad.abs() + 0.01,
        "one broken track should create asymmetric yaw/drag"
    );
    assert_eq!(
        both_broken.kinematic.position,
        Vec3::ZERO,
        "two broken tracks stop controlled hull movement"
    );
    assert_eq!(both_broken.kinematic.yaw_rate_rad_s, 0.0);
}

fn driven_for(tracks: TrackDriveStatus) -> TankDriveState {
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
        water: None,
        rubble: &[],
        ground: None,
    };
    let command = TankCommand::drive(1.0, 0.0);

    for _ in 0..60 {
        step_tank_drive(&mut drive, &spec, modules, world, command, 1.0 / 60.0);
    }
    drive
}
