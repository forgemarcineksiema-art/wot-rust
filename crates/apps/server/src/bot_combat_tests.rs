use game_core::{TankId, TeamId};
use glam::Vec3;

use super::*;
use crate::bots::test_support::tank_with_mask as tank;

#[test]
fn bots_target_only_enemies_spotted_by_their_team() {
    let observer = tank(1, TeamId(1), Vec3::new(300.0, 0.0, 300.0), TeamId(1).spotting_bit());
    let enemy = tank(2, TeamId(2), Vec3::new(305.0, 0.0, 305.0), TeamId(2).spotting_bit());
    assert!(
        bot_best_engageable_enemy(
            &observer,
            &[observer.clone(), enemy],
            None,
            None,
            &[],
            &BotSituation::default()
        )
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

    let blocked = bot_best_engageable_enemy(
        &observer,
        &tanks,
        None,
        None,
        std::slice::from_ref(&wall),
        &BotSituation::default(),
    );
    assert_eq!(blocked.map(|target| target.id), Some(TankId(3)));
    let clear =
        bot_best_engageable_enemy(&observer, &tanks, None, None, &[], &BotSituation::default());
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

/// FINISHING A CRIPPLE. Two enemies, the nearer one fresh and the farther one nearly dead. The
/// selector used to take the near one every time; a crew takes the kill. Damage already spent on a
/// hull is the cheapest damage left on the field — a half-dead tank that lives shoots as often as
/// a whole one.
#[test]
fn a_bot_finishes_the_cripple_instead_of_the_nearer_healthy_enemy() {
    let mask = TeamId(1).spotting_bit() | TeamId(2).spotting_bit();
    let observer = tank(1, TeamId(1), Vec3::new(300.0, 0.0, 300.0), mask);
    let healthy = tank(2, TeamId(2), Vec3::new(300.0, 0.0, 340.0), mask);
    let mut cripple = tank(3, TeamId(2), Vec3::new(300.0, 0.0, 400.0), mask);
    cripple.hit_points = cripple.spec.hit_points / 10;
    let tanks = [observer.clone(), healthy, cripple];

    let chosen =
        bot_best_engageable_enemy(&observer, &tanks, None, None, &[], &BotSituation::default());

    assert_eq!(
        chosen.map(|target| target.id),
        Some(TankId(3)),
        "the kill outranks the nearer hull"
    );
}

/// ANSWERING INCOMING FIRE. Two identical enemies at the same range, one of them in the quarter a
/// shell just came from. The bot shoots back at that quarter. The bearing is the only thing the
/// crew honestly knows — it never reads the shooter's position.
#[test]
fn a_bot_shoots_back_at_the_quarter_the_shell_came_from() {
    let mask = TeamId(1).spotting_bit() | TeamId(2).spotting_bit();
    let observer = tank(1, TeamId(1), Vec3::new(300.0, 0.0, 300.0), mask);
    let ahead = tank(2, TeamId(2), Vec3::new(300.0, 0.0, 360.0), mask);
    let to_the_left = tank(3, TeamId(2), Vec3::new(240.0, 0.0, 300.0), mask);
    let tanks = [observer.clone(), ahead, to_the_left];

    let struck_from_the_left =
        BotSituation { threat_bearing: Some(-std::f32::consts::FRAC_PI_2), ..Default::default() };
    let chosen =
        bot_best_engageable_enemy(&observer, &tanks, None, None, &[], &struck_from_the_left);
    assert_eq!(chosen.map(|target| target.id), Some(TankId(3)));

    // With no hit to answer, the same two enemies tie on range and the first one stands.
    let calm =
        bot_best_engageable_enemy(&observer, &tanks, None, None, &[], &BotSituation::default());
    assert_eq!(calm.map(|target| target.id), Some(TankId(2)));
}

/// THREAT PRIORITY, concretely: of two enemies at equal range, the one whose gun is already laid
/// on this bot is the one about to fire. Not who is dangerous in the abstract — who is pointing.
#[test]
fn a_gun_already_laid_on_this_bot_outranks_one_looking_elsewhere() {
    let mask = TeamId(1).spotting_bit() | TeamId(2).spotting_bit();
    let observer = tank(1, TeamId(1), Vec3::new(300.0, 0.0, 300.0), mask);
    let mut looking_away = tank(2, TeamId(2), Vec3::new(300.0, 0.0, 360.0), mask);
    looking_away.turret_yaw_rad = 0.0; // facing +Z, straight away from the observer
    let mut laid_on_us = tank(3, TeamId(2), Vec3::new(360.0, 0.0, 300.0), mask);
    laid_on_us.turret_yaw_rad = -std::f32::consts::FRAC_PI_2; // swung onto the observer
    let tanks = [observer.clone(), looking_away, laid_on_us];

    let chosen =
        bot_best_engageable_enemy(&observer, &tanks, None, None, &[], &BotSituation::default());

    assert_eq!(chosen.map(|target| target.id), Some(TankId(3)), "shoot the gun that is pointing");
}

/// FOCUS FIRE. Of two equal enemies, the one a teammate is already fighting. Two bots splitting
/// their fire leave two enemies alive and shooting; concentration is what turns damage into kills.
#[test]
fn bots_concentrate_on_the_enemy_a_teammate_is_already_fighting() {
    let mask = TeamId(1).spotting_bit() | TeamId(2).spotting_bit();
    let observer = tank(1, TeamId(1), Vec3::new(300.0, 0.0, 300.0), mask);
    let alone = tank(2, TeamId(2), Vec3::new(300.0, 0.0, 360.0), mask);
    let already_engaged = tank(3, TeamId(2), Vec3::new(360.0, 0.0, 300.0), mask);
    let tanks = [observer.clone(), alone, already_engaged];
    let ally_on_three = [AlliedEngagement { bot: TankId(9), team: TeamId(1), target: TankId(3) }];

    let supported = BotSituation { allied_targets: &ally_on_three, ..Default::default() };
    let chosen = bot_best_engageable_enemy(&observer, &tanks, None, None, &[], &supported);
    assert_eq!(chosen.map(|target| target.id), Some(TankId(3)));

    // The same engagement from the OTHER team is not support, and this bot's own entry is not
    // support either — otherwise the term would just be hysteresis on its own last choice.
    for irrelevant in [
        AlliedEngagement { bot: TankId(9), team: TeamId(2), target: TankId(3) },
        AlliedEngagement { bot: TankId(1), team: TeamId(1), target: TankId(3) },
    ] {
        let situation = BotSituation {
            allied_targets: std::slice::from_ref(&irrelevant),
            ..Default::default()
        };
        let chosen = bot_best_engageable_enemy(&observer, &tanks, None, None, &[], &situation);
        assert_eq!(chosen.map(|target| target.id), Some(TankId(2)), "{irrelevant:?}");
    }
}
