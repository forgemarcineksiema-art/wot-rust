use game_core::math::wrap_angle;
use game_core::{DamageCause, DamageEvent, TankId};
use glam::Vec3;
use sim::{TankCommand, TankState};
use terrain::BattlefieldMap;

use crate::battle::BattleSeed;
use crate::bot_aim::{BotFiringSolution, solve_firing_solution};
use crate::bot_combat::{
    bot_combat_command, bot_nearest_engageable_enemy, bot_target_still_engageable, find_tank,
};
use crate::bot_routes::{
    BotPosture, ROUTE_REPLAN_INTERVAL_TICKS, RoutePlan, bot_posture, bot_route_command,
    seed_route_index,
};

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
/// Cold acquisition (no target held at all) runs on this short stagger instead of every tick,
/// so first contact spreads its raycast burst over a few ticks instead of stacking the whole
/// roster's sweeps onto the same tick as the spotting recompute. ≤ 50 ms of reaction delay —
/// human, and invisible next to the turret slew time. A bot whose target just DIED still swaps
/// hot (see `bot_current_target`).
const ACQUIRE_INTERVAL_TICKS: u64 = 3;

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
/// How long a hit from an unseen gun keeps the crew turning toward the struck side (4 s):
/// long enough to bring the bow and turret around and catch the shooter's next tracer, short
/// enough that one stray round does not park the bot for good. The bearing comes from the
/// strike point on the bot's own hull — the crew knows which side rang, and NOTHING more; the
/// shooter's position is never read, so an unspotted marksman stays unspotted until the turned
/// bot actually sees something.
const THREAT_FACE_TICKS: u32 = 240;

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
    /// A hit from a gun this bot cannot see: the world-yaw bearing of the strike point on its
    /// own hull and the ticks left of turning to face it. See [`THREAT_FACE_TICKS`].
    threat: Option<(f32, u32)>,
    /// The cached water-detour plan; replanned on [`ROUTE_REPLAN_INTERVAL_TICKS`] or when the
    /// objective changes. See `bot_routes::RoutePlan`.
    route_plan: Option<RoutePlan>,
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
            threat: None,
            route_plan: None,
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

    /// `live_cover` is the cover the battle actually blocks with THIS tick (rubble lowered,
    /// destroyed objects omitted) — the same slice every authoritative consumer uses. Bots must
    /// never raycast the raw authored cover: a bot would keep "hiding" behind a building the
    /// battle has already flattened, and would refuse to fire through the hole it just made.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn commands(
        &mut self,
        tick: u64,
        tanks: &[TankState],
        battlefield: &BattlefieldMap,
        ground: Option<&terrain::GroundClassifier>,
        live_cover: &[terrain::StaticCoverObject],
        battle_over: bool,
        damage_events: &[DamageEvent],
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
                        note_incoming_fire(&mut self.agents[index], tank, damage_events);
                        bot_command_for_tank(
                            &mut self.agents[index],
                            tick,
                            tank,
                            tanks,
                            battlefield,
                            ground,
                            live_cover,
                        )
                    }
                },
            );
            commands.push((tank_id, command));
        }
        commands
    }
}

/// Remember last tick's incoming fire as a threat bearing. The crew's honest knowledge is the
/// strike point on their OWN hull — its bearing says which side rang, and that is all the
/// reaction may use. `event.source` is deliberately never resolved to a position.
fn note_incoming_fire(agent: &mut BotAgent, tank: &TankState, damage_events: &[DamageEvent]) {
    for event in damage_events {
        if event.target != tank.id
            || !matches!(event.cause, DamageCause::Shell | DamageCause::Splash)
        {
            continue;
        }
        let strike = event.hit_position - tank.position;
        if strike.x.abs().max(strike.z.abs()) > 1.0e-3 {
            agent.threat = Some((strike.x.atan2(strike.z), THREAT_FACE_TICKS));
        }
    }
}

fn bot_command_for_tank(
    agent: &mut BotAgent,
    tick: u64,
    tank: &TankState,
    tanks: &[TankState],
    battlefield: &BattlefieldMap,
    ground: Option<&terrain::GroundClassifier>,
    live_cover: &[terrain::StaticCoverObject],
) -> TankCommand {
    // Survival preempts everything, combat included: a hull past the route brain's deep-water
    // line — or driving at it with no room to stop — is heading for a flooded engine. It takes
    // the shortest way back to shallow water before doing anything else. Fords (<= 0.9 m) never
    // trip this; the escape resets the stall counter so the slow wade out is not misread as
    // being stuck.
    if crate::bot_routes::bot_in_deep_water(tank, battlefield, ground) {
        agent.stall_ticks = 0;
        agent.last_position = Some(tank.position);
        return crate::bot_routes::water_escape_command(tank, battlefield, ground);
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
        && let Some(target) = bot_current_target(agent, tick, tank, tanks, battlefield, live_cover)
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
            // Standing to shoot is intentional, not a stall. A live engagement also outranks
            // any hit bearing — the crew is already fighting the enemy it can see.
            agent.threat = None;
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
            return bot_combat_command(tank, &solution, tanks);
        }
    } else if !repositioning {
        agent.futile_watch = None;
    }
    agent.solution = None;
    // Shot by a gun nobody on the team can see: turn hull and turret toward the struck side
    // and look for the shooter. Reposition still outranks this — a bot escaping a futile duel
    // must actually relocate, not be pinned in place by the very fire it is escaping.
    if !repositioning && let Some((bearing, ticks)) = agent.threat {
        let remaining = ticks.saturating_sub(1);
        agent.threat = (remaining > 0).then_some((bearing, remaining));
        // Standing to face the threat is intentional, not a stall.
        agent.stall_ticks = 0;
        agent.last_position = Some(tank.position);
        return bot_face_threat_command(tank, bearing);
    }
    let posture = agent.posture;
    let replan_due = cadence_due(tick, tank.id, ROUTE_REPLAN_INTERVAL_TICKS);
    let command = bot_route_command(
        &mut agent.route_index,
        &mut agent.route_plan,
        replan_due,
        posture,
        tank,
        battlefield,
    );
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
///
/// Acquisition is two-speed. A bot that JUST lost its target swaps hot, same tick — a fighting
/// crew shifts to the next threat it already sees, and mass same-tick losses are rare. COLD
/// acquisition (no engagement at all) rides its own short stagger: before first contact every
/// cache is empty, so the moment a spotting recompute lit up both teams, all 13 bots used to
/// fire their full raycast sweeps on that very tick — the recompute's own most expensive tick.
fn bot_current_target<'a>(
    agent: &mut BotAgent,
    tick: u64,
    tank: &TankState,
    tanks: &'a [TankState],
    battlefield: &BattlefieldMap,
    live_cover: &[terrain::StaticCoverObject],
) -> Option<&'a TankState> {
    // The futile hold: this bot walked away from that duel — the held enemy is out of the
    // candidate pool entirely, so a second visible enemy can still be fought.
    let held = agent.futile_hold.map(|(id, _)| id);
    let had_target = agent.target.is_some();
    let cached = agent
        .target
        .and_then(|id| find_tank(tanks, id))
        .filter(|target| Some(target.id) != held && bot_target_still_engageable(tank, target));
    let select_due = if cached.is_some() {
        cadence_due(tick, tank.id, TARGET_RESELECT_INTERVAL_TICKS)
    } else {
        had_target || cadence_due(tick, tank.id, ACQUIRE_INTERVAL_TICKS)
    };
    let target = if select_due {
        bot_nearest_engageable_enemy(tank, tanks, held, Some(&battlefield.heightmap), live_cover)
    } else {
        cached
    };
    agent.target = target.map(|target| target.id);
    target
}

/// Pivot the hull and slew the turret toward the threat bearing; no fire, no drive. The same
/// steering gain as the route brain, so the turn rate is nothing a driving bot could not do.
fn bot_face_threat_command(tank: &TankState, bearing: f32) -> TankCommand {
    let hull_error = wrap_angle(bearing - tank.yaw_rad);
    let turret_error = wrap_angle(bearing - tank.yaw_rad - tank.turret_yaw_rad);
    TankCommand {
        throttle: 0.0,
        steer: (hull_error * 1.8).clamp(-1.0, 1.0),
        turret_yaw_delta: (turret_error * 4.0).clamp(-1.0, 1.0),
        ..TankCommand::idle()
    }
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
            fire_buffered: false,
            aim_dispersion_mrad: 0.0,
            dispersion_shot_index: 0,
            tracks: game_core::TrackHealth::healthy(),
            modules,
            ammo_counts,
            selected_ammo,
            spotted_mask,
            submerged_s: 0.0,
            repair: sim::CrewRepair::default(),
            turret_detached: false,
            armor_breaches: Default::default(),
            track_break_t: [None, None],
            engine_fire: false,
            fuel_fire: false,
            fire_source: None,
            fire_s: 0.0,
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
        let battlefield = map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2);
        let bot = tank(1, TeamId(1), Vec3::new(300.0, 0.0, 300.0), TeamId(1).spotting_bit());
        let mut roster = BotRoster::new(vec![bot.id], BattleSeed::fixed(7));
        let command = |roster: &mut BotRoster| {
            roster.commands(
                0,
                std::slice::from_ref(&bot),
                &battlefield,
                None,
                &battlefield.static_cover,
                false,
                &[],
            )[0]
            .1
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

    /// The bots' own LOS raycasts must follow the battle's LIVE cover: a building the fight
    /// has flattened no longer hides anyone, and a bot must engage straight through the gap it
    /// (or anyone) just made. Before this lock the roster raycast the raw authored cover and
    /// diverged from the authoritative spotting truth for the rest of the battle.
    #[test]
    fn a_bot_engages_through_cover_the_battle_has_destroyed() {
        let battlefield = map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2);
        let mask = TeamId(1).spotting_bit() | TeamId(2).spotting_bit();
        let ground = |x: f32, z: f32| battlefield.heightmap.sample_height(x, z).unwrap_or(0.0);
        let bot = tank(1, TeamId(1), Vec3::new(300.0, ground(300.0, 300.0), 300.0), mask);
        let enemy = tank(2, TeamId(2), Vec3::new(300.0, ground(300.0, 360.0), 360.0), mask);
        let tanks = [bot.clone(), enemy.clone()];
        // A wall square across the only sight line, tall enough to ignore the terrain sample.
        let wall = terrain::StaticCoverObject {
            id: "wall".into(),
            name: "wall".into(),
            kind: terrain::StaticCoverKind::FarmBuilding,
            center: [300.0, ground(300.0, 330.0) + 5.0, 330.0],
            half_extents_m: [20.0, 10.0, 2.0],
        };
        let target_after = |live_cover: &[terrain::StaticCoverObject]| {
            let mut roster = BotRoster::new(vec![bot.id], BattleSeed::fixed(7));
            // Run past the cold-acquisition stagger so the selection cadence certainly fired.
            for tick in 0..=ACQUIRE_INTERVAL_TICKS {
                roster.commands(tick, &tanks, &battlefield, None, live_cover, false, &[]);
            }
            roster.agents[0].target
        };

        assert_eq!(
            target_after(std::slice::from_ref(&wall)),
            None,
            "an intact wall on the sight line must block the bot's own acquisition"
        );
        assert_eq!(
            target_after(&[]),
            Some(enemy.id),
            "with the wall destroyed (omitted from live cover) the bot must engage"
        );
    }

    /// Standing on station is not a stall. The unstuck brain fires on zero progress under a
    /// DRIVE command; an overwatch bot holding its shelf stands still on purpose, and before the
    /// movement-intent gate it bounced between hold and reverse every 1.5 s forever.
    #[test]
    fn an_overwatch_bot_holding_station_never_reads_as_stuck() {
        let battlefield = map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2);
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
            let commands = roster.commands(
                0,
                std::slice::from_ref(&bot),
                &battlefield,
                None,
                &battlefield.static_cover,
                false,
                &[],
            );
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
        let battlefield = map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2);
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
        let engaged = roster.commands(
            due_tick,
            &tanks,
            &battlefield,
            None,
            &battlefield.static_cover,
            false,
            &[],
        )[0]
        .1;
        assert!(engaged.brake > 0.0, "the bot stands to fight the spotted enemy");
        assert_eq!(roster.agents[0].target, Some(first.id));

        // A NEARER enemy appears off-cadence: the held target must not flick.
        let nearer = tank(3, TeamId(2), grounded(300.0, 340.0), mask);
        let tanks = [bot.clone(), first.clone(), nearer.clone()];
        roster.commands(
            due_tick + 1,
            &tanks,
            &battlefield,
            None,
            &battlefield.static_cover,
            false,
            &[],
        );
        assert_eq!(
            roster.agents[0].target,
            Some(first.id),
            "off-cadence the engaged target is held, not flicked"
        );

        // On the next cadence tick the re-selection switches to the nearer enemy.
        roster.commands(
            due_tick + TARGET_RESELECT_INTERVAL_TICKS,
            &tanks,
            &battlefield,
            None,
            &battlefield.static_cover,
            false,
            &[],
        );
        assert_eq!(roster.agents[0].target, Some(nearer.id), "cadence tick re-selects nearest");

        // The held target dying is acted on immediately, cadence or not.
        let mut dead_near = nearer.clone();
        dead_near.hit_points = 0;
        let tanks = [bot.clone(), first.clone(), dead_near];
        roster.commands(
            due_tick + TARGET_RESELECT_INTERVAL_TICKS + 1,
            &tanks,
            &battlefield,
            None,
            &battlefield.static_cover,
            false,
            &[],
        );
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
        let battlefield = map_forge::battlefield(terrain::MapId::BystraValley);
        // Mid-channel, away from every crossing window (bridge z=500±45, fords z=500±180±40).
        let x = terrain::bystra_river_center_x(400.0);
        let ground = battlefield.heightmap.sample_height(x, 400.0).expect("in the map");
        let bot = test_support::tank_at(1, TeamId(1), Vec3::new(x, ground, 400.0));
        assert!(
            crate::bot_routes::bot_in_deep_water(&bot, &battlefield, None),
            "mid-channel must read as deep (test premise)"
        );

        let mut roster = BotRoster::new(vec![bot.id], BattleSeed::fixed(7));
        let command = roster.commands(
            0,
            std::slice::from_ref(&bot),
            &battlefield,
            None,
            &battlefield.static_cover,
            false,
            &[],
        )[0]
        .1;
        assert!(command.throttle < 0.0, "a drowning bot reverses out, got {command:?}");
    }

    /// Cold acquisition rides its own stagger: when a spotting recompute lights up the enemy
    /// team for everyone at once, the roster's raycast sweeps spread over the next
    /// ACQUIRE_INTERVAL ticks instead of stacking on the recompute's own tick. Each bot locks
    /// on within 50 ms — and the existing dead-target test locks that a bot mid-fight still
    /// swaps hot, same tick.
    #[test]
    fn first_contact_acquisition_spreads_across_ticks() {
        let battlefield = map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2);
        let grounded = |x: f32, z: f32| {
            Vec3::new(x, battlefield.heightmap.sample_height(x, z).expect("inside the map"), z)
        };
        let mask = TeamId(1).spotting_bit() | TeamId(2).spotting_bit();
        let bots = [
            tank(1, TeamId(1), grounded(290.0, 300.0), mask),
            tank(2, TeamId(1), grounded(310.0, 300.0), mask),
        ];
        let enemy = tank(9, TeamId(2), grounded(300.0, 380.0), mask);
        let mut roster = BotRoster::new(vec![bots[0].id, bots[1].id], BattleSeed::fixed(7));
        let world = [bots[0].clone(), bots[1].clone(), enemy.clone()];

        // Tick 30: neither bot's acquire slice is due ((30+1)%3=1, (30+2)%3=2) — nobody
        // raycasts on the hypothetical recompute tick itself.
        roster.commands(30, &world, &battlefield, None, &battlefield.static_cover, false, &[]);
        assert_eq!(roster.agents[0].target, None);
        assert_eq!(roster.agents[1].target, None);

        // Tick 31: bot 2's slice. Tick 32: bot 1's. Everyone is locked within the interval.
        roster.commands(31, &world, &battlefield, None, &battlefield.static_cover, false, &[]);
        assert_eq!(roster.agents[0].target, None);
        assert_eq!(roster.agents[1].target, Some(enemy.id));
        roster.commands(32, &world, &battlefield, None, &battlefield.static_cover, false, &[]);
        assert_eq!(roster.agents[0].target, Some(enemy.id));
        assert_eq!(roster.agents[1].target, Some(enemy.id));
    }

    /// A bot shot by a gun nobody on its team can see must not die oblivious: it turns hull and
    /// turret toward the side the round struck. The reaction is honest by construction — the
    /// event's `source` id belongs to no tank in the roster's world slice, so there is nothing
    /// to look up; the bearing comes from the strike point on the bot's own armor.
    #[test]
    fn a_bot_shot_by_an_unseen_gun_turns_toward_the_struck_side() {
        let battlefield = map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2);
        let bot = tank(1, TeamId(1), Vec3::new(300.0, 0.0, 300.0), TeamId(1).spotting_bit());
        let mut roster = BotRoster::new(vec![bot.id], BattleSeed::fixed(7));
        // A round into the LEFT flank (-X), slightly forward, from a shooter that exists
        // nowhere in the passed world.
        let hit = game_core::DamageEvent {
            source: TankId(99),
            target: bot.id,
            hit_position: bot.position + Vec3::new(-1.05, 0.8, 0.3),
            damage_hp: 90,
            penetrated: true,
            cause: DamageCause::Shell,
            ..Default::default()
        };

        let command = roster.commands(
            1,
            std::slice::from_ref(&bot),
            &battlefield,
            None,
            &battlefield.static_cover,
            false,
            &[hit],
        )[0]
        .1;

        assert_eq!(command.throttle, 0.0, "the bot stands to scan, got {command:?}");
        assert!(command.steer < -0.5, "the hull pivots toward the struck LEFT side");
        assert!(command.turret_yaw_delta < -0.5, "the turret slews the same way");
        assert!(!command.fire, "nothing to shoot at yet");
    }

    /// The threat window runs out: one stray round turns the bot for a few seconds, then the
    /// route resumes — a single hit must not park a bot for the rest of the battle.
    #[test]
    fn threat_facing_expires_back_into_the_route() {
        let battlefield = map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2);
        let bot = tank(1, TeamId(1), Vec3::new(300.0, 0.0, 300.0), TeamId(1).spotting_bit());
        let mut roster = BotRoster::new(vec![bot.id], BattleSeed::fixed(7));
        let hit = game_core::DamageEvent {
            source: TankId(99),
            target: bot.id,
            hit_position: bot.position + Vec3::new(-1.05, 0.8, 0.3),
            damage_hp: 90,
            penetrated: true,
            cause: DamageCause::Shell,
            ..Default::default()
        };
        roster.commands(
            1,
            std::slice::from_ref(&bot),
            &battlefield,
            None,
            &battlefield.static_cover,
            false,
            &[hit],
        );

        let mut resumed_after = None;
        for tick in 0..THREAT_FACE_TICKS + 2 {
            let command = roster.commands(
                u64::from(tick) + 2,
                std::slice::from_ref(&bot),
                &battlefield,
                None,
                &battlefield.static_cover,
                false,
                &[],
            )[0]
            .1;
            if command.throttle > 0.0 {
                resumed_after = Some(tick);
                break;
            }
        }
        let resumed = resumed_after.expect("the threat window must expire back into the route");
        assert!(
            resumed >= THREAT_FACE_TICKS - 2,
            "the bot faced the threat for the full window, resumed after {resumed} ticks"
        );
    }

    /// A live engagement outranks any hit bearing: a bot trading fire with a visible enemy does
    /// not spin away because a second, unseen gun clipped its rear.
    #[test]
    fn an_engaged_bot_ignores_hit_bearings_and_keeps_fighting() {
        let battlefield = map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2);
        let grounded = |x: f32, z: f32| {
            Vec3::new(x, battlefield.heightmap.sample_height(x, z).expect("inside the map"), z)
        };
        let bot_id = TankId(1);
        let mask = TeamId(1).spotting_bit() | TeamId(2).spotting_bit();
        let bot = tank(1, TeamId(1), grounded(300.0, 300.0), TeamId(1).spotting_bit());
        let enemy = tank(2, TeamId(2), grounded(300.0, 380.0), mask);
        let mut roster = BotRoster::new(vec![bot_id], BattleSeed::fixed(7));
        let due_tick = TARGET_RESELECT_INTERVAL_TICKS - bot_id.0 % TARGET_RESELECT_INTERVAL_TICKS;
        let rear_hit = game_core::DamageEvent {
            source: TankId(99),
            target: bot_id,
            hit_position: bot.position + Vec3::new(0.0, 0.8, -3.1),
            damage_hp: 90,
            penetrated: true,
            cause: DamageCause::Shell,
            ..Default::default()
        };

        let tanks = [bot.clone(), enemy.clone()];
        let command = roster.commands(
            due_tick,
            &tanks,
            &battlefield,
            None,
            &battlefield.static_cover,
            false,
            &[rear_hit],
        )[0]
        .1;

        assert!(command.brake > 0.0, "the bot stands and fights its visible enemy");
        assert_eq!(command.steer, 0.0, "no threat pivot while engaged");
        assert_eq!(roster.agents[0].target, Some(enemy.id));
        assert_eq!(roster.agents[0].threat, None, "the engagement clears the hit bearing");
    }

    #[test]
    fn dead_or_absent_tanks_idle_and_a_finished_battle_idles_everyone() {
        let battlefield = map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2);
        let mut bot = tank(1, TeamId(1), Vec3::new(300.0, 0.0, 300.0), TeamId(1).spotting_bit());
        let mut roster = BotRoster::new(vec![bot.id, TankId(99)], BattleSeed::fixed(7));

        // Battle over: even a live bot receives idle.
        let over = roster.commands(
            0,
            std::slice::from_ref(&bot),
            &battlefield,
            None,
            &battlefield.static_cover,
            true,
            &[],
        );
        assert!(over.iter().all(|(_, command)| *command == TankCommand::idle()));

        // A dead bot and a bot with no tank in the snapshot both idle.
        bot.hit_points = 0;
        let commands = roster.commands(
            0,
            std::slice::from_ref(&bot),
            &battlefield,
            None,
            &battlefield.static_cover,
            false,
            &[],
        );
        assert!(commands.iter().all(|(_, command)| *command == TankCommand::idle()));
    }
}
