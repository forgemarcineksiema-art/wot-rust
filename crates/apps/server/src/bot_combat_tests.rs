use game_core::{TankId, TeamId};
use glam::Vec3;

use super::*;
use crate::bots::test_support::tank_with_mask as tank;

#[test]
fn bots_target_only_enemies_spotted_by_their_team() {
    let observer = tank(1, TeamId(1), Vec3::new(300.0, 0.0, 300.0), TeamId(1).spotting_bit());
    let enemy = tank(2, TeamId(2), Vec3::new(305.0, 0.0, 305.0), TeamId(2).spotting_bit());
    assert!(
        bot_nearest_engageable_enemy(&observer, &[observer.clone(), enemy], None, None, &[])
            .is_none()
    );
}

#[test]
fn bots_engage_only_enemies_in_their_own_line_of_sight() {
    let observer = tank(1, TeamId(1), Vec3::new(300.0, 0.0, 300.0), TeamId(1).spotting_bit());
    let mask = TeamId(2).spotting_bit() | TeamId(1).spotting_bit();
    let near_enemy = tank(2, TeamId(2), Vec3::new(300.0, 0.0, 360.0), mask);
    let far_enemy = tank(3, TeamId(2), Vec3::new(360.0, 0.0, 300.0), mask);
    let tanks = [observer.clone(), near_enemy, far_enemy];
    let wall = terrain::StaticCoverObject {
        id: "wall".into(),
        name: "wall".into(),
        kind: terrain::StaticCoverKind::FarmBuilding,
        center: [300.0, 5.0, 330.0],
        half_extents_m: [20.0, 10.0, 2.0],
    };

    let blocked =
        bot_nearest_engageable_enemy(&observer, &tanks, None, None, std::slice::from_ref(&wall));
    assert_eq!(blocked.map(|target| target.id), Some(TankId(3)));
    let clear = bot_nearest_engageable_enemy(&observer, &tanks, None, None, &[]);
    assert_eq!(clear.map(|target| target.id), Some(TankId(2)));
}

#[test]
fn the_trigger_waits_for_a_clear_fire_line() {
    let mask = TeamId(1).spotting_bit() | TeamId(2).spotting_bit();
    let mut shooter = tank(1, TeamId(1), Vec3::ZERO, mask);
    let target = tank(2, TeamId(2), Vec3::new(0.0, 0.0, 90.0), mask);
    let ally_in_line = tank(3, TeamId(1), Vec3::new(0.0, 0.0, 45.0), mask);
    let mut ally_clear = ally_in_line.clone();
    ally_clear.position.x = 8.0;

    let solution = crate::bot_aim::solve_firing_solution(&shooter, &target);
    let errors = solution.errors(&shooter);
    shooter.turret_yaw_rad += errors.turret_error;
    shooter.gun_pitch_rad += errors.pitch_error;
    assert!(solution.errors(&shooter).on_target());

    let blocked = [shooter.clone(), target.clone(), ally_in_line];
    assert!(!bot_combat_command(&shooter, &solution, &blocked).fire);
    let clear = [shooter.clone(), target, ally_clear];
    assert!(bot_combat_command(&shooter, &solution, &clear).fire);
}

/// The friendly-fire gate must follow the same lateral lead as the gun.
#[test]
fn the_trigger_checks_teammates_on_the_lead_line() {
    let mask = TeamId(1).spotting_bit() | TeamId(2).spotting_bit();
    let mut shooter = tank(1, TeamId(1), Vec3::ZERO, mask);
    let mut target = tank(2, TeamId(2), Vec3::new(0.0, 0.0, 350.0), mask);
    target.velocity_mps = Vec3::new(target.spec.max_forward_speed_mps, 0.0, 0.0);
    let solution = crate::bot_aim::solve_firing_solution(&shooter, &target);

    let lead_point = solution.aim_point_world();
    let raw_point = target.position + Vec3::Y * target.spec.hitbox.center_y_m;
    let midpoint = shooter.muzzle_world_position().lerp(lead_point, 0.5);
    let ally = tank(3, TeamId(1), Vec3::new(midpoint.x, 0.0, midpoint.z), mask);
    let tanks = [shooter.clone(), target.clone(), ally];
    assert!(
        !ally_blocks_fire_line(&shooter, &target, raw_point, &tanks),
        "test premise: the obsolete current-position ray misses the teammate"
    );
    assert!(ally_blocks_fire_line(&shooter, &target, lead_point, &tanks));

    let errors = solution.errors(&shooter);
    shooter.turret_yaw_rad += errors.turret_error;
    shooter.gun_pitch_rad += errors.pitch_error;
    let tanks = [shooter.clone(), target, tanks[2].clone()];
    assert!(!bot_combat_command(&shooter, &solution, &tanks).fire);
}

#[test]
fn only_living_allies_between_muzzle_and_target_block_the_line() {
    let mask = TeamId(1).spotting_bit() | TeamId(2).spotting_bit();
    let shooter = tank(1, TeamId(1), Vec3::ZERO, mask);
    let target = tank(2, TeamId(2), Vec3::new(0.0, 0.0, 90.0), mask);
    let aim_point = target.position + Vec3::Y * target.spec.hitbox.center_y_m;

    let behind = tank(3, TeamId(1), Vec3::new(0.0, 0.0, 130.0), mask);
    assert!(!ally_blocks_fire_line(
        &shooter,
        &target,
        aim_point,
        &[shooter.clone(), target.clone(), behind]
    ));

    let mut wreck = tank(4, TeamId(1), Vec3::new(0.0, 0.0, 45.0), mask);
    wreck.hit_points = 0;
    assert!(!ally_blocks_fire_line(
        &shooter,
        &target,
        aim_point,
        &[shooter.clone(), target.clone(), wreck]
    ));

    let enemy_between = tank(5, TeamId(2), Vec3::new(0.0, 0.0, 45.0), mask);
    assert!(!ally_blocks_fire_line(
        &shooter,
        &target,
        aim_point,
        &[shooter.clone(), target.clone(), enemy_between]
    ));
}

#[test]
fn cached_targets_drop_the_moment_the_cheap_gates_fail() {
    let observer = tank(1, TeamId(1), Vec3::new(300.0, 0.0, 300.0), TeamId(1).spotting_bit());
    let mask = TeamId(2).spotting_bit() | TeamId(1).spotting_bit();
    let live = tank(2, TeamId(2), Vec3::new(320.0, 0.0, 300.0), mask);
    assert!(bot_target_still_engageable(&observer, &live));

    let mut dead = live.clone();
    dead.hit_points = 0;
    assert!(!bot_target_still_engageable(&observer, &dead));
    let mut unspotted = live.clone();
    unspotted.spotted_mask = TeamId(2).spotting_bit();
    assert!(!bot_target_still_engageable(&observer, &unspotted));
    let mut far = live;
    far.position = Vec3::new(300.0 + VIEW_RANGE_M + 10.0, 0.0, 300.0);
    assert!(!bot_target_still_engageable(&observer, &far));
}
