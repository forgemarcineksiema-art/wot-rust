use ::terrain::{HeightMap, StaticCoverObject};
use game_core::math::integrate_shell_step;
use game_core::{ImpactSurface, ShellImpact};
use glam::Vec3;

use crate::breach_space::admits_existing_channel;
use crate::combat::{ArmorEntry, CombatTickContext, apply_shell_impact};
use crate::event_stamp::BattleEventOutput;
use crate::shell_continuation::{
    continue_through_armor, deflect_shell, kinetic_penetration_continues,
};
use crate::shell_splash::burst_he_splash;
use crate::shell_trace::{
    SegmentImpact, ShellTraceWorld, TraceTank, ground_contact, segment_impact,
};
use crate::{ShellState, TankState};

pub(crate) fn step_shells(
    shells: &mut Vec<ShellState>,
    tanks: &mut [TankState],
    events: &mut BattleEventOutput<'_>,
    context: CombatTickContext,
    heightmap: Option<&HeightMap>,
    cover: &[StaticCoverObject],
    rubble: &[terrain::RubbleMound],
) {
    let dt = context.dt_seconds;
    let mut index = 0;
    // Scratch target/blocker splits reused across every shell this tick — the split changes
    // per shell (ownership decides who is damageable), but the buffers need not be reallocated
    // ten times a tick for it.
    let mut targets: Vec<TraceTank> = Vec::new();
    let mut blockers: Vec<TraceTank> = Vec::new();
    while index < shells.len() {
        let previous = shells[index].position;
        let drag_per_s = shells[index].shell.drag_per_s();
        integrate_shell_step(&mut shells[index].velocity_mps, drag_per_s, dt);
        let velocity = shells[index].velocity_mps;
        shells[index].position += velocity * dt;
        shells[index].age_seconds += dt;
        let segment_distance = shells[index].position.distance(previous);

        trace_split_into(&shells[index], tanks, &mut targets, &mut blockers);
        let world = ShellTraceWorld {
            projectile_radius_m: shells[index].shell.collision_radius_m(),
            tanks: &targets,
            blockers: &blockers,
            heightmap,
            cover,
            rubble,
            water: context.water,
        };
        match segment_impact(previous, shells[index].position, velocity, &world) {
            Some(SegmentImpact::Tank {
                id,
                facing,
                zone,
                impact_angle_degrees,
                hit_position,
                plate_normal,
                thickness_scale,
            }) => {
                let distance_m = shells[index].traveled_m + hit_position.distance(previous);
                let radius_m = shells[index].shell.collision_radius_m();
                // A hole an earlier round opened wide enough to pass this whole projectile costs
                // it no steel — but it does NOT make the target's interior stop existing. The
                // round that goes through the hole is inside the fighting compartment, and it
                // resolves there like any other perforation: modules, damage, egress. It used to
                // be teleported past the hull with `last_penetrated_target` set, so it dealt
                // nothing, touched nothing and emitted no event at all — a shot that vanished
                // into the target with no feedback to the crew that fired it.
                let entry = match tanks.iter().find(|tank| tank.id == id) {
                    Some(tank) if admits_existing_channel(tank, zone, hit_position, radius_m) => {
                        ArmorEntry::OpenChannel
                    }
                    _ => ArmorEntry::Plate,
                };
                let mut carved = Vec::new();
                let Some((event, exit)) = apply_shell_impact(
                    &shells[index],
                    tanks,
                    id,
                    facing,
                    zone,
                    impact_angle_degrees,
                    hit_position,
                    plate_normal,
                    distance_m,
                    context.tick,
                    entry,
                    thickness_scale,
                    &mut carved,
                ) else {
                    // The struck tank is gone (unreachable on today's paths): drop the shell with
                    // no event rather than crashing. `carved` is still empty here.
                    shells.swap_remove(index);
                    continue;
                };
                for record in carved {
                    events.push_armor_breach(record.tank, record.breach);
                }
                // A tungsten core is BRITTLE: past the bounce angle it does not skid — it
                // SHATTERS on the outer face and dies there (Amunicja 3.0 A4, user decision
                // 2026-08-18). The doctrine cost of the light fast round: it never gets the
                // one-skip grace full-bore AP keeps. Outer face only — no interior effect. The
                // event carries the flag since v47; this routing and the client's mark read
                // the same word.
                let ricochet_continues =
                    event.ricocheted && !event.shattered && !shells[index].ricocheted_once;
                // The round flies on only through a hole the armour model actually opened.
                let exit = exit.filter(|exit| kinetic_penetration_continues(&shells[index], exit));
                let direct_target = event.target;
                let splashes = !event.penetrated;
                events.push_damage(event);
                // The hull's own impact (Inny Poziom S9): the shell's death on armour is a
                // `ShellImpact` too, so the client can draw an HE round's blast where the
                // charge went off — the AP path used to leave the client only the plate's clang.
                events.push_impact(ShellImpact {
                    owner: Some(shells[index].owner),
                    position: hit_position,
                    surface: ImpactSurface::Hull,
                    shell_type: shells[index].shell.shell_type,
                    direction: shells[index].velocity_mps.normalize_or_zero(),
                    caliber_mm: shells[index].shell.caliber_mm,
                    shell_id: shells[index].id,
                    ..Default::default()
                });
                if splashes {
                    burst_he_splash(
                        &shells[index],
                        hit_position,
                        tanks,
                        events,
                        Some(direct_target),
                        heightmap,
                        cover,
                        rubble,
                    );
                }
                if ricochet_continues {
                    deflect_shell(&mut shells[index], hit_position, plate_normal, distance_m);
                    index += 1;
                } else if let Some(exit) = exit {
                    continue_through_armor(&mut shells[index], &exit, direct_target, distance_m);
                    index += 1;
                } else {
                    shells.swap_remove(index);
                }
            }
            Some(SegmentImpact::Obstacle { position, surface }) => {
                events.push_impact(ShellImpact {
                    owner: Some(shells[index].owner),
                    position,
                    surface,
                    shell_type: shells[index].shell.shell_type,
                    direction: shells[index].velocity_mps.normalize_or_zero(),
                    caliber_mm: shells[index].shell.caliber_mm,
                    shell_id: shells[index].id,
                    ..Default::default()
                });
                // Z7: a kinetic round meeting the GROUND at a graze skips off it — once, like
                // off a plate: mirrored about the ground's plane, slower, blunted (the mark it
                // leaves is the furrow the impact above already recorded). A charge never
                // skips; a plunge never does; a round that skipped already does not again.
                if surface == ImpactSurface::Terrain
                    && let Some(heightmap) = heightmap
                    && let Some(normal) =
                        ground_ricochet_normal(&shells[index], position, heightmap)
                {
                    let distance_m = shells[index].traveled_m + position.distance(previous);
                    deflect_shell(&mut shells[index], position, normal, distance_m);
                    index += 1;
                    continue;
                }
                burst_he_splash(
                    &shells[index],
                    position,
                    tanks,
                    events,
                    None,
                    heightmap,
                    cover,
                    rubble,
                );
                shells.swap_remove(index);
            }
            None => {
                if step_unhit_shell(
                    shells,
                    tanks,
                    events,
                    index,
                    segment_distance,
                    heightmap,
                    cover,
                    rubble,
                ) {
                    index += 1;
                }
            }
        }
    }
}

/// Z7: a kinetic round skips off the ground when its flight meets the surface within this
/// angle of the plane (the incidence measured to the ground, not its normal).
pub const GROUND_RICOCHET_MAX_DEG: f32 = 12.0;

/// The ground's normal at a terrain impact IF the round skips (Z7): kinetic, not yet skipped,
/// still carrying speed, and meeting the plane within `GROUND_RICOCHET_MAX_DEG` of it.
fn ground_ricochet_normal(
    shell: &ShellState,
    position: Vec3,
    heightmap: &HeightMap,
) -> Option<Vec3> {
    if shell.shell.shell_type == game_core::ShellType::HighExplosive || shell.ricocheted_once {
        return None;
    }
    let velocity = shell.velocity_mps;
    if velocity.length() < 80.0 {
        return None;
    }
    let normal = Vec3::from_array(::terrain::ground_normal_at(heightmap, position.x, position.z));
    let incidence = velocity.normalize().dot(normal).abs().asin().to_degrees();
    (incidence <= GROUND_RICOCHET_MAX_DEG).then_some(normal)
}

#[expect(clippy::too_many_arguments)]
fn step_unhit_shell(
    shells: &mut Vec<ShellState>,
    tanks: &mut [TankState],
    events: &mut BattleEventOutput<'_>,
    index: usize,
    segment_distance: f32,
    heightmap: Option<&HeightMap>,
    cover: &[StaticCoverObject],
    rubble: &[terrain::RubbleMound],
) -> bool {
    if ground_contact(shells[index].position, heightmap) {
        let position = shells[index].position;
        events.push_impact(ShellImpact {
            owner: Some(shells[index].owner),
            position,
            surface: ImpactSurface::Terrain,
            shell_type: shells[index].shell.shell_type,
            direction: shells[index].velocity_mps.normalize_or_zero(),
            caliber_mm: shells[index].shell.caliber_mm,
            shell_id: shells[index].id,
            ..Default::default()
        });
        burst_he_splash(&shells[index], position, tanks, events, None, heightmap, cover, rubble);
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

fn trace_split_into(
    shell: &ShellState,
    tanks: &[TankState],
    targets: &mut Vec<TraceTank>,
    blockers: &mut Vec<TraceTank>,
) {
    targets.clear();
    blockers.clear();
    let owner_team = tanks.iter().find(|tank| tank.id == shell.owner).map(|tank| tank.team);
    for tank in tanks {
        if tank.id == shell.owner || Some(tank.id) == shell.last_penetrated_target {
            continue;
        }
        let mut trace = TraceTank::from_spec(
            tank.id,
            tank.position,
            tank.hull_pose(),
            tank.turret_yaw_rad,
            &tank.spec,
        );
        // A decapitated wreck blocks with its hull only — its turret is gone (see combat's
        // ammo-rack detonation). Live tanks never carry the flag.
        trace.turret_detached = tank.turret_detached;
        if tank.hit_points > 0 && owner_team != Some(tank.team) {
            targets.push(trace);
        } else {
            blockers.push(trace);
        }
    }
}
