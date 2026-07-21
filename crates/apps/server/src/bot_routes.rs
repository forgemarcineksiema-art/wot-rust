//! The bots' route brain — which strategic point a bot drives to and the drive command that gets
//! it there. Split from `bots.rs` (roster, unstuck, combat) for the reviewability budget.
//!
//! Two postures share the map: **skirmishers** rotate through the team's strategic points and
//! keep pressure moving, **overwatch** bots drive to a hull-down or high-ground point on their
//! own side, turn their bow to the field, and hold — the terrain-shaped positions the map author
//! placed finally get used, and the running-gear hull-down mechanics get someone standing in
//! them. Combat always preempts a route (see `bots.rs`).

use game_core::TeamId;
use game_core::math::wrap_angle;
use glam::Vec3;
use sim::{TankCommand, TankState};
use terrain::{BattlefieldMap, StrategicPoint, StrategicRole};

use crate::battle::BattleSeed;

/// How a bot fights the map, fixed for the battle at roster creation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BotPosture {
    /// Rotate through the team's strategic points; the moving pressure of the team.
    Skirmish,
    /// Take a hull-down / high-ground point on the home side and hold it facing the field.
    Overwatch,
}

/// Every third bot holds ground; the rest push. A 7-bot team fields 2 overwatch + 5 skirmish.
pub(crate) fn bot_posture(index: usize) -> BotPosture {
    if index % 3 == 2 { BotPosture::Overwatch } else { BotPosture::Skirmish }
}

/// A raw seeded start index; the route brain reduces it modulo its *actual* point pool, so every
/// point (not just the first five) can be a bot's opening objective.
pub(crate) fn seed_route_index(seed: BattleSeed, index: usize) -> usize {
    seed_route_mix(index as u64 ^ seed.route_seed()) as usize
}

fn seed_route_mix(mut value: u64) -> u64 {
    value ^= value >> 33;
    value = value.wrapping_mul(0xff51_afd7_ed55_8ccd);
    value ^= value >> 33;
    value
}

/// The cached water detour: which objective it was planned against and the waypoint that gets
/// driven at. Replanning the drive line (the full-length deep-water probe) is the route brain's
/// only expensive step, so it runs on the bots' replan cadence instead of every tick; the cache
/// key is the objective, so a route rotation or posture fallback replans immediately.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RoutePlan {
    objective: Vec3,
    drive_at: Vec3,
}

/// How stale a cached water detour may grow (0.4 s, staggered by tank id). A hull covers ~3 m
/// between replans — nothing on a 30-40 m river changes faster, and the every-tick deep-water
/// survival check in `bots.rs` still guards the hull itself.
pub(crate) const ROUTE_REPLAN_INTERVAL_TICKS: u64 = 24;

pub(crate) fn bot_route_command(
    route_index: &mut usize,
    plan: &mut Option<RoutePlan>,
    replan_due: bool,
    posture: BotPosture,
    tank: &TankState,
    battlefield: &BattlefieldMap,
) -> TankCommand {
    match posture {
        BotPosture::Skirmish => {
            let target = bot_route_target(route_index, tank.team, tank.position, battlefield);
            bot_drive_toward(
                tank,
                planned_drive_target(plan, replan_due, tank, target, battlefield),
            )
        }
        BotPosture::Overwatch => bot_overwatch_command(plan, replan_due, tank, battlefield),
    }
}

/// The waypoint to drive at, through the plan cache. Dry maps skip the cache entirely — with no
/// water body every probe is a constant-time no-op, and the lane math wants the live position.
fn planned_drive_target(
    plan: &mut Option<RoutePlan>,
    replan_due: bool,
    tank: &TankState,
    objective: Vec3,
    battlefield: &BattlefieldMap,
) -> Vec3 {
    if battlefield.water.is_none() {
        return water_safe_target(tank, objective, battlefield);
    }
    let stale = replan_due
        || plan.as_ref().is_none_or(|plan| plan.objective.distance_squared(objective) > 1.0);
    if stale {
        *plan = Some(RoutePlan {
            objective,
            drive_at: water_safe_target(tank, objective, battlefield),
        });
    }
    plan.as_ref().expect("plan freshened above").drive_at
}

// --- Water awareness -------------------------------------------------------------------------
// The river is a real decision (the current drowns), so the route brain must make it: a bot
// whose straight line to its objective runs through deep water detours through the nearest
// authored Crossing point instead of driving into the channel. Dry maps never enter this code —
// every helper returns "safe" the moment there is no water body.

/// Deeper than this and the route brain treats the water as impassable. Sits between the ford
/// contract ceiling (`physics::water::FORD_MAX_DEPTH_M` = 0.9 — bots MUST still take fords)
/// and the drowning line (`sim::DROWN_DEPTH_M` = 1.5); the Bystra channel is >= 2.2 m outside
/// the crossings, so any value in between separates cleanly.
const BOT_DEEP_WATER_M: f32 = 1.2;
/// Sampling step when checking the drive line for deep water (finer than any crossing's deck).
const WATER_PROBE_STEP_M: f32 = 12.0;
/// How far past a crossing's centre the exit waypoint sits — beyond the far bank of a
/// 30-40 m river, so the bot commits across the deck instead of stalling mid-crossing.
const CROSSING_EXIT_M: f32 = 45.0;

/// Wading past the ford ceiling arms the momentum check below: every authored ford stays at
/// or under `physics::water::FORD_MAX_DEPTH_M` (0.9), so a hull deeper than this is already
/// OFF every legitimate crossing line.
const WADE_ALERT_M: f32 = 0.95;
/// How far ahead of the hull the armed momentum check probes, in seconds of current velocity.
/// Momentum is what carries a wading hull past the 1.2 m line toward the 1.5 m drowning line
/// before a reactive escape bites — a heavy, slow-reversing hull (the Centurion) overshot to
/// 1.52 m with a purely positional check. The check only arms past `WADE_ALERT_M`, so dry
/// approaches to bridges and honest ford runs never trip it.
const DEEP_WATER_LOOKAHEAD_S: f32 = 0.8;

/// True when the hull stands past the route brain's deep-water line — or is already wading
/// off every crossing line with momentum still carrying it deeper — the survival check
/// `bots.rs` runs before anything else.
pub(crate) fn bot_in_deep_water(tank: &TankState, battlefield: &BattlefieldMap) -> bool {
    let here = water_depth_at(battlefield, tank.position.x, tank.position.z);
    if here > BOT_DEEP_WATER_M {
        return true;
    }
    if here <= WADE_ALERT_M {
        return false;
    }
    let ahead = tank.position + tank.velocity_mps * DEEP_WATER_LOOKAHEAD_S;
    water_depth_at(battlefield, ahead.x, ahead.z) > here
}

/// Standing-water depth under a world point; 0 on dry maps, off-map, or above the waterline.
pub(crate) fn water_depth_at(battlefield: &BattlefieldMap, x: f32, z: f32) -> f32 {
    match (battlefield.water, battlefield.heightmap.sample_height(x, z)) {
        (Some(water), Some(ground)) => water.depth_over(ground).max(0.0),
        _ => 0.0,
    }
}

/// True when the straight drive line from `from` to `to` runs through deep water.
fn line_crosses_deep_water(battlefield: &BattlefieldMap, from: Vec3, to: Vec3) -> bool {
    if battlefield.water.is_none() {
        return false;
    }
    let span = to - from;
    let length = span.length();
    if length < 1.0 {
        return false;
    }
    let steps = (length / WATER_PROBE_STEP_M).ceil() as usize;
    (1..=steps).any(|step| {
        let point = from + span * (step as f32 / steps as f32);
        water_depth_at(battlefield, point.x, point.z) > BOT_DEEP_WATER_M
    })
}

/// The waypoint that actually gets driven at: the objective itself when the straight line is
/// dry, otherwise the nearest authored Crossing (by total detour length) — and once the bot is
/// on the crossing, an exit point past the far bank so it commits across the deck.
fn water_safe_target(tank: &TankState, target: Vec3, battlefield: &BattlefieldMap) -> Vec3 {
    bot_lane(tank, water_safe_waypoint(tank, target, battlefield), battlefield)
}

/// Spread the column: each bot aims a deterministic sideways lane off the shared waypoint, so
/// two bots bound for the same objective stop fighting for the same line forever (the
/// spawn-funnel deadlock: wedge, back out, drive straight back into the same contact). Lanes
/// converge inside arrival range, and a lane that would land in deep water collapses back to
/// the waypoint itself (crossing decks are narrower than the spread).
fn bot_lane(tank: &TankState, waypoint: Vec3, battlefield: &BattlefieldMap) -> Vec3 {
    let span = waypoint - tank.position;
    let flat = Vec3::new(span.x, 0.0, span.z);
    if flat.length() < 35.0 {
        return waypoint;
    }
    let side = Vec3::new(flat.z, 0.0, -flat.x).normalize_or_zero();
    let lane = ((tank.id.0 % 5) as f32 - 2.0) * 9.0;
    let candidate = waypoint + side * lane;
    // The lane must be dry along its DRIVE LINE, not just at the endpoint: an offset exit
    // past a crossing can sit on the dry far bank while the offset line leaves the deck and
    // runs through the channel beside it. Enforced only on the final approach — far from the
    // waypoint the lanes must keep spreading traffic or the spawn-funnel deadlock returns.
    let final_approach = flat.length() < 90.0;
    if water_depth_at(battlefield, candidate.x, candidate.z) > BOT_DEEP_WATER_M
        || (final_approach && line_crosses_deep_water(battlefield, tank.position, candidate))
    {
        return waypoint;
    }
    candidate
}

fn water_safe_waypoint(tank: &TankState, target: Vec3, battlefield: &BattlefieldMap) -> Vec3 {
    if !line_crosses_deep_water(battlefield, tank.position, target) {
        return target;
    }
    let crossings =
        battlefield.strategic_points.iter().filter(|point| point.role == StrategicRole::Crossing);
    let Some(crossing) = crossings.min_by(|a, b| {
        let detour = |point: &StrategicPoint| {
            let position = Vec3::from_array(point.position);
            tank.position.distance(position) + position.distance(target)
        };
        detour(a).total_cmp(&detour(b))
    }) else {
        // A map with deep water but no authored crossing: stop short of the channel.
        return tank.position;
    };
    let center = Vec3::from_array(crossing.position);
    if tank.position.distance(center) > crossing.radius_m.max(20.0) {
        return center;
    }
    // On the crossing: aim past the far bank. The deck's dry direction is discovered by
    // probing, not assumed, so any future map's crossing orientation works unedited.
    crossing_exit(center, target, battlefield)
}

/// The dry exit waypoint on the target's side of a crossing: of the four axis-aligned
/// candidates past the deck, keep the dry ones and take the one nearest the objective.
fn crossing_exit(center: Vec3, target: Vec3, battlefield: &BattlefieldMap) -> Vec3 {
    let candidates = [
        center + Vec3::X * CROSSING_EXIT_M,
        center - Vec3::X * CROSSING_EXIT_M,
        center + Vec3::Z * CROSSING_EXIT_M,
        center - Vec3::Z * CROSSING_EXIT_M,
    ];
    candidates
        .into_iter()
        .filter(|point| water_depth_at(battlefield, point.x, point.z) <= BOT_DEEP_WATER_M)
        .min_by(|a, b| a.distance_squared(target).total_cmp(&b.distance_squared(target)))
        .unwrap_or(center)
}

/// Drive to the nearest own-side hull-down/high-ground point; once inside its radius, swing the
/// bow toward the map centre (gun and strongest armor to the field) and hold on the brake.
fn bot_overwatch_command(
    plan: &mut Option<RoutePlan>,
    replan_due: bool,
    tank: &TankState,
    battlefield: &BattlefieldMap,
) -> TankCommand {
    let Some(point) = bot_overwatch_point(tank.team, tank.position, battlefield) else {
        // A map without authored overwatch ground: fall back to standing pressure mid-map.
        let center =
            Vec3::new(battlefield.size_m[0] * 0.5, tank.position.y, battlefield.size_m[1] * 0.5);
        return bot_drive_toward(
            tank,
            planned_drive_target(plan, replan_due, tank, center, battlefield),
        );
    };
    let target = Vec3::from_array(point.position);
    if tank.position.distance(target) > point.radius_m.max(20.0) {
        return bot_drive_toward(
            tank,
            planned_drive_target(plan, replan_due, tank, target, battlefield),
        );
    }
    // On station. Face the field, then hold — a hull-down shelf only works bow-on.
    let field_center =
        Vec3::new(battlefield.size_m[0] * 0.5, tank.position.y, battlefield.size_m[1] * 0.5);
    let yaw_error = wrap_angle(bot_yaw_to(tank.position, field_center) - tank.yaw_rad);
    if yaw_error.abs() > 0.35 {
        return TankCommand {
            throttle: 0.15,
            steer: (yaw_error * 1.8).clamp(-1.0, 1.0),
            ..TankCommand::idle()
        };
    }
    TankCommand { brake: 0.5, ..TankCommand::idle() }
}

/// The nearest hull-down (preferred) or high-ground point on the team's side of the map.
fn bot_overwatch_point(
    team: TeamId,
    position: Vec3,
    battlefield: &BattlefieldMap,
) -> Option<&StrategicPoint> {
    let candidates = battlefield.strategic_points.iter().filter(|point| {
        matches!(point.role, StrategicRole::HullDown | StrategicRole::HighGround)
            && bot_point_matches_team(point, team, battlefield)
    });
    candidates.min_by(|a, b| {
        // Hull-down shelves outrank bare high ground; distance breaks the tie.
        let rank = |point: &StrategicPoint| u8::from(point.role != StrategicRole::HullDown);
        rank(a).cmp(&rank(b)).then(
            position
                .distance_squared(Vec3::from_array(a.position))
                .total_cmp(&position.distance_squared(Vec3::from_array(b.position))),
        )
    })
}

fn bot_route_target(
    route_index: &mut usize,
    team: TeamId,
    position: Vec3,
    battlefield: &BattlefieldMap,
) -> Vec3 {
    let points: Vec<&StrategicPoint> = battlefield
        .strategic_points
        .iter()
        .filter(|point| bot_point_matches_team(point, team, battlefield))
        .collect();
    if points.is_empty() {
        return Vec3::new(battlefield.size_m[0] * 0.5, position.y, battlefield.size_m[1] * 0.5);
    }
    let point = points[*route_index % points.len()];
    let target = Vec3::from_array(point.position);
    if position.distance(target) < point.radius_m.max(25.0) {
        *route_index = (*route_index + 1) % points.len();
        Vec3::from_array(points[*route_index].position)
    } else {
        target
    }
}

/// A strategic point belongs to a team's plan when it sits on the mid-line (shared objectives:
/// crossings, the central rise) or on the team's own half — decided by geometry against the
/// team's spawn zone, not by string-matching point ids, so any future map layout works unedited.
fn bot_point_matches_team(
    point: &StrategicPoint,
    team: TeamId,
    battlefield: &BattlefieldMap,
) -> bool {
    let center_z = battlefield.size_m[1] * 0.5;
    let point_side = point.position[2] - center_z;
    if point_side.abs() < 25.0 {
        return true;
    }
    let Some(zone) = battlefield.spawn_zones.iter().find(|zone| zone.team == team.0) else {
        return true;
    };
    (zone.center[2] - center_z).signum() == point_side.signum()
}

fn bot_drive_toward(tank: &TankState, target: Vec3) -> TankCommand {
    let desired_yaw = bot_yaw_to(tank.position, target);
    let yaw_error = wrap_angle(desired_yaw - tank.yaw_rad);
    TankCommand {
        throttle: if yaw_error.abs() > 1.2 { 0.35 } else { 0.78 },
        steer: (yaw_error * 1.8).clamp(-1.0, 1.0),
        ..TankCommand::idle()
    }
}

pub(crate) fn bot_yaw_to(from: Vec3, to: Vec3) -> f32 {
    let delta = to - from;
    delta.x.atan2(delta.z)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn battlefield() -> BattlefieldMap {
        map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2)
    }

    fn point_ids(team: TeamId, map: &BattlefieldMap) -> Vec<&str> {
        map.strategic_points
            .iter()
            .filter(|point| bot_point_matches_team(point, team, map))
            .map(|point| point.id.as_str())
            .collect()
    }

    #[test]
    fn team_pools_come_from_geometry_not_point_ids() {
        let map = battlefield();
        let south = point_ids(TeamId(1), &map);
        let north = point_ids(TeamId(2), &map);

        // Shared mid-line objectives appear in both plans.
        for shared in ["rail_crossing_west", "rail_crossing_east", "oktyabrskiy"] {
            assert!(south.contains(&shared), "south pool misses {shared}");
            assert!(north.contains(&shared), "north pool misses {shared}");
        }
        // Own-half points stay own-half.
        assert!(
            south.contains(&"hill_hulldown_south") && !south.iter().any(|id| id.contains("north"))
        );
        assert!(
            north.contains(&"hill_hulldown_north") && !north.iter().any(|id| id.contains("south"))
        );
    }

    #[test]
    fn seeded_start_indices_reach_the_whole_pool_not_just_the_first_five() {
        let map = battlefield();
        let pool = point_ids(TeamId(1), &map).len();
        assert!(pool > 5, "the pool this test guards must exceed the old % 5");
        let mut seen = std::collections::HashSet::new();
        for bot in 0..64 {
            seen.insert(seed_route_index(BattleSeed::fixed(3), bot) % pool);
        }
        assert!(
            seen.iter().any(|index| *index >= 5),
            "start objectives must cover the tail of the pool, saw {seen:?}"
        );
    }

    #[test]
    fn every_third_bot_takes_the_overwatch_posture() {
        let postures: Vec<BotPosture> = (0..7).map(bot_posture).collect();
        let overwatch = postures.iter().filter(|p| **p == BotPosture::Overwatch).count();
        assert_eq!(overwatch, 2, "a 7-bot team fields two overwatch hulls");
        assert_eq!(postures[0], BotPosture::Skirmish, "the first bot pushes");
    }

    #[test]
    fn an_overwatch_bot_drives_to_its_hull_down_shelf_and_holds_facing_the_field() {
        let map = battlefield();
        let shelf = map
            .strategic_points
            .iter()
            .find(|point| point.id == "hill_hulldown_south")
            .expect("authored shelf");
        let mut tank = crate::bots::test_support::tank_at(
            1,
            TeamId(1),
            Vec3::new(shelf.position[0] - 200.0, 0.0, shelf.position[2]),
        );

        // Far away: the posture drives toward the shelf, not some rotation waypoint.
        let mut route_index = 0;
        let mut plan = None;
        let approach = bot_route_command(
            &mut route_index,
            &mut plan,
            true,
            BotPosture::Overwatch,
            &tank,
            &map,
        );
        assert!(approach.throttle > 0.0, "en route the bot drives");
        let desired = bot_yaw_to(tank.position, Vec3::from_array(shelf.position));
        assert!(
            wrap_angle(desired - tank.yaw_rad).signum() == approach.steer.signum()
                || approach.steer.abs() < 1.0e-3,
            "steering must pull toward the shelf"
        );

        // On station and facing the field: full hold, no creep, no route advance.
        tank.position = Vec3::from_array(shelf.position);
        tank.yaw_rad = bot_yaw_to(
            tank.position,
            Vec3::new(map.size_m[0] * 0.5, tank.position.y, map.size_m[1] * 0.5),
        );
        let hold = bot_route_command(
            &mut route_index,
            &mut plan,
            true,
            BotPosture::Overwatch,
            &tank,
            &map,
        );
        assert_eq!(hold.throttle, 0.0, "an overwatch bot on station stands");
        assert!(hold.brake > 0.0, "and holds the brake");

        // On station but bow pointing home: pivot toward the field instead of freezing.
        tank.yaw_rad += std::f32::consts::PI;
        let pivot = bot_route_command(
            &mut route_index,
            &mut plan,
            true,
            BotPosture::Overwatch,
            &tank,
            &map,
        );
        assert!(pivot.steer.abs() > 0.0, "a shelf only works bow-on: the bot swings its hull");
        assert!(pivot.throttle > 0.0 && pivot.throttle < 0.5, "a pivot creeps, not charges");
    }

    /// The river is a route decision, not a grave: a bot bound for the far bank must detour
    /// through an authored Crossing instead of driving into the drowning channel.
    #[test]
    fn a_bot_bound_across_the_river_detours_through_a_crossing() {
        let map = map_forge::battlefield(terrain::MapId::BystraValley);
        // z=420 runs between the crossings (bridge z=500, ford z=320): open channel only.
        // Tank id 2: lane offset 0, so waypoints compare exactly.
        let west = crate::bots::test_support::tank_at(2, TeamId(1), Vec3::new(430.0, 8.0, 420.0));
        let town = Vec3::new(720.0, 10.0, 420.0);

        assert!(
            line_crosses_deep_water(&map, west.position, town),
            "the straight line to the town must cross the channel (test premise)"
        );
        let waypoint = water_safe_target(&west, town, &map);
        let nearest_crossing = map
            .strategic_points
            .iter()
            .filter(|point| point.role == StrategicRole::Crossing)
            .map(|point| Vec3::from_array(point.position).distance(waypoint))
            .fold(f32::MAX, f32::min);
        assert!(
            nearest_crossing < 1.0,
            "the detour waypoint must be an authored crossing, got {waypoint:?}"
        );

        // On the crossing itself the waypoint commits PAST the far bank (dry, on the town's
        // side), so the bot crosses the deck instead of stalling on its own waypoint.
        let bridge = map
            .strategic_points
            .iter()
            .find(|point| point.id == "stone_bridge")
            .expect("authored bridge");
        let on_deck =
            crate::bots::test_support::tank_at(7, TeamId(1), Vec3::from_array(bridge.position));
        let exit = water_safe_target(&on_deck, town, &map);
        assert!(exit.x > bridge.position[0] + 20.0, "the exit sits on the town side: {exit:?}");
        assert!(
            water_depth_at(&map, exit.x, exit.z) <= BOT_DEEP_WATER_M,
            "the exit waypoint must be drivable"
        );
    }

    /// A dry map never pays for water awareness: the safe target IS the target.
    #[test]
    fn dry_maps_route_exactly_as_before() {
        let map = battlefield();
        // Tank id 2: lane offset 0 (the lane spread itself is water-aware, tested above).
        let tank = crate::bots::test_support::tank_at(2, TeamId(1), Vec3::new(300.0, 8.0, 300.0));
        let target = Vec3::new(700.0, 8.0, 700.0);
        assert_eq!(water_safe_target(&tank, target, &map), target);
    }

    #[test]
    fn skirmishers_still_rotate_their_route_on_arrival() {
        let map = battlefield();
        let pool: Vec<&StrategicPoint> = map
            .strategic_points
            .iter()
            .filter(|point| bot_point_matches_team(point, TeamId(1), &map))
            .collect();
        let first = pool[0];
        let tank =
            crate::bots::test_support::tank_at(1, TeamId(1), Vec3::from_array(first.position));

        let mut route_index = 0;
        let mut plan = None;
        bot_route_command(&mut route_index, &mut plan, true, BotPosture::Skirmish, &tank, &map);
        assert_eq!(route_index, 1, "arriving at the objective advances the rotation");
    }
}
