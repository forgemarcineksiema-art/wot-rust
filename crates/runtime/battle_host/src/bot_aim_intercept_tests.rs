use game_core::TeamId;
use sim::{SegmentImpact, ShellTraceWorld, TraceTank, segment_impact};

use super::tests::tank;
use super::*;

/// Fly in target-relative space. Both segment endpoints and velocity are relative, preserving
/// the authoritative narrow-phase geometry and impact direction for constant target motion.
fn moving_target_hit_time(shooter: &TankState, target: &TankState) -> Option<f32> {
    let shell = shooter.selected_shell();
    let mut position = shooter.muzzle_world_position();
    let mut velocity = shooter.hull_pose().basis()
        * gun_direction(shooter.turret_yaw_rad, shooter.gun_pitch_rad)
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
        water: terrain::WaterView::DRY,
    };
    let mut age = 0.0;
    while age < SHELL_MAX_AGE_SECONDS {
        let previous = position;
        integrate_shell_step(&mut velocity, shell.drag_per_s(), SOLVE_DT_S);
        position += velocity * SOLVE_DT_S;
        let next_age = age + SOLVE_DT_S;
        let previous_relative = previous - target.velocity_mps * age;
        let current_relative = position - target.velocity_mps * next_age;
        if matches!(
            segment_impact(
                previous_relative,
                current_relative,
                velocity - target.velocity_mps,
                &world
            ),
            Some(SegmentImpact::Tank { id, .. }) if id == target.id
        ) {
            return Some(next_age);
        }
        age = next_age;
    }
    None
}

fn lay_gun(shooter: &TankState, solution: BotFiringSolution) -> TankState {
    let mut aimed = shooter.clone();
    let errors = solution.errors(&aimed);
    aimed.turret_yaw_rad += errors.turret_error;
    aimed.gun_pitch_rad += errors.pitch_error;
    aimed
}

/// A flank-speed T-54 crosses more than a hull width during a 350 m shell flight.
#[test]
fn the_solved_arc_leads_a_lateral_target_at_battle_range() {
    let shooter = tank(1, TeamId(1), Vec3::ZERO);
    let mut target = tank(2, TeamId(2), Vec3::new(0.0, 0.0, 350.0));
    target.velocity_mps = Vec3::new(target.spec.max_forward_speed_mps, 0.0, 0.0);

    let mut stationary = target.clone();
    stationary.velocity_mps = Vec3::ZERO;
    let stationary_lay = lay_gun(&shooter, solve_firing_solution(&shooter, &stationary));
    assert!(
        moving_target_hit_time(&stationary_lay, &target).is_none(),
        "test premise: current-position aim must miss a flank-speed target"
    );

    let solution = solve_firing_solution(&shooter, &target);
    let shell = shooter.selected_shell();
    let predicted_time = ballistic_lay_to_point(
        shooter.muzzle_world_position(),
        solution.aim_point_world(),
        shell.muzzle_velocity_mps,
        shell.drag_per_s(),
    )
    .expect("battle-range intercept is reachable")
    .flight_time_s;
    let hit_time = moving_target_hit_time(&lay_gun(&shooter, solution), &target)
        .expect("the lead must put the authoritative shell through the translating hull");
    assert!(
        (hit_time - predicted_time).abs() <= SOLVE_DT_S * 2.0,
        "hit at {hit_time}s should match predicted {predicted_time}s"
    );
}

#[test]
fn the_intercept_tracks_targets_closing_and_opening_range() {
    let shooter = tank(1, TeamId(1), Vec3::ZERO);
    for velocity_z in [-10.0, 10.0] {
        let mut target = tank(2, TeamId(2), Vec3::new(0.0, 0.0, 350.0));
        target.velocity_mps.z = velocity_z;
        let solution = solve_firing_solution(&shooter, &target);
        assert_eq!(
            (solution.aim_point_world().z - target.position.z).signum(),
            velocity_z.signum()
        );
        assert!(moving_target_hit_time(&lay_gun(&shooter, solution), &target).is_some());
    }
}

#[test]
fn implausible_lead_falls_back_to_a_finite_stationary_lay() {
    let shooter = tank(1, TeamId(1), Vec3::ZERO);
    let mut target = tank(2, TeamId(2), Vec3::new(0.0, 0.0, 350.0));
    target.velocity_mps = Vec3::new(10_000.0, 0.0, 0.0);
    let solution = solve_firing_solution(&shooter, &target);
    let center = target.position + Vec3::Y * target.spec.hitbox.center_y_m;
    assert_eq!(solution.aim_point_world(), center);
    let errors = solution.errors(&shooter);
    assert!(errors.turret_error.is_finite() && errors.pitch_error.is_finite());
}
