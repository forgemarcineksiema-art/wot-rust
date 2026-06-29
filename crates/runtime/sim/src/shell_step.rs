use ::terrain::{HeightMap, StaticCoverObject};
use game_core::math::GRAVITY_MPS2;
use game_core::{DamageEvent, ImpactSurface, ShellImpact};

use crate::combat::{CombatTickContext, apply_shell_impact};
use crate::shell_trace::{
    SegmentImpact, ShellTraceWorld, TraceTank, ground_contact, segment_impact,
};
use crate::{ShellState, TankState};

pub(crate) fn step_shells(
    shells: &mut Vec<ShellState>,
    tanks: &mut [TankState],
    damage_events: &mut Vec<DamageEvent>,
    shell_impacts: &mut Vec<ShellImpact>,
    context: CombatTickContext,
    heightmap: Option<&HeightMap>,
    cover: &[StaticCoverObject],
) {
    let dt = context.dt_seconds;
    let mut index = 0;
    while index < shells.len() {
        let previous = shells[index].position;
        shells[index].velocity_mps.y -= GRAVITY_MPS2 * dt;
        let velocity = shells[index].velocity_mps;
        shells[index].position += velocity * dt;
        shells[index].age_seconds += dt;
        let segment_distance = shells[index].position.distance(previous);

        let (targets, blockers) = trace_split(&shells[index], tanks);
        let world = ShellTraceWorld { tanks: &targets, blockers: &blockers, heightmap, cover };
        match segment_impact(previous, shells[index].position, velocity, &world) {
            Some(SegmentImpact::Tank { id, facing, zone, impact_angle_degrees, hit_position }) => {
                let distance_m = shells[index].traveled_m + hit_position.distance(previous);
                let event = apply_shell_impact(
                    &shells[index],
                    tanks,
                    id,
                    facing,
                    zone,
                    impact_angle_degrees,
                    hit_position,
                    distance_m,
                );
                damage_events.push(event);
                shells.swap_remove(index);
            }
            Some(SegmentImpact::Obstacle { position, surface }) => {
                shell_impacts.push(ShellImpact { owner: shells[index].owner, position, surface });
                shells.swap_remove(index);
            }
            None => {
                if step_unhit_shell(shells, shell_impacts, index, segment_distance, heightmap) {
                    index += 1;
                }
            }
        }
    }
}

fn step_unhit_shell(
    shells: &mut Vec<ShellState>,
    shell_impacts: &mut Vec<ShellImpact>,
    index: usize,
    segment_distance: f32,
    heightmap: Option<&HeightMap>,
) -> bool {
    if ground_contact(shells[index].position, heightmap) {
        shell_impacts.push(ShellImpact {
            owner: shells[index].owner,
            position: shells[index].position,
            surface: ImpactSurface::Terrain,
        });
        shells.swap_remove(index);
        false
    } else if shells[index].age_seconds >= shells[index].max_age_seconds {
        shells.swap_remove(index);
        false
    } else {
        shells[index].traveled_m += segment_distance;
        true
    }
}

fn trace_split(shell: &ShellState, tanks: &[TankState]) -> (Vec<TraceTank>, Vec<TraceTank>) {
    let owner_team = tanks.iter().find(|tank| tank.id == shell.owner).map(|tank| tank.team);
    let mut targets = Vec::new();
    let mut blockers = Vec::new();
    for tank in tanks {
        if tank.id == shell.owner {
            continue;
        }
        let trace = TraceTank::from_spec(
            tank.id,
            tank.position,
            tank.yaw_rad,
            tank.turret_yaw_rad,
            &tank.spec,
        );
        if tank.hit_points > 0 && owner_team != Some(tank.team) {
            targets.push(trace);
        } else {
            blockers.push(trace);
        }
    }
    (targets, blockers)
}
