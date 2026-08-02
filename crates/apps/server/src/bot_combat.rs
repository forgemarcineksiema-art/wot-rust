//! Bot combat selection and the stand-and-shoot command.
//!
//! Target selection is the only place a bot fires its own LOS raycasts. Cheap gates run first
//! and a candidate earns a raycast only when it can beat the nearest visible target so far.

use game_core::math::{segment_box_entry, world_to_tank_local};
use game_core::{TankId, TeamId};
use glam::Vec3;
use sim::{TankCommand, TankState, VIEW_RANGE_M};

use crate::bot_aim::BotFiringSolution;

/// What a bot knows about its own situation when it chooses whom to shoot.
///
/// Everything here is knowledge a crew legitimately has: which way the last hit rang, and what
/// the rest of the platoon is shooting at. Nothing in it reveals an unspotted enemy.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct BotSituation<'a> {
    /// World-yaw bearing of the last unexplained hit on this bot's own hull, if any.
    pub threat_bearing: Option<f32>,
    /// Who every bot in the battle was engaging at the START of this tick.
    ///
    /// A snapshot, deliberately, and it is the whole roster rather than one team's slice so that
    /// no per-bot list has to be built. Reading LIVE targets would make the roster's iteration
    /// order decide who concentrates on whom — the first bot in the list would choose freely and
    /// the rest would pile onto its choice, and the same battle replayed with the roster built in
    /// another order would diverge. One tick of staleness costs nothing; determinism is not
    /// optional.
    pub allied_targets: &'a [AlliedEngagement],
}

/// One bot's engagement as it stood at the start of the tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AlliedEngagement {
    pub bot: TankId,
    pub team: TeamId,
    pub target: TankId,
}

/// A target close enough to hit is worth more than a silhouette at the edge of vision. This is
/// the term the selector used to have on its own, and it stays the largest.
const PROXIMITY_WEIGHT: f32 = 1.0;

/// Finish the cripple. Two half-dead enemies shoot twice as often as one dead one and one fresh
/// one, so hit points already taken off a hull are the cheapest damage left on the field.
const CRIPPLE_WEIGHT: f32 = 0.9;

/// Answer the gun that is shooting at you. Weighted below proximity on purpose: it biases the
/// choice toward the quarter the hit came from without letting a single ricochet drag a bot's
/// attention across the map past a tank at point-blank range.
const RETURN_FIRE_WEIGHT: f32 = 0.7;

/// A gun already laid on this bot is the one about to fire at it. This is the whole of "threat
/// priority": not who is most dangerous in the abstract, but who is pointing.
const AIMED_AT_WEIGHT: f32 = 0.6;

/// Shoot what a teammate is already shooting. Concentration is what turns damage into kills — two
/// bots splitting fire between two enemies leave both alive and shooting.
const FOCUS_FIRE_WEIGHT: f32 = 0.5;

/// The half-angle inside which a hit bearing and a visible enemy are treated as the same quarter.
const RETURN_FIRE_CONE_RAD: f32 = std::f32::consts::FRAC_PI_3;

/// The half-angle inside which an enemy gun counts as laid on this bot.
///
/// A cone, not a falloff over the whole circle. The first version scored `1 - off/PI`, which gave
/// a gun pointing ninety degrees away — at nobody — half the credit of one pointing straight at
/// the bot, and that was enough to swing a choice between two enemies at identical range. "Aimed
/// at" has to mean aimed at.
const AIMED_AT_CONE_RAD: f32 = std::f32::consts::FRAC_PI_3;

/// The enemy this bot should engage: team-spotted, in range, in its own line of sight, and the
/// best of those by [`engagement_score`].
///
/// The raycast is still EARNED. A candidate is only traced when its cheap score beats the best
/// target found so far, exactly as the nearest-first version worked — every scoring term is
/// arithmetic on state the bot already holds, so ranking costs nothing and the expensive question
/// is asked in the order most likely to end the search on the first ask.
pub(crate) fn bot_best_engageable_enemy<'a>(
    tank: &TankState,
    tanks: &'a [TankState],
    exclude: Option<TankId>,
    heightmap: Option<&terrain::HeightMap>,
    cover: &[terrain::StaticCoverObject],
    situation: &BotSituation<'_>,
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
        if tank.position.distance_squared(target.position) > VIEW_RANGE_M * VIEW_RANGE_M {
            continue;
        }
        let score = engagement_score(tank, target, situation);
        if best.is_some_and(|(_, best_score)| score <= best_score) {
            continue;
        }
        if sim::tank_line_of_sight(tank, target, heightmap, cover) {
            best = Some((target, score));
        }
    }
    best.map(|(target, _)| target)
}

/// How much this bot wants to shoot this enemy, from cheap state only.
///
/// Five terms, each in `0..=1` before its weight, so the weights above ARE the priorities and can
/// be read against each other. With no cripple, no incoming fire, nobody aiming and nobody to
/// support, the score collapses to proximity — which is the behaviour this replaced, so a plain
/// two-tank meeting still ends the way it always did.
fn engagement_score(tank: &TankState, target: &TankState, situation: &BotSituation<'_>) -> f32 {
    let distance = tank.position.distance(target.position);
    let proximity = 1.0 - (distance / VIEW_RANGE_M).clamp(0.0, 1.0);

    let full = target.spec.hit_points.max(1) as f32;
    let cripple = 1.0 - (target.hit_points as f32 / full).clamp(0.0, 1.0);

    let to_target = target.position - tank.position;
    let return_fire = situation.threat_bearing.map_or(0.0, |bearing| {
        let target_bearing = to_target.x.atan2(to_target.z);
        let off = wrapped(target_bearing - bearing).abs();
        if off <= RETURN_FIRE_CONE_RAD { 1.0 - off / RETURN_FIRE_CONE_RAD } else { 0.0 }
    });

    // Is that gun pointing back? The target's turret world yaw against the bearing from it to us.
    let gun_yaw = target.yaw_rad + target.turret_yaw_rad;
    let to_me = -to_target;
    let aimed_at = if to_me.length_squared() > f32::EPSILON {
        let off = wrapped(to_me.x.atan2(to_me.z) - gun_yaw).abs();
        if off <= AIMED_AT_CONE_RAD { 1.0 - off / AIMED_AT_CONE_RAD } else { 0.0 }
    } else {
        0.0
    };

    // A TEAMMATE on it, not this bot's own last choice: counting itself would turn the term into
    // hysteresis on its own decision rather than support for somebody else's.
    let focus = f32::from(situation.allied_targets.iter().any(|engagement| {
        engagement.team == tank.team && engagement.bot != tank.id && engagement.target == target.id
    }));

    PROXIMITY_WEIGHT * proximity
        + CRIPPLE_WEIGHT * cripple
        + RETURN_FIRE_WEIGHT * return_fire
        + AIMED_AT_WEIGHT * aimed_at
        + FOCUS_FIRE_WEIGHT * focus
}

/// An angle folded into `-PI..=PI`.
fn wrapped(angle: f32) -> f32 {
    let turn = std::f32::consts::TAU;
    let folded = (angle + std::f32::consts::PI).rem_euclid(turn) - std::f32::consts::PI;
    if folded.is_finite() { folded } else { 0.0 }
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
