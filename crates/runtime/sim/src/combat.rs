use game_core::math::{plate_normal, world_to_tank_local};
use game_core::{
    ArmorFacing, ArmorZone, DamageCause, DamageEvent, ModuleSlot, PenetrationResult, ShellId,
    TankId, TrackSide, resolve_penetration_at_distance_on_zone,
    resolve_penetration_through_screens,
};
use glam::Vec3;

use crate::aim_dispersion::{apply_shot_bloom, dispersed_gun_direction};
use crate::breach_space::{BreachImpact, find_egress_contact, make_breach, make_egress_breach};
use crate::module_hit::{apply_track_damage_for_hit, impacted_module};
use crate::shell_continuation::ShellExit;
use crate::shell_trace::SHELL_MAX_AGE_SECONDS;
use crate::{ShellState, TankState};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CombatTickContext {
    pub dt_seconds: f32,
    pub tick: u64,
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
    tick: u64,
    breaches_out: &mut Vec<crate::event_stamp::ArmorBreachRecord>,
) -> (DamageEvent, Option<ShellExit>) {
    let target =
        tanks.iter_mut().find(|tank| tank.id == target_id).expect("hit tank still present");
    let target_was_alive = target.hit_points > 0;
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
    let (module, damaged_modules_mask, damaged_components_mask, residual_after_modules_mm) =
        apply_internal_module_path(
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
        local_hit.x,
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

    // What the shell had left when it came out the far side, if it came out at all.
    let mut exit = None;
    if penetration.penetrated {
        let direction = shell.velocity_mps.normalize_or_zero();
        let breach = make_breach(
            target,
            BreachImpact {
                zone,
                shell_id: shell.id,
                shell_type: shell.shell.shell_type,
                created_tick: tick,
                hit_position,
                plate_normal,
                direction,
                caliber_mm: shell.shell.caliber_mm,
                impact_angle_degrees,
                impact_speed_mps: shell.velocity_mps.length(),
                effective_armor_mm: penetration.effective_armor_mm,
                residual_penetration_mm: penetration.remaining_penetration_mm,
            },
        );
        // Replication replays exactly what the authoritative set was GIVEN, in this order — the
        // client's own `add` then reaches the same merge and capacity decisions.
        breaches_out.push(crate::event_stamp::ArmorBreachRecord {
            tank: target.id,
            breach: breach.clone(),
        });
        target.armor_breaches.add(breach);
        if let Some(contact) = find_egress_contact(target, zone, hit_position, direction) {
            let exit_angle =
                direction.dot(contact.normal).abs().clamp(0.0, 1.0).acos().to_degrees();
            let plate = target.spec.hull.plate(contact.zone);
            let required_mm =
                plate.nominal_thickness_mm / exit_angle.to_radians().cos().abs().max(0.18);
            // ONE test decides both halves of leaving: whether the far plate opens, and whether
            // the projectile is still a projectile on the other side of it. They used to be
            // decided from different budgets — the hole from what survived the internal path,
            // the flight from the entry plate alone — so a round that spent itself on the
            // engine still sailed out through steel the game had (correctly) left whole.
            if residual_after_modules_mm > required_mm {
                let speed_scale = (residual_after_modules_mm
                    / penetration.remaining_penetration_mm.max(1.0))
                .sqrt()
                .clamp(0.25, 1.0);
                let egress = make_egress_breach(
                    target,
                    BreachImpact {
                        zone: contact.zone,
                        shell_id: shell.id,
                        shell_type: shell.shell.shell_type,
                        created_tick: tick,
                        hit_position: contact.position,
                        plate_normal: contact.normal,
                        direction,
                        caliber_mm: shell.shell.caliber_mm,
                        impact_angle_degrees: exit_angle,
                        impact_speed_mps: shell.velocity_mps.length() * speed_scale,
                        effective_armor_mm: required_mm,
                        residual_penetration_mm: residual_after_modules_mm - required_mm,
                    },
                );
                breaches_out.push(crate::event_stamp::ArmorBreachRecord {
                    tank: target.id,
                    breach: egress.clone(),
                });
                target.armor_breaches.add(egress);
                exit = Some(ShellExit {
                    position: contact.position,
                    residual_penetration_mm: residual_after_modules_mm - required_mm,
                    speed_scale,
                });
            }
        }
    }
    if destroyed_modules_mask & ModuleSlot::Engine.destroyed_mask_bit() != 0
        && penetration.penetrated
    {
        target.engine_fire = true;
    }
    // Fuel burns as ITSELF (v27): a fuel-tank component holed by the internal path lights a
    // fuel fire, distinct from the engine's own — the tank can blaze with a healthy engine.
    if penetration.penetrated {
        let fuel_bits: u32 = target
            .spec
            .damage_layout
            .components()
            .iter()
            .filter(|component| component.kind == game_core::DamageComponentKind::FuelTank)
            .map(|component| component_bit(component.id))
            .fold(0, |bits, bit| bits | bit);
        if damaged_components_mask & fuel_bits != 0 {
            target.fuel_fire = true;
        }
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

    let event = DamageEvent {
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
        damaged_components_mask,
        event_id: Default::default(),
        occurred_tick: 0,
        shell_id: Some(shell.id),
        target_destroyed: target_was_alive && target.hit_points == 0,
    };
    (event, exit)
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
) -> (Option<ModuleSlot>, u8, u32, f32) {
    if !penetrated || target.spec.damage_layout.is_empty() {
        let module = if target.spec.damage_layout.is_empty() {
            impacted_module(shell.shell.shell_type, penetrated, zone, local_hit, target.spec.hitbox)
        } else {
            target.spec.damage_layout.impacted_module(penetrated, local_hit)
        };
        if let Some(slot) = module {
            target.modules.damage(slot, base_damage_hp);
        }
        return (module, module.map_or(0, ModuleSlot::destroyed_mask_bit), 0, residual_mm);
    }

    let local_direction =
        target.hull_pose().basis().transpose() * shell.velocity_mps.normalize_or_zero();
    let start = local_hit + local_direction * 0.01;
    let end = start + local_direction * 8.0;
    let turret_pivot =
        target.spec.mounts.turret_ring.translation - Vec3::Y * target.spec.hitbox.center_y_m;
    let hits = target.spec.damage_layout.intersections_with_turret_pose(
        true,
        start,
        end,
        target.turret_yaw_rad,
        turret_pivot,
    );
    let mut first = None;
    let mut mask = 0_u8;
    let mut component_mask = 0_u32;
    let mut spall_sources = Vec::new();
    for hit in hits {
        let resistance_mm = hit.material.resistance_mm(hit.path_length_m);
        if residual_mm < resistance_mm * 0.35 {
            break;
        }
        first.get_or_insert(hit.slot);
        mask |= hit.slot.destroyed_mask_bit();
        component_mask |= component_bit(hit.component_id);
        let energy_fraction = (residual_mm / (residual_mm + resistance_mm)).clamp(0.25, 1.0);
        let damage =
            ((base_damage_hp as f32 * energy_fraction * hit.vulnerability).round() as u32).max(1);
        target.modules.damage(hit.slot, damage);
        residual_mm = (residual_mm - resistance_mm).max(0.0);
        if residual_mm > 8.0 {
            spall_sources
                .push((hit.component_id, start + local_direction * (hit.distance_t * 8.0)));
        }
    }
    for (source_id, origin) in spall_sources {
        for (ray_index, spall_direction) in
            deterministic_spall_directions(local_direction, shell.id.0 ^ u64::from(source_id.0))
                .into_iter()
                .enumerate()
        {
            let spall_end = origin + spall_direction * 3.5;
            let Some(hit) = target
                .spec
                .damage_layout
                .intersections_with_turret_pose(
                    true,
                    origin + spall_direction * 0.015,
                    spall_end,
                    target.turret_yaw_rad,
                    turret_pivot,
                )
                .into_iter()
                .find(|hit| hit.component_id != source_id)
            else {
                continue;
            };
            component_mask |= component_bit(hit.component_id);
            mask |= hit.slot.destroyed_mask_bit();
            first.get_or_insert(hit.slot);
            let falloff = [0.22_f32, 0.16, 0.12][ray_index];
            let damage =
                ((base_damage_hp as f32 * falloff * hit.vulnerability).round() as u32).max(1);
            target.modules.damage(hit.slot, damage);
        }
    }
    (first, mask, component_mask, residual_mm)
}

fn component_bit(id: game_core::DamageComponentId) -> u32 {
    1_u32.checked_shl(u32::from(id.0.saturating_sub(1))).unwrap_or(0)
}

fn deterministic_spall_directions(direction: Vec3, seed: u64) -> [Vec3; 3] {
    let forward = direction.normalize_or_zero();
    let helper = if forward.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
    let right = forward.cross(helper).normalize_or_zero();
    let up = right.cross(forward).normalize_or_zero();
    std::array::from_fn(|index| {
        let mixed = game_core::math::splitmix64(seed ^ (index as u64).wrapping_mul(0x9E37_79B9));
        let azimuth = ((mixed >> 40) as f32 / (1_u64 << 24) as f32) * std::f32::consts::TAU;
        let angle = (5.0 + ((mixed >> 24) & 0xff) as f32 / 255.0 * 11.0).to_radians();
        (forward * angle.cos() + (right * azimuth.cos() + up * azimuth.sin()) * angle.sin())
            .normalize_or_zero()
    })
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
    // Which flank the shell met. The track zones say so outright; the skirt pair shares one
    // zone, so the struck plate is whichever side faces the shell's approach, resolved in the
    // hull frame.
    let struck_side = match zone {
        ArmorZone::LeftTrack => TrackSide::Left,
        ArmorZone::RightTrack => TrackSide::Right,
        _ => {
            let local =
                target.hull_pose().basis().transpose() * shell.velocity_mps.normalize_or_zero();
            if local.x < 0.0 { TrackSide::Right } else { TrackSide::Left }
        }
    };
    let side_sign = match struck_side {
        TrackSide::Left => -1.0,
        TrackSide::Right => 1.0,
    };
    let side_slope = target.spec.hull.facet(ArmorFacing::HullSide).slope_degrees;
    let side_normal =
        plate_normal(target.hull_pose(), 0.0, ArmorZone::HullSide, side_sign, side_slope);
    let direction = shell.velocity_mps.normalize_or_zero();
    let side_angle_degrees = (-direction).dot(side_normal).clamp(-1.0, 1.0).acos().to_degrees();

    // The spaced stack standing off this flank, OUTERMOST FIRST — the honest geometry of what
    // the shell actually crosses:
    //   * a skirt hangs outside the belt, so a skirt hit still has the belt behind it;
    //   * a BROKEN track is not there any more (the thrown belt lies on the ground beside the
    //     hull), so it stops screening — the sim never charges armour for steel the eye can
    //     see is missing;
    //   * an empty stack is a bare side plate.
    let belt_zone = match struck_side {
        TrackSide::Left => ArmorZone::LeftTrack,
        TrackSide::Right => ArmorZone::RightTrack,
    };
    let mut screens = [ArmorZone::Skirt; 2];
    let mut screen_count = 0;
    if zone == ArmorZone::Skirt {
        screens[screen_count] = ArmorZone::Skirt;
        screen_count += 1;
    }
    if target.tracks.hp(struck_side) > 0 {
        screens[screen_count] = belt_zone;
        screen_count += 1;
    }
    resolve_penetration_through_screens(
        &shell.shell,
        &target.spec.hull,
        &screens[..screen_count],
        impact_angle_degrees,
        side_angle_degrees,
        distance_m,
    )
}

#[cfg(test)]
mod tests {
    use game_core::{BreachFace, ShellType, TankSpec, TeamId, VehicleKind};

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

        let (event, _) = apply_shell_impact(
            &shell,
            &mut tanks,
            TankId(2),
            ArmorFacing::HullFront,
            ArmorZone::UpperGlacis,
            30.0,
            Vec3::new(0.0, 1.5, 18.5),
            plate,
            18.5,
            0,
            &mut Vec::new(),
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

    /// The integration half of the spaced-armour rule (the resolver's own half is
    /// `game_core/tests/spaced_armor.rs`): THIS is where the stack is built, so this is where a
    /// skirt that forgot the belt behind it would show up. Same tank, same shell, same flank —
    /// the only difference is which layer the trace resolved on, and the skirt is strictly extra
    /// steel in front of the same belt, so it can only cost the shell more.
    #[test]
    fn a_skirt_hit_costs_more_than_the_belt_it_hangs_over() {
        for kind in [VehicleKind::Centurion, VehicleKind::TigerII] {
            let spec = kind.spec();
            let tank = fresh_tank(TankId(2), TeamId(2), spec.clone(), Vec3::ZERO, 0.0);
            // Moving -x: the shell meets this hull's RIGHT flank head-on, whichever layer the
            // trace resolved on.
            let mut shell = shell_toward(
                TankId(1),
                Vec3::new(5.0, 0.9, 0.0),
                Vec3::new(-900.0, 0.0, 0.0),
                &spec,
            );
            for shell_type in [ShellType::ArmorPiercing, ShellType::Heat] {
                shell.shell = match shell_type {
                    ShellType::Heat => game_core::ShellSpec::heat(100.0, 900.0, 300.0, 320),
                    _ => game_core::ShellSpec::armor_piercing(100.0, 900.0, 200.0, 320),
                };
                let skirt = resolve_impact_penetration(&shell, &tank, ArmorZone::Skirt, 0.0, 100.0);
                let belt =
                    resolve_impact_penetration(&shell, &tank, ArmorZone::RightTrack, 0.0, 100.0);
                assert!(
                    skirt.effective_armor_mm > belt.effective_armor_mm,
                    "{kind:?} vs {shell_type:?}: a skirt hit must cost MORE than the bare belt \
                     it hangs over — skirt {:.1} mm vs belt {:.1} mm. The screen stack in \
                     `resolve_impact_penetration` must carry the belt behind the skirt.",
                    skirt.effective_armor_mm,
                    belt.effective_armor_mm
                );
            }
        }
    }

    /// ...and a thrown belt drops OUT of that stack, skirt or no skirt: the sheet is still
    /// bolted on, the running gear behind it is on the ground.
    #[test]
    fn a_thrown_belt_stops_screening_even_under_an_intact_skirt() {
        let spec = VehicleKind::Centurion.spec();
        let make = |broken: bool| {
            let mut tank = fresh_tank(TankId(2), TeamId(2), spec.clone(), Vec3::ZERO, 0.0);
            if broken {
                tank.tracks.break_side(game_core::TrackSide::Right);
            }
            tank
        };
        let shell =
            shell_toward(TankId(1), Vec3::new(5.0, 0.9, 0.0), Vec3::new(-900.0, 0.0, 0.0), &spec);
        let whole = resolve_impact_penetration(&shell, &make(false), ArmorZone::Skirt, 0.0, 100.0);
        let thrown = resolve_impact_penetration(&shell, &make(true), ArmorZone::Skirt, 0.0, 100.0);
        assert!(
            thrown.effective_armor_mm < whole.effective_armor_mm,
            "a thrown belt must stop screening: {:.1} mm vs {:.1} mm",
            thrown.effective_armor_mm,
            whole.effective_armor_mm
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
        let (event, _) = apply_shell_impact(
            &shell,
            &mut tanks,
            TankId(2),
            ArmorFacing::HullSide,
            ArmorZone::HullSide,
            0.0,
            Vec3::new(-1.05, 1.0, -1.8),
            Vec3::NEG_X,
            20.0,
            0,
            &mut Vec::new(),
        );
        assert!(event.penetrated);
        assert_ne!(event.damaged_modules_mask, 0);
        assert_eq!(tanks[0].armor_breaches.breaches().len(), 1);
        assert!(tanks[0].armor_breaches.breaches()[0].lobes()[0].thickness_m > 0.0);
    }

    #[test]
    fn egress_exists_only_when_the_internal_path_retains_enough_energy() {
        let spec = TankSpec::t54_1951();
        let hit = |mut shell: ShellState| {
            let mut tanks = vec![fresh_tank(TankId(2), TeamId(2), spec.clone(), Vec3::ZERO, 0.0)];
            shell.id = ShellId(0xE6_12_34);
            let (event, _) = apply_shell_impact(
                &shell,
                &mut tanks,
                TankId(2),
                ArmorFacing::HullSide,
                ArmorZone::HullSide,
                0.0,
                Vec3::new(-1.05, 1.0, -1.8),
                Vec3::NEG_X,
                20.0,
                44,
                &mut Vec::new(),
            );
            assert!(event.penetrated);
            tanks.remove(0).armor_breaches
        };

        let ap =
            shell_toward(TankId(1), Vec3::new(-5.0, 1.0, -1.8), Vec3::new(900.0, 0.0, 0.0), &spec);
        let stopped = hit(ap);
        assert_eq!(stopped.breaches().len(), 1, "stopped AP must not invent an exit");

        let mut heat = ap;
        heat.shell = spec.gun.special_shell.expect("D-10T HEAT round");
        let through = hit(heat);
        assert_eq!(through.breaches().len(), 2, "retained energy must open the opposite plate");
        assert!(through.breaches().iter().any(|breach| breach.face == BreachFace::Egress));
    }

    #[test]
    fn reusable_aperture_physics_is_not_gated_to_the_t54_asset() {
        let firing_spec = TankSpec::t54_1951();
        let target_spec = VehicleKind::TigerI.spec();
        let mut tanks = vec![fresh_tank(TankId(2), TeamId(2), target_spec, Vec3::ZERO, 0.0)];
        let mut shell = shell_toward(
            TankId(1),
            Vec3::new(-5.0, 1.0, 0.0),
            Vec3::new(900.0, 0.0, 0.0),
            &firing_spec,
        );
        shell.shell = firing_spec.gun.special_shell.expect("D-10T HEAT round");
        let (event, _) = apply_shell_impact(
            &shell,
            &mut tanks,
            TankId(2),
            ArmorFacing::HullSide,
            ArmorZone::HullSide,
            0.0,
            Vec3::new(-1.8, 1.0, 0.0),
            Vec3::NEG_X,
            20.0,
            7,
            &mut Vec::new(),
        );
        assert!(event.penetrated);
        assert!(!tanks[0].armor_breaches.breaches().is_empty());
    }
}

#[cfg(test)]
mod w0_7_tests {
    use super::component_bit;

    /// v27: the component mask is u32. Ids 17+ were silently maskless on u16 — the loose rounds
    /// and every future component now own a real bit.
    #[test]
    fn component_ids_beyond_sixteen_own_real_bits() {
        let mut seen = 0_u32;
        for id in 1..=32_u16 {
            let bit = component_bit(game_core::DamageComponentId(id));
            assert_ne!(bit, 0, "id {id} must own a bit on the widened mask");
            assert_eq!(seen & bit, 0, "id {id} must not collide");
            seen |= bit;
        }
    }
}
