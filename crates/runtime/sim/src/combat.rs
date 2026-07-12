use game_core::math::{plate_normal, world_to_tank_local};
use game_core::{
    ArmorFacing, ArmorZone, DamageCause, DamageEvent, ModuleSlot, PenetrationResult, ShellId,
    TankId, resolve_penetration_at_distance_on_zone, resolve_penetration_through_track,
};
use glam::Vec3;

use crate::aim_dispersion::{apply_shot_bloom, dispersed_gun_direction};
use crate::breach_space::{BreachImpact, make_breach};
use crate::module_hit::{apply_track_damage_for_hit, impacted_module};
use crate::shell_trace::SHELL_MAX_AGE_SECONDS;
use crate::{ShellState, TankState};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CombatTickContext {
    pub dt_seconds: f32,
    /// The map's standing water: shells die in a splash at the surface (see `shell_trace`).
    pub water: Option<terrain::WaterBody>,
}

/// How early a fire click may land before the reload completes and still count: the input
/// buffer window. Inside it the shot is HELD and released the tick the breech closes; earlier
/// clicks are genuine misfires and refuse as before. Sized to human anticipation timing (the
/// player squeezes as the reticle reads ready), not to hide the reload.
pub const FIRE_BUFFER_S: f32 = 0.3;

/// Whether a refused fire click qualifies for the input buffer: everything about the gun is
/// ready EXCEPT the last sliver of reload.
pub(crate) fn fire_click_buffers(tank: &TankState) -> bool {
    let selected = (tank.selected_ammo as usize).min(game_core::MAX_AMMO_SLOTS - 1);
    tank.hit_points > 0
        && tank.modules.is_functional(ModuleSlot::Gun)
        && tank.modules.is_functional(ModuleSlot::AmmoRack)
        && tank.ammo_counts[selected] > 0
        && tank.reload_remaining_s > 0.0
        && tank.reload_remaining_s <= FIRE_BUFFER_S
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
    tank.reload_remaining_s = tank.full_reload_seconds();
    let shell_id = ShellId::from_shot(tank.id, tank.dispersion_shot_index);
    tank.dispersion_shot_index = tank.dispersion_shot_index.wrapping_add(1);
    apply_shot_bloom(tank);

    // The shell leaves the *visible* muzzle: the mount pivots about the trunnion and ring exactly
    // like the rendered gun submesh. Dispersion only perturbs the velocity direction — the barrel
    // itself does not jump around the aim point between shots.
    let muzzle = tank.muzzle_world_position();
    Some(ShellState {
        id: shell_id,
        owner: tank.id,
        position: muzzle,
        velocity_mps: direction * shell.muzzle_velocity_mps,
        shell,
        age_seconds: 0.0,
        traveled_m: 0.0,
        max_age_seconds: SHELL_MAX_AGE_SECONDS,
        ricocheted_once: false,
        last_penetrated_target: None,
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
    let before_destroyed = target.modules.destroyed_mask();
    let (module, damaged_modules_mask) = apply_internal_module_path(
        target,
        shell,
        penetration.penetrated,
        penetration.remaining_penetration_mm,
        penetration.module_damage_hp,
        zone,
        local_hit,
    );
    let destroyed_modules_mask = target.modules.destroyed_mask() & !before_destroyed;
    // How hard this shell bit the track band: a clean, near-normal AP round throws it outright; an
    // oblique or ricocheting hit only degrades it; an HE burst chips it (see `track_hit_damage`).
    let track_chunk = game_core::track_hit_damage(
        shell.shell.caliber_mm,
        impact_angle_degrees,
        shell.shell.shell_type,
        penetration.penetrated,
        penetration.ricocheted,
    );
    let track_hit = apply_track_damage_for_hit(
        target,
        module,
        zone,
        shell.shell.shell_type,
        penetration.penetrated,
        track_chunk,
    );
    if let Some(hit) = track_hit
        && hit.broke
    {
        let index = match hit.side {
            game_core::TrackSide::Left => 0,
            game_core::TrackSide::Right => 1,
        };
        target.track_break_t[index] = Some(
            ((local_hit.z + target.spec.hitbox.half_length_m)
                / (2.0 * target.spec.hitbox.half_length_m))
                .clamp(0.0, 1.0),
        );
    }

    if penetration.penetrated && target.spec.kind == game_core::VehicleKind::T54_1951 {
        let direction = shell.velocity_mps.normalize_or_zero();
        let breach = make_breach(
            target,
            BreachImpact {
                zone,
                hit_position,
                plate_normal,
                direction,
                caliber_mm: shell.shell.caliber_mm,
                effective_armor_mm: penetration.effective_armor_mm,
                residual_penetration_mm: penetration.remaining_penetration_mm,
            },
        );
        target.armor_breaches.add(breach);
    }
    if destroyed_modules_mask & ModuleSlot::Engine.destroyed_mask_bit() != 0
        && penetration.penetrated
    {
        target.engine_fire = true;
    }

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
        track_hit,
        damaged_modules_mask,
        destroyed_modules_mask,
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_internal_module_path(
    target: &mut TankState,
    shell: &ShellState,
    penetrated: bool,
    mut residual_mm: f32,
    base_damage_hp: u32,
    zone: ArmorZone,
    local_hit: Vec3,
) -> (Option<ModuleSlot>, u8) {
    if !penetrated || target.spec.damage_layout.is_empty() {
        let module = if target.spec.damage_layout.is_empty() {
            impacted_module(shell.shell.shell_type, penetrated, zone, local_hit, target.spec.hitbox)
        } else {
            target.spec.damage_layout.impacted_module(penetrated, local_hit)
        };
        if let Some(slot) = module {
            target.modules.damage(slot, base_damage_hp);
        }
        return (module, module.map_or(0, ModuleSlot::destroyed_mask_bit));
    }

    let local_direction =
        target.hull_pose().basis().transpose() * shell.velocity_mps.normalize_or_zero();
    let start = local_hit + local_direction * 0.01;
    let end = start + local_direction * 8.0;
    let hits = target.spec.damage_layout.intersections(true, start, end);
    let mut first = None;
    let mut mask = 0_u8;
    for hit in hits {
        let resistance_mm = 18.0 + hit.path_length_m * 55.0;
        if residual_mm < resistance_mm * 0.35 {
            break;
        }
        first.get_or_insert(hit.slot);
        mask |= hit.slot.destroyed_mask_bit();
        let energy_fraction = (residual_mm / (residual_mm + resistance_mm)).clamp(0.25, 1.0);
        let damage = ((base_damage_hp as f32 * energy_fraction).round() as u32).max(1);
        target.modules.damage(hit.slot, damage);
        residual_mm = (residual_mm - resistance_mm).max(0.0);
    }
    (first, mask)
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
    if !matches!(zone, ArmorZone::LeftTrack | ArmorZone::RightTrack | ArmorZone::Skirt) {
        return resolve_penetration_at_distance_on_zone(
            &shell.shell,
            &target.spec.hull,
            zone,
            impact_angle_degrees,
            distance_m,
        );
    }
    // A BROKEN track is not there any more: the thrown belt lies on the ground beside the
    // hull, so the shot meets the bare side plate with no spaced screen. The sim stops
    // charging armor for steel the eye can see is missing.
    let broken_side = match zone {
        ArmorZone::LeftTrack => target.tracks.hp(game_core::TrackSide::Left) == 0,
        ArmorZone::RightTrack => target.tracks.hp(game_core::TrackSide::Right) == 0,
        _ => false,
    };
    let side_sign = match zone {
        ArmorZone::LeftTrack => -1.0,
        ArmorZone::RightTrack => 1.0,
        // The skirt pair shares one zone; the struck plate is whichever side faces the shell's
        // approach, resolved in the hull frame.
        _ => {
            let local =
                target.hull_pose().basis().transpose() * shell.velocity_mps.normalize_or_zero();
            if local.x < 0.0 { 1.0 } else { -1.0 }
        }
    };
    let side_slope = target.spec.hull.facet(ArmorFacing::HullSide).slope_degrees;
    let side_normal =
        plate_normal(target.hull_pose(), 0.0, ArmorZone::HullSide, side_sign, side_slope);
    let direction = shell.velocity_mps.normalize_or_zero();
    let side_angle_degrees = (-direction).dot(side_normal).clamp(-1.0, 1.0).acos().to_degrees();
    if broken_side {
        return resolve_penetration_at_distance_on_zone(
            &shell.shell,
            &target.spec.hull,
            ArmorZone::HullSide,
            side_angle_degrees,
            distance_m,
        );
    }
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
            id: ShellId::default(),
            owner,
            position: from,
            velocity_mps: velocity,
            shell: spec.gun.ammo_options()[0],
            age_seconds: 0.0,
            traveled_m: 0.0,
            max_age_seconds: SHELL_MAX_AGE_SECONDS,
            ricocheted_once: false,
            last_penetrated_target: None,
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

    /// K4: a BROKEN track is thrown steel on the ground, not a spaced screen. A side hit on
    /// the broken track resolves against the bare hull side alone, so its effective armour is
    /// LESS than the same hit on a healthy track (which still adds the belt's LOS). The sim
    /// stops charging armour for a track the eye can see is gone.
    #[test]
    fn a_broken_track_no_longer_screens_the_hull_side() {
        let spec = TankSpec::t54_1951();
        let make = |broken: bool| {
            let mut tank = fresh_tank(TankId(2), TeamId(2), spec.clone(), Vec3::ZERO, 0.0);
            if broken {
                tank.tracks.break_side(game_core::TrackSide::Left);
            }
            tank
        };
        // A shell moving +x meets the LEFT side plate (outward normal -x) head-on.
        let shell =
            shell_toward(TankId(1), Vec3::new(-5.0, 0.0, 0.0), Vec3::new(900.0, 0.0, 0.0), &spec);

        let healthy =
            resolve_impact_penetration(&shell, &make(false), ArmorZone::LeftTrack, 0.0, 20.0);
        let broken =
            resolve_impact_penetration(&shell, &make(true), ArmorZone::LeftTrack, 0.0, 20.0);
        assert!(
            broken.effective_armor_mm < healthy.effective_armor_mm - 1.0,
            "a broken track must drop the effective armour (screen gone): broken {} vs healthy {}",
            broken.effective_armor_mm,
            healthy.effective_armor_mm
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

    #[test]
    fn penetrating_t54_side_creates_a_persistent_channel_and_module_mask() {
        let spec = TankSpec::t54_1951();
        let mut tanks = vec![fresh_tank(TankId(2), TeamId(2), spec.clone(), Vec3::ZERO, 0.0)];
        let shell =
            shell_toward(TankId(1), Vec3::new(-5.0, 1.0, -1.8), Vec3::new(900.0, 0.0, 0.0), &spec);
        let event = apply_shell_impact(
            &shell,
            &mut tanks,
            TankId(2),
            ArmorFacing::HullSide,
            ArmorZone::HullSide,
            0.0,
            Vec3::new(-1.05, 1.0, -1.8),
            Vec3::NEG_X,
            20.0,
        );
        assert!(event.penetrated);
        assert_ne!(event.damaged_modules_mask, 0);
        assert_eq!(tanks[0].armor_breaches.breaches().len(), 1);
        assert!(tanks[0].armor_breaches.breaches()[0].thickness_m > 0.0);
    }
}
