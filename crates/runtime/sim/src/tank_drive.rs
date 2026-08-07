use game_core::{ModuleSlot, TankSpec};
use physics::{
    GroundStep, TankControlInput, TankControllerSettings, TankFootprint, TankKinematicState,
    TankWorldObstacles,
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
    /// Running-gear contact stations for the support envelope (trench bridging, crest overhang).
    /// `None` keeps the legacy centre-probe ride height.
    pub footprint: Option<&'a game_core::ContactFootprint>,
    /// The map's standing water (wading drag + riverbed traction cut); `None` is a dry map.
    pub water: Option<terrain::WaterBody>,
    /// Collapsed buildings, as ground (see `terrain::RubbleMound`). Empty until something comes
    /// down, so an untouched battlefield drives bit-identically.
    pub rubble: &'a [terrain::RubbleMound],
    /// The map's ground rule — what the surface under the tracks IS, from the same
    /// `terrain::GroundClassifier` the picture's splat is baked from. `None` is bare grass, which
    /// is the model before material existed.
    pub ground: Option<&'a terrain::GroundClassifier>,
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
    let phase = advance_tank_drive(drive, spec, modules, world, command, dt);
    settle_tank_drive(drive, spec, modules, world, command, phase, dt)
}

/// What the drive phase decided, carried to the settle phase.
#[derive(Debug, Clone, Copy)]
pub struct DrivePhase {
    settings: TankControllerSettings,
    step: physics::TankStepContact,
}

/// PHASE 1: decide the hull's velocity and heading for this tick without moving it, so a caller
/// holding the whole roster can solve hull-to-hull contacts before any of them is spent.
pub fn advance_tank_drive(
    drive: &mut TankDriveState,
    spec: &TankSpec,
    modules: DriveModuleStatus,
    world: TankDriveWorld<'_>,
    command: TankCommand,
    dt: f32,
) -> DrivePhase {
    let (settings, input) = if modules.tracks.any_ok() {
        drive_inputs(spec, modules, command)
    } else {
        // Both tracks thrown. The hull has no drive and no steering — and the shed belts drag
        // under it, which is what the brake channel already models, so it stops within a hull
        // length or two.
        //
        // What it does NOT do is lose the momentum it already had. This used to force the
        // velocity to ZERO every tick, which was invisible while a hull's own drive was the only
        // thing that could put velocity there and became a real hole the moment CONTACT could: a
        // ram throws the victim's track, and from that instant the victim was unpushable — the
        // handbrake of a dead vehicle deleting the shove that had just been handed to it. It also
        // froze a hull thrown in mid-air, since the whole world step was skipped with it.
        // A thrown track removes the DRIVE, not the momentum.
        (
            TankControllerSettings::from_spec(spec),
            TankControlInput { throttle: 0.0, steer: 0.0, brake: 1.0 },
        )
    };
    let obstacles = drive_obstacles(spec, world);
    let step = physics::advance_tank_on_world(
        &mut drive.kinematic,
        input,
        &settings,
        world.heightmap,
        obstacles,
        world.footprint,
        dt,
    );
    DrivePhase { settings, step }
}

/// PHASE 3: spend the velocity the hull ended up with — after any contact solve has had its say —
/// and finish the tick's aiming and bloom.
pub fn settle_tank_drive(
    drive: &mut TankDriveState,
    spec: &TankSpec,
    modules: DriveModuleStatus,
    world: TankDriveWorld<'_>,
    command: TankCommand,
    phase: DrivePhase,
    dt: f32,
) -> GroundStep {
    let ground = physics::settle_tank_on_world(
        &mut drive.kinematic,
        &phase.settings,
        phase.step,
        world.heightmap,
        drive_obstacles(spec, world),
        world.footprint,
        dt,
    );
    finish_tank_drive(drive, spec, modules, command, dt);
    ground
}

fn drive_obstacles<'a>(spec: &TankSpec, world: TankDriveWorld<'a>) -> TankWorldObstacles<'a> {
    TankWorldObstacles::new(world.cover, TankFootprint::from_plan(spec.hull_plan()))
        .with_water(world.water)
        .with_rubble(world.rubble)
        .with_ground(world.ground)
}

/// How hard a hull can actually stop on ground of the given grip scale.
///
/// Braking is grip-limited, so it is the smaller of what the brakes can demand and what the
/// tracks can deliver — the same `mu * g * traction` cap the drive is bounded by. Exposed because
/// anything PLANNING around a stop has to plan with the real number: the bots' water escape reads
/// it to size its approach probe, and a fixed figure under-reached exactly on the slick wet margin
/// where backing out of a channel actually happens.
pub fn braking_deceleration_mps2(spec: &TankSpec, ground_grip_scale: f32) -> f32 {
    let settings = TankControllerSettings::from_spec(spec);
    let track_limit =
        settings.longitudinal_grip_mu * game_core::math::GRAVITY_MPS2 * ground_grip_scale.max(0.0);
    settings.brake_deceleration_mps2.min(track_limit)
}

/// The controller settings and control input one tick of module damage implies.
///
/// Partial module damage shapes the drive continuously: a wounded engine delivers less P/v power
/// (slower launch, lower top-speed equilibrium, weaker climbs — all emergent), a wounded
/// suspension turns mushy. Healthy fractions are exactly 1.0, so the healthy path stays
/// bit-identical (replay-locked).
fn drive_inputs(
    spec: &TankSpec,
    modules: DriveModuleStatus,
    command: TankCommand,
) -> (TankControllerSettings, TankControlInput) {
    let mut settings = TankControllerSettings::from_spec(spec);
    settings.drive_power_mps3 *= modules.engine_power_fraction;
    settings.turn_rate_rad_s *= modules.suspension_agility;
    settings.yaw_accel_rad_s2 *= modules.suspension_agility;
    // A dead engine removes drive; damaged/thrown tracks shape the rest.
    let (mut throttle, steer) =
        if modules.engine_ok { (command.throttle, command.steer) } else { (0.0, 0.0) };
    // What each belt can still put down, whatever tier it is on. Two whole belts are exactly
    // (1.0, 1.0), which forces no turn at all, so the healthy drive stays bit-identical and the
    // fleet's mobility table does not move. Everything below is what a DIFFERENCE between the two
    // does — and a difference is the only thing that turns a tracked hull.
    settings.belts =
        physics::BeltDrive::new(modules.tracks.left.traction(), modules.tracks.right.traction());
    if modules.tracks.both_ok() {
        // Both sides rolling. A Damaged pool dulls agility and shaves a little top speed — and now,
        // if the two sides are drained UNEVENLY, it also pulls, gently, toward the worse belt. That
        // is not a new rule; it is the same belt difference the thrown case uses, applied to a pair
        // that is merely unequal. Both healthy → traction is exactly 1.0 on both sides, so every
        // factor below is 1.0 and nothing pulls. One light hit is imperceptible; two damaged sides
        // compound.
        let traction = modules.tracks.rolling_traction();
        settings.turn_rate_rad_s *= traction;
        settings.yaw_accel_rad_s2 *= traction;
        settings.drive_power_mps3 *= DAMAGED_SPEED_FLOOR + (1.0 - DAMAGED_SPEED_FLOOR) * traction;
    } else {
        // Exactly one side thrown (the both-thrown case is the hard stop in `advance_tank_drive`).
        // The live side still drives, so the hull crawls — slow, but able to STOP.
        //
        // Where it GOES is no longer a bias bolted onto the player's steer. It used to be: a fixed
        // 0.18 added to `steer`, gated on throttle. That made a thrown track a speed tax you could
        // buy your way out of — measured, holding exactly 0.18 of counter-steer drove a T-54 with
        // a dead left track in a dead straight line, at the very same 7.67 m/s it managed with no
        // steer at all, and 2 m FURTHER in ten seconds for not having to arc. A track you can
        // counter-steer away is not damage.
        //
        // Now the dead belt is declared to the drive model and the turn falls out of the belt
        // difference (`BeltDrive::forced_yaw_rate`). Counter-steering can no longer cancel it: the
        // only way to match a dead belt is to slow the live one, which is a real price in speed.
        throttle *= BROKEN_ONE_THROTTLE_SCALE;
        settings.turn_rate_rad_s *= BROKEN_ONE_TURN_SCALE;
        settings.yaw_accel_rad_s2 *= BROKEN_ONE_TURN_SCALE;
    }
    (settings, TankControlInput { throttle, steer, brake: command.brake })
}

/// Turret, gun and bloom — the half of the tick that does not care where the hull ended up.
fn finish_tank_drive(
    drive: &mut TankDriveState,
    spec: &TankSpec,
    modules: DriveModuleStatus,
    command: TankCommand,
    dt: f32,
) {
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

/// Project a tank into the drive step's neutral state.
fn drive_state_of(tank: &TankState) -> TankDriveState {
    TankDriveState {
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
    }
}

pub(crate) fn tank_drive_modules(tank: &TankState) -> DriveModuleStatus {
    let tracks = if tank.modules.is_functional(ModuleSlot::Suspension) {
        TrackDriveStatus::from_track_health(&tank.tracks)
    } else {
        TrackDriveStatus::broken()
    };
    DriveModuleStatus::from_module_hp(tracks, tank.modules.hit_points_by_slot(), &tank.spec)
}

/// PHASE 1 for one tank: advance its drive without moving it, and write the new velocity and
/// heading back so the roster's contact solve can read them.
pub(crate) fn advance_tank(
    tank: &mut TankState,
    command: TankCommand,
    dt: f32,
    world: TankDriveWorld<'_>,
) -> DrivePhase {
    let mut drive = drive_state_of(tank);
    let modules = tank_drive_modules(tank);
    let phase = advance_tank_drive(&mut drive, &tank.spec, modules, world, command, dt);
    tank.velocity_mps = drive.kinematic.velocity;
    tank.yaw_rad = drive.kinematic.yaw_rad;
    tank.hull_yaw_velocity_rad_s = drive.kinematic.yaw_rate_rad_s;
    phase
}

/// PHASE 3 for one tank: spend the velocity that survived the contact solve.
pub(crate) fn settle_tank(
    tank: &mut TankState,
    command: TankCommand,
    dt: f32,
    world: TankDriveWorld<'_>,
    phase: DrivePhase,
) -> GroundStep {
    let mut drive = drive_state_of(tank);
    let modules = tank_drive_modules(tank);
    let ground = settle_tank_drive(&mut drive, &tank.spec, modules, world, command, phase, dt);

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
