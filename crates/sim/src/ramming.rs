use game_core::{DamageCause, DamageEvent, ModuleSlot, TankId};
use glam::Vec3;

use crate::TankState;

const RAM_MIN_CLOSING_SPEED_MPS: f32 = 2.5;
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
) {
    for left in 0..tanks.len() {
        for right in left + 1..tanks.len() {
            let Some((left_before, right_before)) =
                snapshot_pair(before, tanks[left].id, tanks[right].id)
            else {
                continue;
            };
            let Some(contact) = contact_delta(&tanks[left], &tanks[right]) else {
                continue;
            };
            let closing = closing_speed(left_before, right_before, contact);
            if closing < RAM_MIN_CLOSING_SPEED_MPS {
                continue;
            }
            let damage = ram_damage_hp(left_before.mass_kg, right_before.mass_kg, closing);
            apply_pair_damage(left, right, damage, tanks, damage_events);
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

fn contact_delta(left: &TankState, right: &TankState) -> Option<Vec3> {
    let delta = horizontal(right.position - left.position);
    let distance = delta.length();
    if distance <= f32::EPSILON {
        return None;
    }
    let contact_distance =
        left.spec.hitbox.half_length_m + right.spec.hitbox.half_length_m + RAM_CONTACT_SLOP_M;
    (distance <= contact_distance).then_some(delta)
}

fn closing_speed(left: RammingSnapshot, right: RammingSnapshot, current_delta: Vec3) -> f32 {
    let direction = current_delta.normalize();
    let relative_velocity = left.velocity_mps - right.velocity_mps;
    relative_velocity.dot(direction).max(0.0)
}

fn ram_damage_hp(left_mass: f32, right_mass: f32, closing_speed: f32) -> u32 {
    let reduced_mass = (left_mass * right_mass) / (left_mass + right_mass).max(1.0);
    let severity = closing_speed - RAM_MIN_CLOSING_SPEED_MPS;
    ((severity * severity * reduced_mass) / 4_500.0).round().clamp(12.0, 360.0) as u32
}

fn apply_pair_damage(
    left_index: usize,
    right_index: usize,
    damage: u32,
    tanks: &mut [TankState],
    damage_events: &mut Vec<DamageEvent>,
) {
    let hit_position = (tanks[left_index].position + tanks[right_index].position) * 0.5;
    let left_id = tanks[left_index].id;
    let right_id = tanks[right_index].id;
    apply_single_damage(right_index, damage, tanks);
    apply_single_damage(left_index, damage / 2, tanks);
    damage_events.push(ram_event(left_id, right_id, hit_position, damage));
    damage_events.push(ram_event(right_id, left_id, hit_position, damage / 2));
}

fn apply_single_damage(index: usize, damage: u32, tanks: &mut [TankState]) {
    let tank = &mut tanks[index];
    tank.hit_points = tank.hit_points.saturating_sub(damage);
    tank.modules.damage(ModuleSlot::Suspension, damage.saturating_mul(2));
}

fn ram_event(source: TankId, target: TankId, hit_position: Vec3, damage_hp: u32) -> DamageEvent {
    DamageEvent {
        source,
        target,
        hit_position,
        damage_hp,
        penetrated: false,
        cause: DamageCause::Ram,
        module: Some(ModuleSlot::Suspension),
        ..Default::default()
    }
}

fn horizontal(vector: Vec3) -> Vec3 {
    Vec3::new(vector.x, 0.0, vector.z)
}
