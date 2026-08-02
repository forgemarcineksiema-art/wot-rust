use game_core::TeamId;
use sim::{ShellTraceWorld, TraceOutcome, TraceTank, trace_shell};

use super::*;
use crate::bots::test_support::tank_with_mask;

pub(super) fn tank(id: u64, team: TeamId, position: Vec3) -> TankState {
    tank_with_mask(id, team, position, u8::MAX)
}

/// The honesty lock: lay the gun exactly on the solved solution and fly the authoritative tracer.
#[test]
fn the_solved_arc_lands_on_the_target_hull_at_range() {
    let shooter = tank(1, TeamId(1), Vec3::ZERO);
    let target = tank(2, TeamId(2), Vec3::new(0.0, 0.0, 380.0));
    let aim = solve_firing_solution(&shooter, &target).errors(&shooter);

    let muzzle = shooter.muzzle_world_position();
    let aim_point = target.position + Vec3::Y * target.spec.hitbox.center_y_m;
    let direct_pitch = ((aim_point.y - muzzle.y) / 380.0).atan();
    let solved_pitch = shooter.gun_pitch_rad + aim.pitch_error;
    assert!(
        solved_pitch > direct_pitch + 1.0e-4,
        "ballistic pitch {solved_pitch} must compensate drop over direct {direct_pitch}"
    );

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
    let world = ShellTraceWorld {
        projectile_radius_m: shell.collision_radius_m(),
        tanks: &targets,
        blockers: &[],
        heightmap: None,
        cover: &[],
        water: None,
    };
    match trace_shell(
        aimed.muzzle_world_position(),
        velocity,
        shell.drag_per_s(),
        SOLVE_DT_S,
        SHELL_MAX_AGE_SECONDS,
        &world,
    ) {
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

/// Zero target velocity is the stationary contract exactly, including hull-attitude pullback.
#[test]
fn zero_velocity_preserves_the_stationary_lay() {
    let mut shooter = tank(1, TeamId(1), Vec3::new(10.0, 2.0, -5.0));
    shooter.hull_pitch_rad = 0.08;
    shooter.hull_roll_rad = -0.05;
    shooter.turret_yaw_rad = 0.2;
    shooter.gun_pitch_rad = -0.02;
    let target = tank(2, TeamId(2), Vec3::new(70.0, 7.0, 310.0));
    let solution = solve_firing_solution(&shooter, &target);

    let muzzle = shooter.muzzle_world_position();
    let center = target.position + Vec3::Y * target.spec.hitbox.center_y_m;
    let shell = shooter.selected_shell();
    let lay = lay_to_point(muzzle, center, shell.muzzle_velocity_mps, shell.drag_per_s());
    let delta = center - muzzle;
    let local =
        shooter.hull_pose().basis().transpose() * gun_direction(delta.x.atan2(delta.z), lay.pitch);
    let expected_turret = local.x.atan2(local.z);
    // The shooter's OWN arc — the fleet constant is gone.
    let (min_pitch, max_pitch) = shooter.spec.gun_pitch_limits_rad();
    let expected_pitch = local.y.clamp(-1.0, 1.0).asin().clamp(min_pitch, max_pitch);
    let errors = solution.errors(&shooter);

    assert_eq!(solution.aim_point_world(), center);
    assert!(
        (errors.turret_error - wrap_angle(expected_turret - shooter.turret_yaw_rad)).abs() < 1.0e-6
    );
    assert!((errors.pitch_error - (expected_pitch - shooter.gun_pitch_rad)).abs() < 1.0e-6);
    let distance = delta.length().max(1.0);
    assert!(
        (errors.yaw_gate - (target.spec.hitbox.half_width_m * GATE_MARGIN / distance).atan()).abs()
            < 1.0e-7
    );
}

/// Angular acceptance follows the target's apparent size, not a range-independent constant.
#[test]
fn the_fire_gate_tightens_with_range() {
    let shooter = tank(1, TeamId(1), Vec3::ZERO);
    let far = tank(2, TeamId(2), Vec3::new(0.0, 0.0, 380.0));
    let near = tank(3, TeamId(2), Vec3::new(0.0, 0.0, 40.0));

    let far_aim = solve_firing_solution(&shooter, &far).errors(&shooter);
    let near_aim = solve_firing_solution(&shooter, &near).errors(&shooter);
    assert!(
        far_aim.yaw_gate < 0.01,
        "at 380 m the hull subtends a few mrad, got {}",
        far_aim.yaw_gate
    );
    assert!(near_aim.yaw_gate > far_aim.yaw_gate * 5.0);

    let mut sloppy = shooter.clone();
    sloppy.turret_yaw_rad = 0.015;
    assert!(!solve_firing_solution(&sloppy, &far).errors(&sloppy).on_target());
    assert!(solve_firing_solution(&sloppy, &near).errors(&sloppy).on_target());
}
