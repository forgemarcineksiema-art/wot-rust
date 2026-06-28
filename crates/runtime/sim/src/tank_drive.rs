use game_core::{ModuleSlot, TankSpec};
use physics::{
    TankControlInput, TankControllerSettings, TankFootprint, TankKinematicState, TankObstacle,
    TankWorldObstacles, step_tank_on_world_with_tanks,
};
use terrain::{HeightMap, StaticCoverObject};

use crate::TankCommand;
use crate::aim_dispersion::command_bloom;
use crate::aiming::{AimingState, step_aiming};
use crate::tank_state::TankState;

/// The minimal per-tank state a fixed-tick drive step reads and writes: hull kinematics, turret/gun
/// aim, and the live aim dispersion. The server projects a `TankState` into this; the client
/// predictor stores it directly. Both run [`step_tank_drive`], so the local tank is simulated by
/// exactly the same code as the authority.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TankDriveState {
    pub kinematic: TankKinematicState,
    pub aiming: AimingState,
    pub aim_dispersion_mrad: f32,
}

/// Which modules still work, plus partial gun damage for the dispersion math. Built from
/// `ModuleHealth` on the server and from the snapshot's destroyed-module mask on the client.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DriveModuleStatus {
    pub tracks_ok: bool,
    pub engine_ok: bool,
    pub turret_ok: bool,
    pub gun_damage_fraction: f32,
}

/// The world a hull is driven against for one tick.
#[derive(Debug, Clone, Copy)]
pub struct TankDriveWorld<'a> {
    pub heightmap: Option<&'a HeightMap>,
    pub cover: &'a [StaticCoverObject],
    pub tank_obstacles: &'a [TankObstacle],
}

/// Advance one fixed tick: movement (terrain + cover + tank collision), turret/gun aiming, and
/// command bloom — each gated by module health. Aim-dispersion recovery and firing are the
/// caller's responsibility. `command` must already be clamped.
pub fn step_tank_drive(
    drive: &mut TankDriveState,
    spec: &TankSpec,
    modules: DriveModuleStatus,
    world: TankDriveWorld<'_>,
    command: TankCommand,
    dt: f32,
) {
    if modules.tracks_ok {
        let settings = TankControllerSettings::from_spec(spec);
        // A dead engine removes drive; a thrown track (the `else` branch) removes all movement.
        let (throttle, steer) =
            if modules.engine_ok { (command.throttle, command.steer) } else { (0.0, 0.0) };
        let input = TankControlInput { throttle, steer, brake: command.brake };
        let obstacles = TankWorldObstacles::new(
            world.cover,
            TankFootprint::from_hitbox(spec.hitbox),
            world.tank_obstacles,
        );
        step_tank_on_world_with_tanks(
            &mut drive.kinematic,
            input,
            &settings,
            world.heightmap,
            obstacles,
            dt,
        );
    } else {
        // A thrown track removes all hull motion, linear and angular.
        drive.kinematic.velocity = glam::Vec3::ZERO;
        drive.kinematic.yaw_rate_rad_s = 0.0;
    }

    // A destroyed turret cannot traverse, but the gun can still elevate.
    let mut aim_command = command;
    if !modules.turret_ok {
        aim_command.turret_yaw_delta = 0.0;
        drive.aiming.turret_yaw_velocity_rad_s = 0.0;
    }
    step_aiming(&mut drive.aiming, spec, aim_command, dt);

    // Bloom reads the hull's world velocity magnitude. The rigid-body state already carries the
    // velocity vector, so this is the same value the server stores in `velocity_mps`.
    command_bloom(
        &mut drive.aim_dispersion_mrad,
        spec,
        modules.gun_damage_fraction,
        drive.kinematic.velocity.length(),
        command,
        dt,
    );
}

pub(crate) fn step_tank(
    tank: &mut TankState,
    command: TankCommand,
    dt: f32,
    heightmap: Option<&HeightMap>,
    cover: &[StaticCoverObject],
    tank_obstacles: &[TankObstacle],
) {
    let mut drive = TankDriveState {
        kinematic: TankKinematicState {
            position: tank.position,
            velocity: tank.velocity_mps,
            yaw_rad: tank.yaw_rad,
            yaw_rate_rad_s: tank.hull_yaw_velocity_rad_s,
        },
        aiming: AimingState {
            turret_yaw_rad: tank.turret_yaw_rad,
            turret_yaw_velocity_rad_s: tank.turret_yaw_velocity_rad_s,
            gun_pitch_rad: tank.gun_pitch_rad,
        },
        aim_dispersion_mrad: tank.aim_dispersion_mrad,
    };
    let modules = DriveModuleStatus {
        tracks_ok: tank.modules.is_functional(ModuleSlot::Suspension),
        engine_ok: tank.modules.is_functional(ModuleSlot::Engine),
        turret_ok: tank.modules.is_functional(ModuleSlot::Turret),
        gun_damage_fraction: crate::aim_dispersion::gun_damage_fraction(tank),
    };
    let world = TankDriveWorld { heightmap, cover, tank_obstacles };

    step_tank_drive(&mut drive, &tank.spec, modules, world, command, dt);

    tank.position = drive.kinematic.position;
    tank.yaw_rad = drive.kinematic.yaw_rad;
    tank.velocity_mps = drive.kinematic.velocity;
    tank.hull_yaw_velocity_rad_s = drive.kinematic.yaw_rate_rad_s;
    tank.turret_yaw_rad = drive.aiming.turret_yaw_rad;
    tank.turret_yaw_velocity_rad_s = drive.aiming.turret_yaw_velocity_rad_s;
    tank.gun_pitch_rad = drive.aiming.gun_pitch_rad;
    tank.aim_dispersion_mrad = drive.aim_dispersion_mrad;
}
