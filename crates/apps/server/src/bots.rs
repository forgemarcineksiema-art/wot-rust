use game_core::math::wrap_angle;
use game_core::{TankId, TeamId};
use glam::Vec3;
use sim::{MAX_GUN_PITCH_RAD, MIN_GUN_PITCH_RAD, TankCommand, TankState, VIEW_RANGE_M};
use terrain::{BattlefieldMap, StrategicPoint, StrategicRole};

use crate::battle::BattleSeed;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BotAgent {
    tank_id: TankId,
    route_index: usize,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
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
            .map(|(index, tank_id)| BotAgent {
                tank_id,
                route_index: seed_route_index(seed, index),
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

fn seed_route_index(seed: BattleSeed, index: usize) -> usize {
    (seed_route_mix(index as u64 ^ seed.route_seed()) % 5) as usize
}

fn seed_route_mix(mut value: u64) -> u64 {
    value ^= value >> 33;
    value = value.wrapping_mul(0xff51_afd7_ed55_8ccd);
    value ^= value >> 33;
    value
}

fn bot_command_for_tank(
    agent: &mut BotAgent,
    tank: &TankState,
    tanks: &[TankState],
    battlefield: &BattlefieldMap,
) -> TankCommand {
    if let Some(target) = bot_nearest_visible_enemy(tank, tanks) {
        return bot_combat_command(tank, target);
    }
    bot_route_command(agent, tank, battlefield)
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

fn bot_route_command(
    agent: &mut BotAgent,
    tank: &TankState,
    battlefield: &BattlefieldMap,
) -> TankCommand {
    let target = bot_route_target(agent, tank.team, tank.position, battlefield);
    let desired_yaw = bot_yaw_to(tank.position, target);
    let yaw_error = wrap_angle(desired_yaw - tank.yaw_rad);
    TankCommand {
        throttle: if yaw_error.abs() > 1.2 { 0.35 } else { 0.78 },
        steer: (yaw_error * 1.8).clamp(-1.0, 1.0),
        brake: 0.0,
        turret_yaw_delta: 0.0,
        gun_pitch_delta: 0.0,
        fire: false,
        select_ammo: None,
    }
}

fn bot_route_target(
    agent: &mut BotAgent,
    team: TeamId,
    position: Vec3,
    battlefield: &BattlefieldMap,
) -> Vec3 {
    let points: Vec<&StrategicPoint> = battlefield
        .strategic_points
        .iter()
        .filter(|point| bot_point_matches_team(point, team))
        .collect();
    if points.is_empty() {
        return Vec3::new(battlefield.size_m[0] * 0.5, position.y, battlefield.size_m[1] * 0.5);
    }
    let point = points[agent.route_index % points.len()];
    let target = Vec3::from_array(point.position);
    if position.distance(target) < point.radius_m.max(25.0) {
        agent.route_index = (agent.route_index + 1) % points.len();
        Vec3::from_array(points[agent.route_index].position)
    } else {
        target
    }
}

fn bot_point_matches_team(point: &StrategicPoint, team: TeamId) -> bool {
    point.role == StrategicRole::Crossing
        || point.id == "oktyabrskiy"
        || (team == TeamId(1) && point.id.contains("south"))
        || (team == TeamId(2) && point.id.contains("north"))
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

fn bot_yaw_to(from: Vec3, to: Vec3) -> f32 {
    let delta = to - from;
    delta.x.atan2(delta.z)
}

#[cfg(test)]
mod tests {
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
