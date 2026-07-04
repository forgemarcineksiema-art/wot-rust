use game_core::math::{plate_normal, world_to_tank_local};
use game_core::{
    ArmorFacing, ArmorZone, DamageCause, DamageEvent, ModuleSlot, PenetrationResult, TankId,
    resolve_penetration_at_distance_on_zone, resolve_penetration_through_track,
};
use glam::Vec3;

use crate::aim_dispersion::{apply_shot_bloom, dispersed_gun_direction};
use crate::module_hit::{apply_track_damage_for_hit, impacted_module};
use crate::shell_trace::SHELL_MAX_AGE_SECONDS;
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

    // The shell leaves the *visible* muzzle: the mount pivots about the trunnion and ring exactly
    // like the rendered gun submesh. Dispersion only perturbs the velocity direction — the barrel
    // itself does not jump around the aim point between shots.
    let mounts = tank.spec.mounts;
    // A non-stock gun fires from its own barrel tip: scale the muzzle by installed/stock length so
    // the shell spawn tracks the longer/shorter barrel (and the rendered gun, which scales to match).
    let stock_barrel = tank.spec.kind.stock_barrel_length_m();
    let barrel_scale =
        if stock_barrel > 0.0 { tank.spec.gun.barrel_length_m / stock_barrel } else { 1.0 };
    let muzzle = game_core::math::muzzle_world_position_scaled(
        &mounts,
        tank.position,
        tank.hull_pose(),
        tank.turret_yaw_rad,
        tank.gun_pitch_rad,
        barrel_scale,
    );
    Some(ShellState {
        owner: tank.id,
        position: muzzle,
        velocity_mps: direction * shell.muzzle_velocity_mps,
        shell,
        age_seconds: 0.0,
        traveled_m: 0.0,
        max_age_seconds: SHELL_MAX_AGE_SECONDS,
        ricocheted_once: false,
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_shell_impact(
    shell: &ShellState,
    tanks: &mut [TankState],
    target_id: TankId,
    facing: ArmorFacing,
    zone: ArmorZone,
    impact_angle_degrees: f32,
    hit_position: Vec3,
    distance_m: f32,
) -> DamageEvent {
    let target =
        tanks.iter_mut().find(|tank| tank.id == target_id).expect("hit tank still present");
    let penetration =
        resolve_impact_penetration(shell, target, zone, impact_angle_degrees, distance_m);
    if penetration.damage_hp > 0 {
        target.hit_points = target.hit_points.saturating_sub(penetration.damage_hp);
    }

    let local_hit = world_to_tank_local(
        hit_position,
        target.position,
        target.spec.hitbox.center_y_m,
        target.hull_pose(),
    );
    let module = if target.spec.damage_layout.is_empty() {
        impacted_module(
            shell.shell.shell_type,
            penetration.penetrated,
            zone,
            local_hit,
            target.spec.hitbox,
        )
    } else {
        target.spec.damage_layout.impacted_module(penetration.penetrated, local_hit)
    };
    if let Some(module) = module {
        target.modules.damage(module, penetration.module_damage_hp);
    }
    apply_track_damage_for_hit(
        target,
        module,
        zone,
        shell.shell.shell_type,
        penetration.penetrated,
    );

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

/// The armor test for one resolved hit. Ordinary zones test their single plate; the track zones
/// are a SPACED-ARMOR pair — the track band screens the hull side plate behind it, and each
/// layer is measured against its own true 3D normal (the side plate carries its slope and the
/// hull's live attitude, exactly like a direct side hit would).
fn resolve_impact_penetration(
    shell: &ShellState,
    target: &TankState,
    zone: ArmorZone,
    impact_angle_degrees: f32,
    distance_m: f32,
) -> PenetrationResult {
    if !matches!(zone, ArmorZone::LeftTrack | ArmorZone::RightTrack) {
        return resolve_penetration_at_distance_on_zone(
            &shell.shell,
            &target.spec.hull,
            zone,
            impact_angle_degrees,
            distance_m,
        );
    }
    let side_sign = if zone == ArmorZone::LeftTrack { -1.0 } else { 1.0 };
    let side_slope = target.spec.hull.facet(ArmorFacing::HullSide).slope_degrees;
    let side_normal =
        plate_normal(target.hull_pose(), 0.0, ArmorZone::HullSide, side_sign, side_slope);
    let direction = shell.velocity_mps.normalize_or_zero();
    let side_angle_degrees = (-direction).dot(side_normal).clamp(-1.0, 1.0).acos().to_degrees();
    resolve_penetration_through_track(
        &shell.shell,
        &target.spec.hull,
        zone,
        impact_angle_degrees,
        side_angle_degrees,
        distance_m,
    )
}
