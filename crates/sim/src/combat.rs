use ::terrain::{HeightMap, StaticCoverObject};
use game_core::math::{GRAVITY_MPS2, world_to_tank_local};
use game_core::{
    ArmorFacing, DamageCause, DamageEvent, ImpactSurface, ModuleSlot, MountFrames, ShellImpact,
    TankId, resolve_penetration_at_distance_on_zone,
};
use glam::Vec3;

use crate::aim_dispersion::{apply_shot_bloom, dispersed_gun_direction};
use crate::module_hit::impacted_module;
use crate::shell_trace::{
    SHELL_MAX_AGE_SECONDS, SegmentImpact, ShellTraceWorld, TraceTank, ground_contact,
    segment_impact,
};
use crate::{ShellState, TankState};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CombatTickContext {
    pub dt_seconds: f32,
}

pub(crate) fn try_fire_shell(tank: &mut TankState, tick: u64) -> Option<ShellState> {
    if tank.reload_remaining_s > 0.0
        || tank.hit_points == 0
        || !tank.modules.is_functional(ModuleSlot::Gun)
        || !tank.modules.is_functional(ModuleSlot::AmmoRack)
    {
        return None;
    }

    let direction = dispersed_gun_direction(tank, tick);
    let shell = tank.spec.gun.shell;
    tank.reload_remaining_s = tank.spec.gun.reload_seconds;
    tank.dispersion_shot_index = tank.dispersion_shot_index.wrapping_add(1);
    apply_shot_bloom(tank);

    let muzzle = MountFrames::for_vehicle(tank.spec.kind).muzzle.translation;
    Some(ShellState {
        owner: tank.id,
        position: tank.position + Vec3::Y * muzzle.y + direction * muzzle.z,
        velocity_mps: direction * shell.muzzle_velocity_mps,
        shell,
        age_seconds: 0.0,
        traveled_m: 0.0,
        max_age_seconds: SHELL_MAX_AGE_SECONDS,
    })
}

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
                if ground_contact(shells[index].position, heightmap) {
                    shell_impacts.push(ShellImpact {
                        owner: shells[index].owner,
                        position: shells[index].position,
                        surface: ImpactSurface::Terrain,
                    });
                    shells.swap_remove(index);
                } else if shells[index].age_seconds >= shells[index].max_age_seconds {
                    // Expired into open sky: there is no surface to mark.
                    shells.swap_remove(index);
                } else {
                    shells[index].traveled_m += segment_distance;
                    index += 1;
                }
            }
        }
    }
}

/// Split the battle into this shell's damageable targets and absorbing blockers, as neutral
/// [`TraceTank`]s. Live enemies take damage; live teammates and every wreck absorb the shell
/// without damage. The owner belongs to neither slice.
fn trace_split(shell: &ShellState, tanks: &[TankState]) -> (Vec<TraceTank>, Vec<TraceTank>) {
    let owner_team = tanks.iter().find(|tank| tank.id == shell.owner).map(|tank| tank.team);
    let mut targets = Vec::new();
    let mut blockers = Vec::new();
    for tank in tanks {
        if tank.id == shell.owner {
            continue;
        }
        let trace = TraceTank {
            id: tank.id,
            position: tank.position,
            yaw_rad: tank.yaw_rad,
            turret_yaw_rad: tank.turret_yaw_rad,
            hitbox: tank.spec.hitbox,
        };
        if tank.hit_points > 0 && owner_team != Some(tank.team) {
            targets.push(trace);
        } else {
            blockers.push(trace);
        }
    }
    (targets, blockers)
}

#[allow(clippy::too_many_arguments)]
fn apply_shell_impact(
    shell: &ShellState,
    tanks: &mut [TankState],
    target_id: TankId,
    facing: ArmorFacing,
    zone: game_core::ArmorZone,
    impact_angle_degrees: f32,
    hit_position: Vec3,
    distance_m: f32,
) -> DamageEvent {
    let target =
        tanks.iter_mut().find(|tank| tank.id == target_id).expect("hit tank still present");
    let penetration = resolve_penetration_at_distance_on_zone(
        &shell.shell,
        &target.spec.hull,
        zone,
        impact_angle_degrees,
        distance_m,
    );
    if penetration.damage_hp > 0 {
        target.hit_points = target.hit_points.saturating_sub(penetration.damage_hp);
    }

    let local_hit = world_to_tank_local(
        hit_position,
        target.position,
        target.spec.hitbox.center_y_m,
        target.yaw_rad,
    );
    let module = impacted_module(
        shell.shell.shell_type,
        penetration.penetrated,
        zone,
        local_hit,
        target.spec.hitbox,
    );
    if let Some(module) = module {
        target.modules.damage(module, penetration.module_damage_hp);
    }

    DamageEvent {
        source: shell.owner,
        target: target.id,
        hit_position,
        damage_hp: penetration.damage_hp,
        penetrated: penetration.penetrated,
        cause: DamageCause::Shell,
        module,
        ricocheted: penetration.ricocheted,
        shell_type: shell.shell.shell_type,
        impact_angle_degrees,
        effective_armor_mm: penetration.effective_armor_mm,
        shell_penetration_mm: penetration.effective_armor_mm + penetration.remaining_penetration_mm,
        nominal_armor_mm: target.spec.hull.nominal_thickness_mm(facing),
        armor_facing: facing,
        armor_zone: zone,
    }
}
