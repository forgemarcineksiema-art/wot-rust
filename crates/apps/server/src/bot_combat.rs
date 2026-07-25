//! Bot combat selection and the stand-and-shoot command.
//!
//! Target selection is the only place a bot fires its own LOS raycasts. Cheap gates run first
//! and a candidate earns a raycast only when it can beat the nearest visible target so far.

use game_core::TankId;
use game_core::math::{segment_box_entry, world_to_tank_local};
use glam::Vec3;
use sim::{TankCommand, TankState, VIEW_RANGE_M};

use crate::bot_aim::BotFiringSolution;

/// Nearest enemy this bot may engage: team-spotted, in range and in its own line of sight.
pub(crate) fn bot_nearest_engageable_enemy<'a>(
    tank: &TankState,
    tanks: &'a [TankState],
    exclude: Option<TankId>,
    heightmap: Option<&terrain::HeightMap>,
    cover: &[terrain::StaticCoverObject],
) -> Option<&'a TankState> {
    let mut best: Option<(&TankState, f32)> = None;
    for target in tanks {
        if Some(target.id) == exclude
            || target.team == tank.team
            || target.hit_points == 0
            || target.spotted_mask & tank.team.spotting_bit() == 0
        {
            continue;
        }
        let d2 = tank.position.distance_squared(target.position);
        if d2 > VIEW_RANGE_M * VIEW_RANGE_M {
            continue;
        }
        if best.is_some_and(|(_, best_d2)| d2 >= best_d2) {
            continue;
        }
        if sim::tank_line_of_sight(tank, target, heightmap, cover) {
            best = Some((target, d2));
        }
    }
    best.map(|(target, _)| target)
}

/// Cheap between-reselection gate; full LOS is rechecked on the next selection cadence.
pub(crate) fn bot_target_still_engageable(tank: &TankState, target: &TankState) -> bool {
    target.team != tank.team
        && target.hit_points > 0
        && target.spotted_mask & tank.team.spotting_bit() != 0
        && tank.position.distance_squared(target.position) <= VIEW_RANGE_M * VIEW_RANGE_M
}

pub(crate) fn find_tank(tanks: &[TankState], id: TankId) -> Option<&TankState> {
    tanks.iter().find(|tank| tank.id == id)
}

/// Track the cached ballistic solution; fire only when the lay and friendly-fire gates agree.
pub(crate) fn bot_combat_command(
    tank: &TankState,
    solution: &BotFiringSolution,
    tanks: &[TankState],
) -> TankCommand {
    let aim = solution.errors(tank);
    let fire = aim.on_target()
        && tank.reload_remaining_s <= 0.0
        && find_tank(tanks, solution.target).is_some_and(|target| {
            !ally_blocks_fire_line(tank, target, solution.aim_point_world(), tanks)
        });
    TankCommand {
        throttle: 0.0,
        steer: 0.0,
        brake: 0.35,
        turret_yaw_delta: (aim.turret_error * 4.0).clamp(-1.0, 1.0),
        gun_pitch_delta: (aim.pitch_error * 4.0).clamp(-1.0, 1.0),
        fire,
        select_ammo: None,
    }
}

/// Whether a living teammate's hull occupies the line to the solved intercept point.
///
/// The straight segment approximates the arc; allies that matter are close enough that drop is
/// centimetres. Wrecks still absorb shells, but only living allies hold the trigger.
pub(crate) fn ally_blocks_fire_line(
    tank: &TankState,
    target: &TankState,
    aim_point: Vec3,
    tanks: &[TankState],
) -> bool {
    let muzzle = tank.muzzle_world_position();
    tanks.iter().any(|ally| {
        if ally.id == tank.id
            || ally.id == target.id
            || ally.team != tank.team
            || ally.hit_points == 0
        {
            return false;
        }
        let hitbox = &ally.spec.hitbox;
        let start = world_to_tank_local(muzzle, ally.position, hitbox.center_y_m, ally.hull_pose());
        let end =
            world_to_tank_local(aim_point, ally.position, hitbox.center_y_m, ally.hull_pose());
        let half = Vec3::new(hitbox.half_width_m, hitbox.half_height_m, hitbox.half_length_m);
        segment_box_entry(start, end, -half, half).is_some()
    })
}

#[cfg(test)]
#[path = "bot_combat_tests.rs"]
mod tests;
