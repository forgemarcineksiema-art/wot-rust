use game_core::{DamageCause, DamageEvent, ModuleSlot, TankId};
use glam::Vec3;
use physics::{TankObstacle, tank_footprints_touch};

use crate::TankState;
use crate::event_stamp::BattleEventStamp;

/// Below this closing speed contact is PARKING, not ramming: a gentle nudge into a neighbour
/// (bots crowding the spawn included) deals nothing at all. ~16 km/h.
const RAM_MIN_CLOSING_SPEED_MPS: f32 = 4.5;
const RAM_CONTACT_SLOP_M: f32 = 0.12;

#[derive(Debug, Clone, Copy)]
pub(crate) struct RammingSnapshot {
    id: TankId,
    velocity_mps: Vec3,
    mass_kg: f32,
}

pub(crate) fn capture_ramming_snapshots(tanks: &[TankState]) -> Vec<RammingSnapshot> {
    tanks
        .iter()
        .map(|tank| RammingSnapshot {
            id: tank.id,
            velocity_mps: tank.velocity_mps,
            mass_kg: tank.spec.mass_kg,
        })
        .collect()
}

pub(crate) fn apply_ramming_damage(
    before: &[RammingSnapshot],
    tanks: &mut [TankState],
    damage_events: &mut Vec<DamageEvent>,
    event_stamp: &mut BattleEventStamp,
    dt_seconds: f32,
) {
    for left in 0..tanks.len() {
        for right in left + 1..tanks.len() {
            let Some((left_before, right_before)) =
                snapshot_pair(before, tanks[left].id, tanks[right].id)
            else {
                continue;
            };
            let delta = horizontal(tanks[right].position - tanks[left].position);
            if delta.length() <= f32::EPSILON {
                continue;
            }
            // Teammates never grind each other down: a friendly shove is physics, not damage
            // (the collision itself still stops the hulls).
            if tanks[left].team == tanks[right].team {
                continue;
            }
            let closing = closing_speed(left_before, right_before, delta);
            if closing < RAM_MIN_CLOSING_SPEED_MPS {
                continue;
            }
            if !hulls_in_ram_contact(&tanks[left], &tanks[right], closing, dt_seconds) {
                continue;
            }
            let damage = ram_damage_hp(left_before.mass_kg, right_before.mass_kg, closing);
            if damage == 0 {
                continue;
            }
            apply_pair_damage(left, right, damage, tanks, damage_events, event_stamp);
        }
    }
}

fn snapshot_pair(
    snapshots: &[RammingSnapshot],
    left: TankId,
    right: TankId,
) -> Option<(RammingSnapshot, RammingSnapshot)> {
    Some((
        *snapshots.iter().find(|snapshot| snapshot.id == left)?,
        *snapshots.iter().find(|snapshot| snapshot.id == right)?,
    ))
}

/// True when the two hull footprints (the same OBBs movement collides) touch within the ram
/// slop. The slop grows by one tick of closing distance so a fast pair resolved just short of
/// contact by the movement step still registers the ram instead of tunnelling under the
/// predicate. Center-distance circles are forbidden here: they fired on clean side-by-side
/// passes (see `tests/ramming_contact.rs`, the negative contact tests).
fn hulls_in_ram_contact(
    left: &TankState,
    right: &TankState,
    closing_speed_mps: f32,
    dt_seconds: f32,
) -> bool {
    let slop_m = RAM_CONTACT_SLOP_M + closing_speed_mps.max(0.0) * dt_seconds.max(0.0);
    let left_hull = TankObstacle::from_hitbox(left.position, left.yaw_rad, left.spec.hitbox);
    let right_hull = TankObstacle::from_hitbox(right.position, right.yaw_rad, right.spec.hitbox);
    tank_footprints_touch(&left_hull, &right_hull, slop_m)
}

fn closing_speed(left: RammingSnapshot, right: RammingSnapshot, current_delta: Vec3) -> f32 {
    let direction = current_delta.normalize();
    let relative_velocity = left.velocity_mps - right.velocity_mps;
    relative_velocity.dot(direction).max(0.0)
}

fn ram_damage_hp(left_mass: f32, right_mass: f32, closing_speed: f32) -> u32 {
    let reduced_mass = (left_mass * right_mass) / (left_mass + right_mass).max(1.0);
    let severity = closing_speed - RAM_MIN_CLOSING_SPEED_MPS;
    // Scales from ZERO at the threshold — the old 12 HP floor meant the gentlest qualifying
    // bump bruised hulls and (doubled) suspensions; a real charge still caps at 360.
    ((severity * severity * reduced_mass) / 4_500.0).round().clamp(0.0, 360.0) as u32
}

fn apply_pair_damage(
    left_index: usize,
    right_index: usize,
    damage: u32,
    tanks: &mut [TankState],
    damage_events: &mut Vec<DamageEvent>,
    event_stamp: &mut BattleEventStamp,
) {
    let hit_position = (tanks[left_index].position + tanks[right_index].position) * 0.5;
    let left_id = tanks[left_index].id;
    let right_id = tanks[right_index].id;
    let right_destroyed = apply_single_damage(right_index, damage, tanks);
    let left_destroyed = apply_single_damage(left_index, damage / 2, tanks);
    event_stamp.push_damage(
        damage_events,
        ram_event(left_id, right_id, hit_position, damage, right_destroyed),
    );
    event_stamp.push_damage(
        damage_events,
        ram_event(right_id, left_id, hit_position, damage / 2, left_destroyed),
    );
}

fn apply_single_damage(index: usize, damage: u32, tanks: &mut [TankState]) -> bool {
    let tank = &mut tanks[index];
    let target_was_alive = tank.hit_points > 0;
    tank.hit_points = tank.hit_points.saturating_sub(damage);
    // The suspension takes what the hull takes — the old x2 turned every contact into a
    // mobility kill long before the hull cared.
    tank.modules.damage(ModuleSlot::Suspension, damage);
    target_was_alive && tank.hit_points == 0
}

fn ram_event(
    source: TankId,
    target: TankId,
    hit_position: Vec3,
    damage_hp: u32,
    target_destroyed: bool,
) -> DamageEvent {
    DamageEvent {
        source,
        target,
        hit_position,
        damage_hp,
        penetrated: false,
        cause: DamageCause::Ram,
        module: Some(ModuleSlot::Suspension),
        target_destroyed,
        ..Default::default()
    }
}

fn horizontal(vector: Vec3) -> Vec3 {
    Vec3::new(vector.x, 0.0, vector.z)
}
