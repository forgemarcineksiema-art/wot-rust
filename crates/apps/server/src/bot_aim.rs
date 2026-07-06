//! The bots' gunnery: a ballistic aim solution on the shared shell integrator plus a fire gate
//! scaled by what the target actually subtends. Split from `bots.rs` (roster, targeting, unstuck)
//! for the reviewability budget.
//!
//! Two honesty rules. The pitch solve flies [`game_core::math::integrate_shell_step`] â€” the exact
//! per-tick step the authoritative sim applies to a live shell â€” so the solved arc IS the arc the
//! server will fly; there is no private bot ballistics to drift from the game's. And the fire
//! gate is the angle the target's hull actually subtends at this range: the old flat 0.08 rad
//! gate meant "up to 32 m wide of the mark at 400 m", which turned long-range bot fire into
//! terrain decoration.

use game_core::math::{gun_direction, integrate_shell_step, wrap_angle};
use glam::Vec3;
use sim::{MAX_GUN_PITCH_RAD, MIN_GUN_PITCH_RAD, SHELL_MAX_AGE_SECONDS, TankState};

/// The solve steps with the server's own 60 Hz tick, matching the live shell integration exactly.
const SOLVE_DT_S: f32 = 1.0 / 60.0;
/// Fixed-point refinements of the drop compensation. Each round folds the measured miss at the
/// target's range back into the pitch; drop varies slowly with pitch at tank-gun velocities, so
/// a few rounds settle far below the fire gate's tolerance.
const SOLVE_ROUNDS: usize = 4;
/// The fire gate accepts aim inside this fraction of the target's half-extents â€” the middle of
/// the silhouette, leaving the rim as margin for dispersion.
const GATE_MARGIN: f32 = 0.8;

/// How far the current gun lay is from the solved firing solution, plus the angular gate the
/// target subtends. `on_target` compares the two; the command layer turns the errors into
/// turret/gun rate commands.
#[derive(Debug, Clone, Copy)]
pub(crate) struct BotAimErrors {
    pub turret_error: f32,
    pub pitch_error: f32,
    pub yaw_gate: f32,
    pub pitch_gate: f32,
}

impl BotAimErrors {
    pub(crate) fn on_target(&self) -> bool {
        self.turret_error.abs() < self.yaw_gate && self.pitch_error.abs() < self.pitch_gate
    }
}

/// A solved firing lay in ABSOLUTE hull-frame angles, cached on the bot between solves. The
/// ballistic integration is the expensive half (it flies the shared shell step), so the bot
/// recomputes it on a cadence; the per-tick half — how far the turret still has to slew —
/// is [`Self::errors`], pure arithmetic against the live gun state. Storing absolutes (not
/// errors) is what makes the cache honest: the errors shrink as the turret slews even while
/// the solution itself stands still.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct BotFiringSolution {
    /// Who this lay was solved against — a target switch invalidates the cache.
    pub target: game_core::TankId,
    desired_turret_yaw_rad: f32,
    desired_gun_pitch_rad: f32,
    yaw_gate: f32,
    pitch_gate: f32,
}

impl BotFiringSolution {
    /// The remaining lay error against the live gun state, with the solved gates.
    pub(crate) fn errors(&self, tank: &TankState) -> BotAimErrors {
        BotAimErrors {
            turret_error: wrap_angle(self.desired_turret_yaw_rad - tank.turret_yaw_rad),
            pitch_error: self.desired_gun_pitch_rad - tank.gun_pitch_rad,
            yaw_gate: self.yaw_gate,
            pitch_gate: self.pitch_gate,
        }
    }
}

/// The full firing solution against a live target: ballistic drop compensated on the shared
/// integrator, pulled back through the hull attitude so a pitched or rolled hull still lays the
/// barrel on the point, gated by the target's real angular size.
pub(crate) fn solve_firing_solution(tank: &TankState, target: &TankState) -> BotFiringSolution {
    let muzzle = tank.muzzle_world_position();
    let aim_point = target.position + Vec3::Y * target.spec.hitbox.center_y_m;
    let delta = aim_point - muzzle;
    let flat = Vec3::new(delta.x, 0.0, delta.z).length().max(1.0);
    let shell = tank.selected_shell();
    let world_pitch = ballistic_pitch(shell.muzzle_velocity_mps, shell.drag_per_s(), flat, delta.y);
    let world_yaw = delta.x.atan2(delta.z);
    // The live shell leaves along hull_basis * gun_direction(turret, pitch): pull the desired
    // world direction back into the hull frame so the lay stays true on slopes and side tilts.
    let local = tank.hull_pose().basis().transpose() * gun_direction(world_yaw, world_pitch);
    let desired_turret = local.x.atan2(local.z);
    let desired_pitch = local.y.clamp(-1.0, 1.0).asin().clamp(MIN_GUN_PITCH_RAD, MAX_GUN_PITCH_RAD);
    let distance = delta.length().max(1.0);
    BotFiringSolution {
        target: target.id,
        desired_turret_yaw_rad: desired_turret,
        desired_gun_pitch_rad: desired_pitch,
        yaw_gate: (target.spec.hitbox.half_width_m * GATE_MARGIN / distance).atan(),
        pitch_gate: (target.spec.hitbox.half_height_m * GATE_MARGIN / distance).atan(),
    }
}

/// The world-frame gun pitch that lands the shared-integrator arc `rise_m` above the muzzle at
/// `flat_m` downrange. Starts from the straight line and folds each round's measured miss back
/// into the pitch; a target out of ballistic reach falls back to the straight line (the route
/// brain is meanwhile closing the distance).
fn ballistic_pitch(muzzle_velocity_mps: f32, drag_per_s: f32, flat_m: f32, rise_m: f32) -> f32 {
    let direct = (rise_m / flat_m).atan();
    let mut pitch = direct;
    for _ in 0..SOLVE_ROUNDS {
        let Some(height) = arc_height_at_range(muzzle_velocity_mps, drag_per_s, pitch, flat_m)
        else {
            return direct;
        };
        pitch += ((rise_m - height) / flat_m).atan();
    }
    pitch
}

/// Fly the shared shell step in the shot's vertical plane and report the arc's height (relative
/// to the muzzle) where it crosses `flat_m` downrange; `None` if the shell dies first.
fn arc_height_at_range(
    muzzle_velocity_mps: f32,
    drag_per_s: f32,
    pitch: f32,
    flat_m: f32,
) -> Option<f32> {
    let mut position = Vec3::ZERO;
    let mut velocity = Vec3::new(pitch.cos(), pitch.sin(), 0.0) * muzzle_velocity_mps;
    let mut age = 0.0;
    while age < SHELL_MAX_AGE_SECONDS {
        let previous = position;
        integrate_shell_step(&mut velocity, drag_per_s, SOLVE_DT_S);
        position += velocity * SOLVE_DT_S;
        age += SOLVE_DT_S;
        if position.x >= flat_m {
            let t = (flat_m - previous.x) / (position.x - previous.x).max(1.0e-6);
            return Some(previous.y + (position.y - previous.y) * t);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use game_core::TeamId;
    use sim::{ShellTraceWorld, TraceOutcome, TraceTank, trace_shell};

    use super::*;

    fn tank(id: u64, team: TeamId, position: Vec3) -> TankState {
        let spec = game_core::VehicleKind::T54_1951.spec();
        let modules = spec.module_health;
        let ammo_counts = spec.ammo.counts;
        let selected_ammo = spec.ammo.initial_selected;
        TankState {
            id: game_core::TankId(id),
            team,
            hit_points: spec.hit_points,
            spec,
            position,
            yaw_rad: 0.0,
            turret_yaw_rad: 0.0,
            turret_yaw_velocity_rad_s: 0.0,
            gun_pitch_rad: 0.0,
            velocity_mps: Vec3::ZERO,
            hull_yaw_velocity_rad_s: 0.0,
            hull_pitch_rad: 0.0,
            hull_roll_rad: 0.0,
            reload_remaining_s: 0.0,
            aim_dispersion_mrad: 0.0,
            dispersion_shot_index: 0,
            tracks: game_core::TrackDamageMask::healthy(),
            modules,
            ammo_counts,
            selected_ammo,
            spotted_mask: u8::MAX,
        }
    }

    /// The honesty lock: lay the gun exactly on the solved solution, fire the shared tracer
    /// along it, and the shell must strike the target tank near the aim point â€” at a range where
    /// gravity drop is a miss if uncompensated. Solver, muzzle chain and tracer all share the
    /// sim's own math, so this holds by construction until someone forks one of them.
    #[test]
    fn the_solved_arc_lands_on_the_target_hull_at_range() {
        let shooter = tank(1, TeamId(1), Vec3::ZERO);
        let target = tank(2, TeamId(2), Vec3::new(0.0, 0.0, 380.0));
        let aim = solve_firing_solution(&shooter, &target).errors(&shooter);

        // Drop compensation is real: the solved pitch sits ABOVE the straight line to the point.
        let muzzle = shooter.muzzle_world_position();
        let aim_point = target.position + Vec3::Y * target.spec.hitbox.center_y_m;
        let direct_pitch = ((aim_point.y - muzzle.y) / 380.0).atan();
        let solved_pitch = shooter.gun_pitch_rad + aim.pitch_error;
        assert!(
            solved_pitch > direct_pitch + 1.0e-4,
            "ballistic pitch {solved_pitch} must compensate drop over direct {direct_pitch}"
        );

        // Fire the authoritative tracer along the solved lay: it must hit the target's hull.
        let mut aimed = shooter.clone();
        aimed.turret_yaw_rad += aim.turret_error;
        aimed.gun_pitch_rad += aim.pitch_error;
        let shell = aimed.selected_shell();
        let velocity = (aimed.hull_pose().basis()
            * gun_direction(aimed.turret_yaw_rad, aimed.gun_pitch_rad))
            * shell.muzzle_velocity_mps;
        let targets = [TraceTank::from_spec(
            target.id,
            target.position,
            target.hull_pose(),
            target.turret_yaw_rad,
            &target.spec,
        )];
        let world = ShellTraceWorld { tanks: &targets, blockers: &[], heightmap: None, cover: &[] };
        let outcome = trace_shell(
            aimed.muzzle_world_position(),
            velocity,
            shell.drag_per_s(),
            SOLVE_DT_S,
            SHELL_MAX_AGE_SECONDS,
            &world,
        );
        match outcome {
            TraceOutcome::Tank { id, hit_position, .. } => {
                assert_eq!(id, target.id);
                assert!(
                    (hit_position.y - aim_point.y).abs() <= target.spec.hitbox.half_height_m,
                    "the arc should land near the aim height, hit {hit_position:?}"
                );
            }
            other => panic!("the solved arc must strike the target, got {other:?}"),
        }
    }

    /// The fire gate scales with range: an aim error that the old flat 0.08 rad gate would have
    /// fired on (missing by many meters at distance) must hold fire, and the gate the target
    /// subtends up close stays generous so brawls do not stall waiting for a perfect lay.
    #[test]
    fn the_fire_gate_tightens_with_range() {
        let shooter = tank(1, TeamId(1), Vec3::ZERO);
        let far = tank(2, TeamId(2), Vec3::new(0.0, 0.0, 380.0));
        let near = tank(3, TeamId(2), Vec3::new(0.0, 0.0, 40.0));

        let far_aim = solve_firing_solution(&shooter, &far).errors(&shooter);
        let near_aim = solve_firing_solution(&shooter, &near).errors(&shooter);

        assert!(
            far_aim.yaw_gate < 0.01,
            "at 380 m the hull subtends a few mrad, not 0.08 rad (got {})",
            far_aim.yaw_gate
        );
        assert!(near_aim.yaw_gate > far_aim.yaw_gate * 5.0, "the gate must open up close");

        // A 0.015 rad lay error is a 5.7 m miss at 380 m (the old flat gate fired anyway) but
        // lands 0.6 m off center at 40 m â€” well inside the hull. Range decides, not a constant.
        let mut sloppy = shooter.clone();
        sloppy.turret_yaw_rad = 0.015;
        assert!(
            !solve_firing_solution(&sloppy, &far).errors(&sloppy).on_target(),
            "5.7 m wide at 380 m: hold fire"
        );
        assert!(
            solve_firing_solution(&sloppy, &near).errors(&sloppy).on_target(),
            "0.6 m wide at 40 m: certain hit"
        );
    }
}
