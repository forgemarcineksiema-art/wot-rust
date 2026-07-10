use game_core::{ModuleSlot, TankSpec};
use physics::{
    GroundStep, TankControlInput, TankControllerSettings, TankFootprint, TankKinematicState,
    TankObstacle, TankWorldObstacles, step_tank_on_world_with_tanks,
};
use terrain::{HeightMap, StaticCoverObject};

use crate::TankCommand;
use crate::aim_dispersion::command_bloom;
use crate::aiming::{AimingState, step_aiming};
use crate::drive_modules::{DriveModuleStatus, TrackDriveStatus};
use crate::tank_state::TankState;

/// A hull with two fully-drained-but-not-thrown tracks keeps at least this fraction of top speed
/// (turn is scaled by traction directly, which bottoms out lower). Damaged mobility should sting,
/// not immobilize — a thrown track is what stops you.
const DAMAGED_SPEED_FLOOR: f32 = 0.75;
/// One thrown track: the live side still drives the hull, but only this fraction of throttle
/// reaches the ground — a slow, deliberate crawl that still lets you reposition or line up a shot.
const BROKEN_ONE_THROTTLE_SCALE: f32 = 0.55;
/// ...and it turns this sluggishly on the one good track.
const BROKEN_ONE_TURN_SCALE: f32 = 0.6;
/// Under power a one-track hull drifts toward the dead side by this much steer — small enough that
/// a counter-steer overrides it, so the hull stays drivable where you point it.
const BROKEN_ONE_DRIFT_BIAS: f32 = 0.18;
/// Throttle magnitude below this reads as "no drive input": no drift bias is applied, so a hull
/// with one thrown track sits still at rest instead of pivoting in place forever.
const DRIVE_INPUT_EPS: f32 = 0.05;

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

/// The world a hull is driven against for one tick.
#[derive(Debug, Clone, Copy)]
pub struct TankDriveWorld<'a> {
    pub heightmap: Option<&'a HeightMap>,
    pub cover: &'a [StaticCoverObject],
    pub tank_obstacles: &'a [TankObstacle],
    /// Running-gear contact stations for the support envelope (trench bridging, crest overhang).
    /// `None` keeps the legacy centre-probe ride height.
    pub footprint: Option<&'a game_core::ContactFootprint>,
    /// The map's standing water (wading drag + riverbed traction cut); `None` is a dry map.
    pub water: Option<terrain::WaterBody>,
}

/// Advance one fixed tick: movement (terrain + cover + tank collision), turret/gun aiming, and
/// command bloom — each gated by module health. Aim-dispersion recovery and firing are the
/// caller's responsibility. `command` must already be clamped. The returned [`GroundStep`]
/// reports whether the hull ended the tick on the ground and how hard a landing it absorbed —
/// the server turns hard landings into fall damage, the client into a camera slam.
pub fn step_tank_drive(
    drive: &mut TankDriveState,
    spec: &TankSpec,
    modules: DriveModuleStatus,
    world: TankDriveWorld<'_>,
    command: TankCommand,
    dt: f32,
) -> GroundStep {
    let ground = if modules.tracks.any_ok() {
        let mut settings = TankControllerSettings::from_spec(spec);
        // Partial module damage shapes the drive continuously: a wounded engine delivers less
        // P/v power (slower launch, lower top-speed equilibrium, weaker climbs — all emergent),
        // a wounded suspension turns mushy. Healthy fractions are exactly 1.0, so the healthy
        // path stays bit-identical (replay-locked).
        settings.drive_power_mps3 *= modules.engine_power_fraction;
        settings.turn_rate_rad_s *= modules.suspension_agility;
        settings.yaw_accel_rad_s2 *= modules.suspension_agility;
        // A dead engine removes drive; damaged/thrown tracks shape the rest.
        let (mut throttle, mut steer) =
            if modules.engine_ok { (command.throttle, command.steer) } else { (0.0, 0.0) };
        if modules.tracks.both_ok() {
            // Both sides rolling. A Damaged pool only dulls agility (and shaves a little top
            // speed); it never biases the steer, so the hull still drives exactly where it is
            // pointed and can be a hair twitchy but never squirrelly. Both healthy → traction is
            // exactly 1.0, so every factor below is 1.0 and the healthy drive stays bit-identical
            // (replay-locked). One light hit is imperceptible; two damaged sides compound.
            let traction = modules.tracks.rolling_traction();
            settings.turn_rate_rad_s *= traction;
            settings.yaw_accel_rad_s2 *= traction;
            settings.drive_power_mps3 *=
                DAMAGED_SPEED_FLOOR + (1.0 - DAMAGED_SPEED_FLOOR) * traction;
        } else {
            // Exactly one side thrown (the both-thrown case is the hard stop below). The live side
            // still drives, so the hull crawls — slow, but fully controllable and able to STOP.
            // The working track pushes the hull around the dead one, so UNDER POWER it drifts
            // toward the dead side. The drift is small, gated on real throttle input (so a
            // released hull sits still instead of pivoting forever — the old idle-spin bug), and
            // simply ADDED to the player's steer so a counter-steer overrides it.
            throttle *= BROKEN_ONE_THROTTLE_SCALE;
            settings.turn_rate_rad_s *= BROKEN_ONE_TURN_SCALE;
            settings.yaw_accel_rad_s2 *= BROKEN_ONE_TURN_SCALE;
            if command.throttle.abs() > DRIVE_INPUT_EPS {
                let dead_sign = if modules.tracks.right_ok() { -1.0 } else { 1.0 };
                let drift = BROKEN_ONE_DRIFT_BIAS * dead_sign * command.throttle.signum();
                steer = (steer + drift).clamp(-1.0, 1.0);
            }
        }
        let input = TankControlInput { throttle, steer, brake: command.brake };
        let obstacles = TankWorldObstacles::new(
            world.cover,
            TankFootprint::from_hitbox(spec.hitbox),
            world.tank_obstacles,
        )
        .with_water(world.water);
        step_tank_on_world_with_tanks(
            &mut drive.kinematic,
            input,
            &settings,
            world.heightmap,
            obstacles,
            world.footprint,
            dt,
        )
    } else {
        // A thrown track removes all hull motion, linear and angular.
        drive.kinematic.velocity = glam::Vec3::ZERO;
        drive.kinematic.yaw_rate_rad_s = 0.0;
        GroundStep::resting()
    };

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
    ground
}

pub(crate) fn step_tank(
    tank: &mut TankState,
    command: TankCommand,
    dt: f32,
    heightmap: Option<&HeightMap>,
    cover: &[StaticCoverObject],
    tank_obstacles: &[TankObstacle],
    water: Option<terrain::WaterBody>,
) -> GroundStep {
    let mut drive = TankDriveState {
        kinematic: TankKinematicState {
            position: tank.position,
            velocity: tank.velocity_mps,
            yaw_rad: tank.yaw_rad,
            yaw_rate_rad_s: tank.hull_yaw_velocity_rad_s,
            pitch_rad: tank.hull_pitch_rad,
            roll_rad: tank.hull_roll_rad,
        },
        aiming: AimingState {
            turret_yaw_rad: tank.turret_yaw_rad,
            turret_yaw_velocity_rad_s: tank.turret_yaw_velocity_rad_s,
            gun_pitch_rad: tank.gun_pitch_rad,
        },
        aim_dispersion_mrad: tank.aim_dispersion_mrad,
    };
    let suspension_ok = tank.modules.is_functional(ModuleSlot::Suspension);
    let tracks = if suspension_ok {
        TrackDriveStatus::from_track_health(&tank.tracks)
    } else {
        TrackDriveStatus::broken()
    };
    let modules =
        DriveModuleStatus::from_module_hp(tracks, tank.modules.hit_points_by_slot(), &tank.spec);
    let footprint = tank.spec.contact_footprint();
    let world =
        TankDriveWorld { heightmap, cover, tank_obstacles, footprint: Some(&footprint), water };

    let ground = step_tank_drive(&mut drive, &tank.spec, modules, world, command, dt);

    tank.position = drive.kinematic.position;
    tank.yaw_rad = drive.kinematic.yaw_rad;
    tank.velocity_mps = drive.kinematic.velocity;
    tank.hull_yaw_velocity_rad_s = drive.kinematic.yaw_rate_rad_s;
    tank.hull_pitch_rad = drive.kinematic.pitch_rad;
    tank.hull_roll_rad = drive.kinematic.roll_rad;
    tank.turret_yaw_rad = drive.aiming.turret_yaw_rad;
    tank.turret_yaw_velocity_rad_s = drive.aiming.turret_yaw_velocity_rad_s;
    tank.gun_pitch_rad = drive.aiming.gun_pitch_rad;
    tank.aim_dispersion_mrad = drive.aim_dispersion_mrad;
    ground
}
