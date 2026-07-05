use game_core::TankId;
use game_core::math::wrap_angle;
use glam::Vec3;
use sim::{MAX_GUN_PITCH_RAD, MIN_GUN_PITCH_RAD, TankCommand, TankState, VIEW_RANGE_M};
use terrain::BattlefieldMap;

use crate::battle::BattleSeed;
use crate::bot_routes::{bot_route_command, bot_yaw_to, seed_route_index};

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
    /// Hull position at the previous command, for stall detection.
    last_position: Option<Vec3>,
    /// Consecutive ticks the bot commanded drive but the hull did not move.
    stall_ticks: u32,
    /// Remaining ticks of the current back-out maneuver.
    reverse_ticks: u32,
}

impl BotAgent {
    fn new(tank_id: TankId, route_index: usize) -> Self {
        Self { tank_id, route_index, last_position: None, stall_ticks: 0, reverse_ticks: 0 }
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
            .map(|(index, tank_id)| BotAgent::new(tank_id, seed_route_index(seed, index)))
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
    if let Some(target) = bot_nearest_visible_enemy(tank, tanks) {
        // Standing to shoot is intentional, not a stall.
        agent.stall_ticks = 0;
        agent.last_position = Some(tank.position);
        return bot_combat_command(tank, target);
    }
    if let Some(command) = bot_unstuck_command(agent, tank) {
        return command;
    }
    bot_route_command(&mut agent.route_index, tank, battlefield)
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

fn bot_nearest_visible_enemy<'a>(
    tank: &TankState,
    tanks: &'a [TankState],
) -> Option<&'a TankState> {
    tanks
        .iter()
        .filter(|target| {
            target.team != tank.team
                && target.hit_points > 0
                && target.position.distance(tank.position) <= VIEW_RANGE_M
                && target.spotted_mask & tank.team.spotting_bit() != 0
        })
        .min_by(|a, b| {
            tank.position
                .distance_squared(a.position)
                .total_cmp(&tank.position.distance_squared(b.position))
        })
}

fn bot_combat_command(tank: &TankState, target: &TankState) -> TankCommand {
    let aim = bot_aim_solution(tank, target.position + Vec3::Y * target.spec.hitbox.center_y_m);
    TankCommand {
        throttle: 0.0,
        steer: 0.0,
        brake: 0.35,
        turret_yaw_delta: (aim.turret_error * 4.0).clamp(-1.0, 1.0),
        gun_pitch_delta: (aim.pitch_error * 4.0).clamp(-1.0, 1.0),
        fire: aim.turret_error.abs() < 0.08
            && aim.pitch_error.abs() < 0.06
            && tank.reload_remaining_s <= 0.0,
        select_ammo: None,
    }
}

#[derive(Debug, Clone, Copy)]
struct BotAimSolution {
    turret_error: f32,
    pitch_error: f32,
}

fn bot_aim_solution(tank: &TankState, target: Vec3) -> BotAimSolution {
    let delta = target - tank.position;
    let desired_yaw = bot_yaw_to(tank.position, target);
    let desired_turret = wrap_angle(desired_yaw - tank.yaw_rad);
    let flat = Vec3::new(delta.x, 0.0, delta.z).length().max(1.0);
    let desired_pitch = (delta.y / flat).atan().clamp(MIN_GUN_PITCH_RAD, MAX_GUN_PITCH_RAD);
    BotAimSolution {
        turret_error: wrap_angle(desired_turret - tank.turret_yaw_rad),
        pitch_error: desired_pitch - tank.gun_pitch_rad,
    }
}

#[cfg(test)]
mod tests {
    use game_core::TeamId;

    use super::*;

    fn tank(id: u64, team: TeamId, position: Vec3, spotted_mask: u8) -> TankState {
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
        }
    }

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

    #[test]
    fn bots_target_only_enemies_spotted_by_their_team() {
        let observer = tank(1, TeamId(1), Vec3::new(300.0, 0.0, 300.0), TeamId(1).spotting_bit());
        let unspotted_enemy =
            tank(2, TeamId(2), Vec3::new(305.0, 0.0, 305.0), TeamId(2).spotting_bit());

        assert!(
            bot_nearest_visible_enemy(&observer, &[observer.clone(), unspotted_enemy]).is_none(),
            "bot AI must reuse the authoritative spotting mask instead of running a private LOS target"
        );
    }
}
