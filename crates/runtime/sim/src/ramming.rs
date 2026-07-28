use game_core::math::horizontal_forward;
use game_core::{DamageCause, DamageEvent, ModuleSlot, TankId};
use glam::Vec3;

use crate::TankState;
use crate::event_stamp::BattleEventStamp;

/// Below this closing speed contact is PARKING, not ramming: a gentle nudge into a neighbour
/// (bots crowding the spawn included) deals nothing at all. ~16 km/h.
const RAM_MIN_CLOSING_SPEED_MPS: f32 = 4.5;

/// Share of the collision a hull pays when it meets it BOW ON: the thickest, most sloped
/// structure on the vehicle with the whole hull braced behind it. This is the charger's share.
const RAM_FACE_FRONT: f32 = 0.6;
/// ...on the FLANK: the same impulse through thin plate, straight into the running gear.
const RAM_FACE_SIDE: f32 = 1.4;
/// ...and on the REAR: engine deck and final drives, between the two.
const RAM_FACE_REAR: f32 = 1.0;

/// Charge both hulls of every contact the solver resolved this tick.
///
/// The collision that hurts is the collision the physics actually resolved. Ramming used to run
/// its own parallel truth — its own closing speed, its own slop radius around the hull footprints,
/// its own idea of who was touching — and two answers to "did these tanks collide" is one answer
/// too many. It now reads [`physics::ContactPair`]: the normal impulse the contact exchanged, with
/// mass and closing speed already folded together, which is exactly what a collision's severity is.
pub(crate) fn apply_ramming_damage(
    pairs: &[physics::ContactPair],
    tanks: &mut [TankState],
    damage_events: &mut Vec<DamageEvent>,
    event_stamp: &mut BattleEventStamp,
) {
    for pair in pairs {
        let (left, right) = (pair.a, pair.b);
        // Teammates never grind each other down: a friendly shove is physics, not damage (the
        // contact itself still pushes both hulls).
        if tanks[left].team == tanks[right].team {
            continue;
        }
        let reduced_mass = {
            let (a, b) = (tanks[left].spec.mass_kg, tanks[right].spec.mass_kg);
            (a * b) / (a + b).max(1.0)
        };
        // Back out the closing speed the impulse represents, so the damage curve keeps the
        // envelope it was tuned to (zero at the threshold, capped at 360 for a real charge).
        let closing = pair.normal_impulse_ns / reduced_mass.max(1.0);
        if closing < RAM_MIN_CLOSING_SPEED_MPS {
            continue;
        }
        let base = ram_damage_hp(reduced_mass, closing);
        if base == 0 {
            continue;
        }
        let delta = horizontal(tanks[right].position - tanks[left].position);
        if delta.length() <= f32::EPSILON {
            continue;
        }
        let direction = delta.normalize();
        let left_damage = ram_face_damage(base, direction, tanks[left].yaw_rad);
        let right_damage = ram_face_damage(base, -direction, tanks[right].yaw_rad);
        if left_damage == 0 && right_damage == 0 {
            continue;
        }
        apply_pair_damage(
            left,
            right,
            (left_damage, right_damage),
            tanks,
            damage_events,
            event_stamp,
        );
    }
}

fn ram_damage_hp(reduced_mass: f32, closing_speed: f32) -> u32 {
    let severity = closing_speed - RAM_MIN_CLOSING_SPEED_MPS;
    // Scales from ZERO at the threshold — the old 12 HP floor meant the gentlest qualifying
    // bump bruised hulls and (doubled) suspensions; a real charge still caps at 360.
    ((severity * severity * reduced_mass) / 4_500.0).round().clamp(0.0, 360.0) as u32
}

/// This hull's share of the shared impulse, from the face it met the contact with.
/// `contact_direction` points from the hull's centre toward the contact (horizontal, unit).
///
/// This is where a ram's asymmetry actually lives, and it is geometry — which is why it is
/// replay-stable and independent of who is stored where. A charging hull meets the contact bow
/// on and pays [`RAM_FACE_FRONT`]; a hull caught broadside pays [`RAM_FACE_SIDE`] for the same
/// impulse, because the same energy is going through a thin flank into the running gear.
fn ram_face_factor(contact_direction: Vec3, yaw_rad: f32) -> f32 {
    let alignment = contact_direction.dot(horizontal_forward(yaw_rad)).clamp(-1.0, 1.0);
    if alignment >= 0.0 {
        RAM_FACE_SIDE + (RAM_FACE_FRONT - RAM_FACE_SIDE) * alignment
    } else {
        RAM_FACE_SIDE + (RAM_FACE_REAR - RAM_FACE_SIDE) * -alignment
    }
}

fn ram_face_damage(base_hp: u32, contact_direction: Vec3, yaw_rad: f32) -> u32 {
    ((base_hp as f32 * ram_face_factor(contact_direction, yaw_rad)).round()).clamp(0.0, 360.0)
        as u32
}

fn apply_pair_damage(
    left_index: usize,
    right_index: usize,
    (left_damage, right_damage): (u32, u32),
    tanks: &mut [TankState],
    damage_events: &mut Vec<DamageEvent>,
    event_stamp: &mut BattleEventStamp,
) {
    let hit_position = (tanks[left_index].position + tanks[right_index].position) * 0.5;
    let left_id = tanks[left_index].id;
    let right_id = tanks[right_index].id;
    let right_destroyed = apply_single_damage(right_index, right_damage, tanks);
    let left_destroyed = apply_single_damage(left_index, left_damage, tanks);
    event_stamp.push_damage(
        damage_events,
        ram_event(left_id, right_id, hit_position, right_damage, right_destroyed),
    );
    event_stamp.push_damage(
        damage_events,
        ram_event(right_id, left_id, hit_position, left_damage, left_destroyed),
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
