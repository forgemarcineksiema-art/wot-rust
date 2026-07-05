//! The bots' route brain — which strategic point a bot drives to and the drive command that gets
//! it there. Split from `bots.rs` (roster, unstuck, combat) for the reviewability budget.

use game_core::TeamId;
use game_core::math::wrap_angle;
use glam::Vec3;
use sim::{TankCommand, TankState};
use terrain::{BattlefieldMap, StrategicPoint, StrategicRole};

use crate::battle::BattleSeed;

pub(crate) fn seed_route_index(seed: BattleSeed, index: usize) -> usize {
    (seed_route_mix(index as u64 ^ seed.route_seed()) % 5) as usize
}

fn seed_route_mix(mut value: u64) -> u64 {
    value ^= value >> 33;
    value = value.wrapping_mul(0xff51_afd7_ed55_8ccd);
    value ^= value >> 33;
    value
}

pub(crate) fn bot_route_command(
    route_index: &mut usize,
    tank: &TankState,
    battlefield: &BattlefieldMap,
) -> TankCommand {
    let target = bot_route_target(route_index, tank.team, tank.position, battlefield);
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
    route_index: &mut usize,
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
    let point = points[*route_index % points.len()];
    let target = Vec3::from_array(point.position);
    if position.distance(target) < point.radius_m.max(25.0) {
        *route_index = (*route_index + 1) % points.len();
        Vec3::from_array(points[*route_index].position)
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

pub(crate) fn bot_yaw_to(from: Vec3, to: Vec3) -> f32 {
    let delta = to - from;
    delta.x.atan2(delta.z)
}
