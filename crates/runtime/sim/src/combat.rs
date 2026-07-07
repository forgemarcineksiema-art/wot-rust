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
    /// The map's standing water: shells die in a splash at the surface (see `shell_trace`).
    pub water: Option<terrain::WaterBody>,
}

pub(crate) fn try_fire_shell(tank: &mut TankState, tick: u64) -> Option<ShellState> {
    let selected = (tank.selected_ammo as usize).min(game_core::MAX_AMMO_SLOTS - 1);
    if tank.reload_remaining_s > 0.0
        || tank.hit_points == 0
        || !tank.modules.is_functional(ModuleSlot::Gun)
        || !tank.modules.is_functional(ModuleSlot::AmmoRack)
        // An empty slot refuses to fire: the player must switch to a slot with rounds left.
        || tank.ammo_counts[selected] == 0
    {
        return None;
    }

    let direction = dispersed_gun_direction(tank, tick);
    let shell = tank.selected_shell();
    tank.ammo_counts[selected] -= 1;
    tank.reload_remaining_s = tank.spec.gun.reload_seconds;
    tank.dispersion_shot_index = tank.dispersion_shot_index.wrapping_add(1);
    apply_shot_bloom(tank);

    // The shell leaves the *visible* muzzle: the mount pivots about the trunnion and ring exactly
    // like the rendered gun submesh. Dispersion only perturbs the velocity direction — the barrel
    // itself does not jump around the aim point between shots.
    let muzzle = tank.muzzle_world_position();
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
    plate_normal: Vec3,
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

    // The jack-in-the-box: an ammo-rack detonation that kills the tank in this same resolution
    // blows the turret off. Deterministic — a pure consequence of the module damage above, no
    // RNG. The trace then skips this wreck's turret and the client flies it on a ballistic arc.
    if module == Some(ModuleSlot::AmmoRack)
        && !target.modules.is_functional(ModuleSlot::AmmoRack)
        && target.hit_points == 0
    {
        target.turret_detached = true;
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
        // The presentation truth the client needs to seat the mark flush on the visual armor:
        // the plate's true world normal (from the armor-volume trace) and the shell's heading.
        plate_normal,
        shell_direction: shell.velocity_mps.normalize_or_zero(),
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

#[cfg(test)]
mod tests {
    use game_core::{ShellType, TankSpec, TeamId};

    use super::*;
    use crate::shell_trace::SHELL_MAX_AGE_SECONDS;
    use crate::tank_factory::fresh_tank;

    fn shell_toward(owner: TankId, from: Vec3, velocity: Vec3, spec: &TankSpec) -> ShellState {
        ShellState {
            owner,
            position: from,
            velocity_mps: velocity,
            shell: spec.gun.ammo_options()[0],
            age_seconds: 0.0,
            traveled_m: 0.0,
            max_age_seconds: SHELL_MAX_AGE_SECONDS,
            ricocheted_once: false,
        }
    }

    /// The event must carry through EXACTLY the struck-plate normal the trace resolved and the
    /// shell's own heading — this is the wire truth the client seats the impact mark on. The
    /// trace's normal correctness is locked separately (tests/armor_geometry.rs); here we lock
    /// that `apply_shell_impact` neither drops nor mangles it.
    #[test]
    fn the_event_carries_the_plate_normal_and_shell_heading_verbatim() {
        let spec = TankSpec::t54_1951();
        let mut tanks =
            vec![fresh_tank(TankId(2), TeamId(2), spec.clone(), Vec3::new(0.0, 0.0, 20.0), 0.0)];
        let velocity = Vec3::new(0.0, 0.0, 900.0);
        let shell = shell_toward(TankId(1), Vec3::new(0.0, 1.5, 0.0), velocity, &spec);
        // A representative reclined-glacis normal: outward, up-and-back toward the shooter.
        let plate = Vec3::new(0.0, 0.5, -0.866).normalize();

        let event = apply_shell_impact(
            &shell,
            &mut tanks,
            TankId(2),
            ArmorFacing::HullFront,
            ArmorZone::UpperGlacis,
            30.0,
            Vec3::new(0.0, 1.5, 18.5),
            plate,
            18.5,
        );

        assert!((event.plate_normal - plate).length() < 1.0e-3, "plate normal survives verbatim");
        assert!(
            (event.shell_direction - velocity.normalize()).length() < 1.0e-6,
            "shell direction is the normalized shell velocity"
        );
        assert!(
            event.plate_normal.dot(event.shell_direction) < 0.0,
            "an outward plate normal opposes the incoming shell"
        );
    }

    /// Non-shell damage has no struck plate: the default must be a zero vector, which the client
    /// reads as "no normal, fall back". Locks the guarantee the splash/ram/landing paths lean on
    /// via `..Default::default()`.
    #[test]
    fn a_defaulted_event_carries_no_normal() {
        let event = DamageEvent { cause: DamageCause::Splash, ..Default::default() };
        assert_eq!(event.plate_normal, Vec3::ZERO);
        assert_eq!(event.shell_direction, Vec3::ZERO);
        assert_eq!(ShellType::default(), event.shell_type);
    }
}
