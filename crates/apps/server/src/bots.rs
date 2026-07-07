use game_core::TankId;
use glam::Vec3;
use sim::{TankCommand, TankState};
use terrain::BattlefieldMap;

use crate::battle::BattleSeed;
use crate::bot_combat::{bot_combat_command, bot_nearest_engageable_enemy};
use crate::bot_routes::{BotPosture, bot_posture, bot_route_command, seed_route_index};

/// How little per-tick progress counts as "not moving" while the bot commands forward drive
/// (0.01 m/tick = 0.6 m/s at 60 Hz — well under the slowest route crawl).
const STALL_PROGRESS_EPS_M: f32 = 0.01;
/// Ticks of no progress under a drive command before the bot decides it is stuck (1.5 s).
const STALL_TICKS_TO_REVERSE: u32 = 90;
/// How long a stuck bot backs out before resuming its route (~1.3 s, a few hull-lengths' arc).
const REVERSE_TICKS: u32 = 80;

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
}

impl BotAgent {
    fn new(tank_id: TankId, route_index: usize, posture: BotPosture) -> Self {
        Self {
            tank_id,
            route_index,
            posture,
            last_position: None,
            stall_ticks: 0,
            reverse_ticks: 0,
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
                        bot_command_for_tank(&mut self.agents[index], tank, tanks, battlefield)
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
    tank: &TankState,
    tanks: &[TankState],
    battlefield: &BattlefieldMap,
) -> TankCommand {
    if let Some(target) = bot_nearest_engageable_enemy(
        tank,
        tanks,
        Some(&battlefield.heightmap),
        &battlefield.static_cover,
    ) {
        // Standing to shoot is intentional, not a stall.
        agent.stall_ticks = 0;
        agent.last_position = Some(tank.position);
        return bot_combat_command(tank, target);
    }
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
        return Some(bot_reverse_command(tank));
    }
    if moved_m < STALL_PROGRESS_EPS_M {
        agent.stall_ticks += 1;
    } else {
        agent.stall_ticks = 0;
    }
    if agent.stall_ticks >= STALL_TICKS_TO_REVERSE {
        agent.stall_ticks = 0;
        agent.reverse_ticks = REVERSE_TICKS;
        return Some(bot_reverse_command(tank));
    }
    None
}

fn bot_reverse_command(tank: &TankState) -> TankCommand {
    let steer = if tank.id.0.is_multiple_of(2) { 0.45 } else { -0.45 };
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
            roster.commands(std::slice::from_ref(&bot), &battlefield, false)[0].1
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
            let commands = roster.commands(std::slice::from_ref(&bot), &battlefield, false);
            let (_, command) =
                *commands.iter().find(|(id, _)| *id == TankId(12)).expect("overwatch command");
            assert!(
                command.throttle >= 0.0,
                "tick {tick}: the hold flipped into a reverse (stuck misread)"
            );
            assert!(command.brake > 0.0 || command.throttle > 0.0, "the shelf is held actively");
        }
    }

    #[test]
    fn dead_or_absent_tanks_idle_and_a_finished_battle_idles_everyone() {
        let battlefield = terrain::prokhorovka_hill_252_2();
        let mut bot = tank(1, TeamId(1), Vec3::new(300.0, 0.0, 300.0), TeamId(1).spotting_bit());
        let mut roster = BotRoster::new(vec![bot.id, TankId(99)], BattleSeed::fixed(7));

        // Battle over: even a live bot receives idle.
        let over = roster.commands(std::slice::from_ref(&bot), &battlefield, true);
        assert!(over.iter().all(|(_, command)| *command == TankCommand::idle()));

        // A dead bot and a bot with no tank in the snapshot both idle.
        bot.hit_points = 0;
        let commands = roster.commands(std::slice::from_ref(&bot), &battlefield, false);
        assert!(commands.iter().all(|(_, command)| *command == TankCommand::idle()));
    }
}
