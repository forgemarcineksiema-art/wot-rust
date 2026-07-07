use game_core::TankId;
use glam::Vec3;
use sim::{TankCommand, TankState};
use terrain::BattlefieldMap;

use crate::battle::BattleSeed;
use crate::bot_aim::{BotFiringSolution, solve_firing_solution};
use crate::bot_combat::{
    bot_combat_command, bot_nearest_engageable_enemy, bot_target_still_engageable, find_tank,
};
use crate::bot_routes::{BotPosture, bot_posture, bot_route_command, seed_route_index};

/// How little per-tick progress counts as "not moving" while the bot commands forward drive
/// (0.01 m/tick = 0.6 m/s at 60 Hz — well under the slowest route crawl).
const STALL_PROGRESS_EPS_M: f32 = 0.01;
/// Ticks of no progress under a drive command before the bot decides it is stuck (1.5 s).
const STALL_TICKS_TO_REVERSE: u32 = 90;
/// How long a stuck bot backs out before resuming its route (~1.3 s, a few hull-lengths' arc).
const REVERSE_TICKS: u32 = 80;
/// After the back-out the bot drives ~2 s on a sideways arc before resuming its route: the
/// route brain steers straight at the waypoint, so without the sidestep a bot wedged on a
/// parked hull (or a corner) reverses and drives right back into the same contact forever —
/// the spawn-cluster deadlock. The arc's side alternates per attempt, so both ways get tried.
const SIDESTEP_TICKS: u32 = 120;

/// The expensive halves of the bot brain run on cadences, not every tick. Target selection
/// (the only per-bot LOS raycasts) re-runs every 6 ticks — the spotting recompute interval,
/// so a fresher answer would read stale data anyway; between runs the cached target is held
/// through the cheap gates. The ballistic solve re-runs every 3 ticks; between runs the turret
/// slews toward the cached ABSOLUTE lay (see `BotFiringSolution`), so aiming stays smooth.
/// Both cadences are staggered by tank id, spreading the work across ticks instead of letting
/// all 13 bots think in the same tick — the difference between a level cost and a spike.
const TARGET_RESELECT_INTERVAL_TICKS: u64 = 6;
const AIM_SOLVE_INTERVAL_TICKS: u64 = 3;

/// Ticks of shooting at one target with its hit points unmoved before the duel is declared
/// futile (12 s — a full reload cycle and then some of shells eating a crest or a bank).
/// Combat preempts the route, so without this a hull-down standoff parks both teams for the
/// whole battle (the seed-23 stalemate: shells in flight forever, zero damage after t=22 s).
const FUTILE_ENGAGE_TICKS: u32 = 720;
/// How long a futile target stays blacklisted for THIS bot (20 s): long enough to actually
/// drive somewhere better, short enough to re-engage from the new angle.
const FUTILE_TARGET_HOLD_TICKS: u32 = 1200;
/// After a futile duel the bot DRIVES for this long with combat suppressed (8 s). Swapping
/// targets is not enough — on an open flank a second futile enemy is always visible, and the
/// one-tick route command between duels moves the hull nowhere.
const REPOSITION_TICKS: u32 = 480;

/// True on the ticks where `tank_id`'s slice of periodic work is due (deterministic stagger).
fn cadence_due(tick: u64, tank_id: TankId, interval: u64) -> bool {
    tick.wrapping_add(tank_id.0).is_multiple_of(interval)
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct BotAgent {
    tank_id: TankId,
    route_index: usize,
    /// Skirmish (rotate objectives) or overwatch (hold a hull-down point) — see `bot_routes`.
    posture: BotPosture,
    /// Hull position at the previous command, for stall detection.
    last_position: Option<Vec3>,
    /// Consecutive ticks the bot commanded drive but the hull did not move.
    stall_ticks: u32,
    /// Remaining ticks of the current back-out maneuver.
    reverse_ticks: u32,
    /// The engaged enemy, held between re-selections through the cheap gates.
    target: Option<TankId>,
    /// The cached ballistic lay against `target`, recomputed on the solve cadence.
    solution: Option<BotFiringSolution>,
    /// Futility watch on the engaged target: its hit points when the watch started and how
    /// long they have not moved. Progress (anyone damaging it) resets the clock.
    futile_watch: Option<(TankId, u32, u32)>,
    /// A target this bot walked away from, and the ticks left before it may re-engage it.
    futile_hold: Option<(TankId, u32)>,
    /// Remaining ticks of combat-suppressed driving after a futile duel.
    reposition_ticks: u32,
    /// Remaining ticks of the post-back-out sideways arc.
    sidestep_ticks: u32,
    /// Which side the next escape maneuver arcs toward; flipped per attempt.
    escape_left: bool,
}

impl BotAgent {
    fn new(tank_id: TankId, route_index: usize, posture: BotPosture) -> Self {
        let escape_left = tank_id.0.is_multiple_of(2);
        Self {
            tank_id,
            route_index,
            posture,
            last_position: None,
            stall_ticks: 0,
            reverse_ticks: 0,
            target: None,
            solution: None,
            futile_watch: None,
            futile_hold: None,
            reposition_ticks: 0,
            sidestep_ticks: 0,
            escape_left,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct BotRoster {
    agents: Vec<BotAgent>,
}

impl BotRoster {
    pub(crate) fn empty() -> Self {
        Self { agents: Vec::new() }
    }

    pub(crate) fn new(tank_ids: Vec<TankId>, seed: BattleSeed) -> Self {
        let agents = tank_ids
            .into_iter()
            .enumerate()
            .map(|(index, tank_id)| {
                BotAgent::new(tank_id, seed_route_index(seed, index), bot_posture(index))
            })
            .collect();
        Self { agents }
    }

    pub(crate) fn commands(
        &mut self,
        tick: u64,
        tanks: &[TankState],
        battlefield: &BattlefieldMap,
        battle_over: bool,
    ) -> Vec<(TankId, TankCommand)> {
        let mut commands = Vec::with_capacity(self.agents.len());
        for index in 0..self.agents.len() {
            let tank_id = self.agents[index].tank_id;
            let command = tanks.iter().find(|tank| tank.id == tank_id).map_or_else(
                TankCommand::idle,
                |tank| {
                    if battle_over || tank.hit_points == 0 {
                        TankCommand::idle()
                    } else {
                        bot_command_for_tank(
                            &mut self.agents[index],
                            tick,
                            tank,
                            tanks,
                            battlefield,
                        )
                    }
                },
            );
            commands.push((tank_id, command));
        }
        commands
    }
}

fn bot_command_for_tank(
    agent: &mut BotAgent,
    tick: u64,
    tank: &TankState,
    tanks: &[TankState],
    battlefield: &BattlefieldMap,
) -> TankCommand {
    // Survival preempts everything, combat included: a hull past the route brain's deep-water
    // line is heading for a flooded engine — back out the way it came before doing anything
    // else. Fords (<= 0.9 m) never trip this; the escape resets the stall counter so the slow
    // wade out is not misread as being stuck.
    if crate::bot_routes::bot_in_deep_water(tank, battlefield) {
        agent.stall_ticks = 0;
        agent.last_position = Some(tank.position);
        return bot_reverse_command(agent.escape_left);
    }
    // The futile-target hold runs down whether fighting or driving.
    if let Some((_, ticks)) = &mut agent.futile_hold {
        *ticks = ticks.saturating_sub(1);
        if *ticks == 0 {
            agent.futile_hold = None;
        }
    }
    let repositioning = agent.reposition_ticks > 0;
    if repositioning {
        agent.reposition_ticks -= 1;
    }
    if !repositioning
        && let Some(target) = bot_current_target(agent, tick, tank, tanks, battlefield)
    {
        // A duel that changes nothing is a parked bot: shells eating a crest or a bank while
        // combat preempts the route for the whole battle. When the target's hit points have
        // not moved for the whole futility window, walk away — combat is suppressed for the
        // reposition window so the hull actually relocates, and the hold keeps this bot from
        // re-locking the same enemy the moment it stops.
        let futile = match agent.futile_watch {
            Some((id, hp, ticks)) if id == target.id && hp == target.hit_points => {
                agent.futile_watch = Some((id, hp, ticks + 1));
                ticks + 1 >= FUTILE_ENGAGE_TICKS
            }
            _ => {
                agent.futile_watch = Some((target.id, target.hit_points, 0));
                false
            }
        };
        if futile {
            agent.futile_hold = Some((target.id, FUTILE_TARGET_HOLD_TICKS));
            agent.futile_watch = None;
            agent.reposition_ticks = REPOSITION_TICKS;
            agent.target = None;
            agent.solution = None;
        } else {
            // Standing to shoot is intentional, not a stall.
            agent.stall_ticks = 0;
            agent.last_position = Some(tank.position);
            let solve_due = match &agent.solution {
                Some(solution) if solution.target == target.id => {
                    cadence_due(tick, tank.id, AIM_SOLVE_INTERVAL_TICKS)
                }
                _ => true,
            };
            if solve_due {
                agent.solution = Some(solve_firing_solution(tank, target));
            }
            let solution = agent.solution.expect("solution cached above");
            return bot_combat_command(tank, &solution);
        }
    } else if !repositioning {
        agent.futile_watch = None;
    }
    agent.solution = None;
    let posture = agent.posture;
    let command = bot_route_command(&mut agent.route_index, posture, tank, battlefield);
    // Stall detection guards MOVEMENT intent only. An overwatch bot holding its shelf (and the
    // slow on-station pivot) stands still on purpose — without this gate the hold would read as
    // "stuck" every 1.5 s and the bot would bounce off its own position forever.
    if command.throttle > 0.2 {
        if let Some(unstuck) = bot_unstuck_command(agent, tank) {
            return unstuck;
        }
    } else {
        agent.stall_ticks = 0;
        agent.last_position = Some(tank.position);
    }
    command
}

/// Detect and escape a physical block. The route brain only ever drives FORWARD, so two bots that
/// meet nose-to-nose (or a bot pinned against the player or a wall) push into the contact for the
/// rest of the battle. After [`STALL_TICKS_TO_REVERSE`] ticks of commanded drive with no progress,
/// the bot backs out on an arc for [`REVERSE_TICKS`]; the arc's side alternates by tank id so a
/// deadlocked pair swings apart instead of mirroring each other back into the same contact.
fn bot_unstuck_command(agent: &mut BotAgent, tank: &TankState) -> Option<TankCommand> {
    let moved_m = agent.last_position.map_or(f32::MAX, |previous| previous.distance(tank.position));
    agent.last_position = Some(tank.position);
    if agent.reverse_ticks > 0 {
        agent.reverse_ticks -= 1;
        if agent.reverse_ticks == 0 {
            // Back-out done: arc sideways before the route brain aims straight back at the
            // waypoint (and straight back into whatever the bot was wedged on).
            agent.sidestep_ticks = SIDESTEP_TICKS;
        }
        return Some(bot_reverse_command(agent.escape_left));
    }
    if agent.sidestep_ticks > 0 {
        agent.sidestep_ticks -= 1;
        let steer = if agent.escape_left { 0.55 } else { -0.55 };
        return Some(TankCommand { throttle: 0.75, steer, ..TankCommand::idle() });
    }
    if moved_m < STALL_PROGRESS_EPS_M {
        agent.stall_ticks += 1;
    } else {
        agent.stall_ticks = 0;
    }
    if agent.stall_ticks >= STALL_TICKS_TO_REVERSE {
        agent.stall_ticks = 0;
        agent.reverse_ticks = REVERSE_TICKS;
        // A new escape attempt tries the other side than the last one.
        agent.escape_left = !agent.escape_left;
        return Some(bot_reverse_command(agent.escape_left));
    }
    None
}

/// The engaged target for this tick: the cached one while the cheap gates hold, a full
/// re-selection (the only per-bot LOS raycasts) on the reselect cadence or the moment the
/// cache fails. Full selection every tick was the single hottest per-tick cost in a 7v7 —
/// up to seven 400 m raycasts per bot per tick, duplicating what spotting already knew.
fn bot_current_target<'a>(
    agent: &mut BotAgent,
    tick: u64,
    tank: &TankState,
    tanks: &'a [TankState],
    battlefield: &BattlefieldMap,
) -> Option<&'a TankState> {
    // The futile hold: this bot walked away from that duel — the held enemy is out of the
    // candidate pool entirely, so a second visible enemy can still be fought.
    let held = agent.futile_hold.map(|(id, _)| id);
    let cached = agent
        .target
        .and_then(|id| find_tank(tanks, id))
        .filter(|target| Some(target.id) != held && bot_target_still_engageable(tank, target));
    let target = if cached.is_none() || cadence_due(tick, tank.id, TARGET_RESELECT_INTERVAL_TICKS) {
        bot_nearest_engageable_enemy(
            tank,
            tanks,
            held,
            Some(&battlefield.heightmap),
            &battlefield.static_cover,
        )
    } else {
        cached
    };
    agent.target = target.map(|target| target.id);
    target
}

fn bot_reverse_command(left: bool) -> TankCommand {
    let steer = if left { 0.45 } else { -0.45 };
    TankCommand {
        throttle: -0.7,
        steer,
        brake: 0.0,
        turret_yaw_delta: 0.0,
        gun_pitch_delta: 0.0,
        fire: false,
        select_ammo: None,
    }
}

/// Test-only tank constructors shared by the bot modules' tests.
#[cfg(test)]
pub(crate) mod test_support {
    use game_core::{TankId, TeamId};
    use glam::Vec3;
    use sim::TankState;

    pub(crate) fn tank_with_mask(
        id: u64,
        team: TeamId,
        position: Vec3,
        spotted_mask: u8,
    ) -> TankState {
        let spec = game_core::VehicleKind::T54_1951.spec();
        let modules = spec.module_health;
        let ammo_counts = spec.ammo.counts;
        let selected_ammo = spec.ammo.initial_selected;
        TankState {
            id: TankId(id),
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
            spotted_mask,
            submerged_s: 0.0,
            repair: sim::CrewRepair::default(),
        }
    }

    pub(crate) fn tank_at(id: u64, team: TeamId, position: Vec3) -> TankState {
        tank_with_mask(id, team, position, team.spotting_bit())
    }
}

#[cfg(test)]
mod tests {
    use game_core::TeamId;

    use super::test_support::tank_with_mask as tank;
    use super::*;

    /// The route brain only drives forward, so a physically blocked bot (nose-to-nose with
    /// another bot, pinned on the player, wedged on cover) used to push into the contact for the
    /// rest of the battle. Locked here: zero progress under a drive command flips the bot into a
    /// reverse arc, and after the back-out it resumes its route.
    #[test]
    fn a_blocked_bot_backs_out_instead_of_pushing_forever() {
        let battlefield = terrain::prokhorovka_hill_252_2();
        let bot = tank(1, TeamId(1), Vec3::new(300.0, 0.0, 300.0), TeamId(1).spotting_bit());
        let mut roster = BotRoster::new(vec![bot.id], BattleSeed::fixed(7));
        let command = |roster: &mut BotRoster| {
            roster.commands(0, std::slice::from_ref(&bot), &battlefield, false)[0].1
        };

        assert!(command(&mut roster).throttle > 0.0, "the route brain drives forward");

        // The hull never moves (blocked): within the stall window the bot must flip to reverse.
        let mut reversed_after = None;
        for tick in 0..STALL_TICKS_TO_REVERSE + 2 {
            if command(&mut roster).throttle < 0.0 {
                reversed_after = Some(tick);
                break;
            }
        }
        assert!(reversed_after.is_some(), "a stalled bot must back out");

        // The back-out runs its course and the route resumes.
        let mut resumed = false;
        for _ in 0..REVERSE_TICKS + 2 {
            if command(&mut roster).throttle > 0.0 {
                resumed = true;
                break;
            }
        }
        assert!(resumed, "after backing out the bot resumes its route");
    }

    /// Standing on station is not a stall. The unstuck brain fires on zero progress under a
    /// DRIVE command; an overwatch bot holding its shelf stands still on purpose, and before the
    /// movement-intent gate it bounced between hold and reverse every 1.5 s forever.
    #[test]
    fn an_overwatch_bot_holding_station_never_reads_as_stuck() {
        let battlefield = terrain::prokhorovka_hill_252_2();
        let shelf = battlefield
            .strategic_points
            .iter()
            .find(|point| point.id == "hill_hulldown_south")
            .expect("authored shelf");
        // Index 2 takes the overwatch posture (see `bot_posture`).
        let mut roster =
            BotRoster::new(vec![TankId(10), TankId(11), TankId(12)], BattleSeed::fixed(7));
        let mut bot = test_support::tank_at(12, TeamId(1), Vec3::from_array(shelf.position));
        bot.yaw_rad = crate::bot_routes::bot_yaw_to(
            bot.position,
            Vec3::new(battlefield.size_m[0] * 0.5, bot.position.y, battlefield.size_m[1] * 0.5),
        );

        for tick in 0..STALL_TICKS_TO_REVERSE * 3 {
            let commands = roster.commands(0, std::slice::from_ref(&bot), &battlefield, false);
            let (_, command) =
                *commands.iter().find(|(id, _)| *id == TankId(12)).expect("overwatch command");
            assert!(
                command.throttle >= 0.0,
                "tick {tick}: the hold flipped into a reverse (stuck misread)"
            );
            assert!(command.brake > 0.0 || command.throttle > 0.0, "the shelf is held actively");
        }
    }

    /// The performance contract behind the bot brain's cadences, locked as behavior: between
    /// re-selection ticks a bot HOLDS its engaged target (a human gunner does not flick every
    /// 16 ms), and on the cadence tick it re-selects the nearest visible enemy. A target that
    /// dies is dropped the same tick regardless of cadence.
    #[test]
    fn an_engaged_target_is_held_between_reselect_ticks_and_dropped_when_dead() {
        let battlefield = terrain::prokhorovka_hill_252_2();
        let grounded = |x: f32, z: f32| {
            Vec3::new(x, battlefield.heightmap.sample_height(x, z).expect("inside the map"), z)
        };
        let bot_id = TankId(1);
        let mask = TeamId(1).spotting_bit() | TeamId(2).spotting_bit();
        let bot = tank(1, TeamId(1), grounded(300.0, 300.0), TeamId(1).spotting_bit());
        let first = tank(2, TeamId(2), grounded(300.0, 380.0), mask);
        let mut roster = BotRoster::new(vec![bot_id], BattleSeed::fixed(7));

        // Engage on a cadence tick (tick + id divisible by the reselect interval).
        let due_tick = TARGET_RESELECT_INTERVAL_TICKS - bot_id.0 % TARGET_RESELECT_INTERVAL_TICKS;
        let tanks = [bot.clone(), first.clone()];
        let engaged = roster.commands(due_tick, &tanks, &battlefield, false)[0].1;
        assert!(engaged.brake > 0.0, "the bot stands to fight the spotted enemy");
        assert_eq!(roster.agents[0].target, Some(first.id));

        // A NEARER enemy appears off-cadence: the held target must not flick.
        let nearer = tank(3, TeamId(2), grounded(300.0, 340.0), mask);
        let tanks = [bot.clone(), first.clone(), nearer.clone()];
        roster.commands(due_tick + 1, &tanks, &battlefield, false);
        assert_eq!(
            roster.agents[0].target,
            Some(first.id),
            "off-cadence the engaged target is held, not flicked"
        );

        // On the next cadence tick the re-selection switches to the nearer enemy.
        roster.commands(due_tick + TARGET_RESELECT_INTERVAL_TICKS, &tanks, &battlefield, false);
        assert_eq!(roster.agents[0].target, Some(nearer.id), "cadence tick re-selects nearest");

        // The held target dying is acted on immediately, cadence or not.
        let mut dead_near = nearer.clone();
        dead_near.hit_points = 0;
        let tanks = [bot.clone(), first.clone(), dead_near];
        roster.commands(due_tick + TARGET_RESELECT_INTERVAL_TICKS + 1, &tanks, &battlefield, false);
        assert_eq!(
            roster.agents[0].target,
            Some(first.id),
            "a dead target is dropped and replaced the same tick"
        );
    }

    /// Survival preempts everything: a hull past the deep-water line backs out immediately —
    /// it does not stand and fight, and it does not wait out the stall counter while flooding.
    #[test]
    fn a_bot_in_deep_water_backs_out_before_anything_else() {
        let battlefield = terrain::bystra_valley();
        // Mid-channel, away from every crossing window (bridge z=500±45, fords z=500±180±40).
        let x = terrain::bystra_river_center_x(400.0);
        let ground = battlefield.heightmap.sample_height(x, 400.0).expect("in the map");
        let bot = test_support::tank_at(1, TeamId(1), Vec3::new(x, ground, 400.0));
        assert!(
            crate::bot_routes::bot_in_deep_water(&bot, &battlefield),
            "mid-channel must read as deep (test premise)"
        );

        let mut roster = BotRoster::new(vec![bot.id], BattleSeed::fixed(7));
        let command = roster.commands(0, std::slice::from_ref(&bot), &battlefield, false)[0].1;
        assert!(command.throttle < 0.0, "a drowning bot reverses out, got {command:?}");
    }

    #[test]
    fn dead_or_absent_tanks_idle_and_a_finished_battle_idles_everyone() {
        let battlefield = terrain::prokhorovka_hill_252_2();
        let mut bot = tank(1, TeamId(1), Vec3::new(300.0, 0.0, 300.0), TeamId(1).spotting_bit());
        let mut roster = BotRoster::new(vec![bot.id, TankId(99)], BattleSeed::fixed(7));

        // Battle over: even a live bot receives idle.
        let over = roster.commands(0, std::slice::from_ref(&bot), &battlefield, true);
        assert!(over.iter().all(|(_, command)| *command == TankCommand::idle()));

        // A dead bot and a bot with no tank in the snapshot both idle.
        bot.hit_points = 0;
        let commands = roster.commands(0, std::slice::from_ref(&bot), &battlefield, false);
        assert!(commands.iter().all(|(_, command)| *command == TankCommand::idle()));
    }
}
