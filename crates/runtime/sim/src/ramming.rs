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

/// Charge both hulls of every COLLISION the solver finished this tick (X7).
///
/// The collision that hurts is the collision the physics actually resolved. Ramming used to run
/// its own parallel truth — its own closing speed, its own slop radius around the hull footprints,
/// its own idea of who was touching — and two answers to "did these tanks collide" is one answer
/// too many. Then it read the solver's per-tick impulse, which was honest about WHO collided and
/// a hidden die about how hard: the speculative contact shuts whatever gap is left in the touch
/// tick, so the impulse that tick carried was the charge minus a sub-tick's travel — ~268 HP or
/// ~55 HP for the same charge by 8 cm of spawn distance. It now reads [`physics::ContactImpact`]:
/// the PEAK closing speed of the whole approach, reported once when the approach ends. The bill
/// is the charge, not the tick the plates happened to meet in.
///
/// Both hulls pay, weighted by mass: the impulse is shared (Newton's third law) but the lighter
/// hull takes the larger change of velocity, so each pays `2·m_other / (m_a + m_b)` of the shared
/// base — equal masses pay exactly the base each, as before — times the face it met the contact
/// with. A wall (an immovable solid) is the infinite-mass limit: the hull pays it alone, against
/// its own mass, and the wall takes nothing here (what a hull does TO cover is Z12's).
pub(crate) fn apply_ramming_damage(
    impacts: &[physics::ContactImpact],
    bodies: &[physics::ContactBody],
    tanks: &mut [TankState],
    damage_events: &mut Vec<DamageEvent>,
    event_stamp: &mut BattleEventStamp,
) {
    for impact in impacts {
        let closing = impact.peak_closing_mps;
        if closing < RAM_MIN_CLOSING_SPEED_MPS {
            continue;
        }
        let index_of = |id: u64| tanks.iter().position(|tank| tank.id.0 == id);
        match (index_of(impact.a), index_of(impact.b)) {
            (Some(left), Some(right)) => {
                // Teammates never grind each other down: a friendly shove is physics, not
                // damage (the contact itself still pushes both hulls).
                if tanks[left].team == tanks[right].team {
                    continue;
                }
                let (mass_a, mass_b) = (tanks[left].spec.mass_kg, tanks[right].spec.mass_kg);
                let reduced_mass = (mass_a * mass_b) / (mass_a + mass_b).max(1.0);
                let base = ram_damage_hp(reduced_mass, closing);
                if base == 0 {
                    continue;
                }
                let delta = horizontal(tanks[right].position - tanks[left].position);
                if delta.length() <= f32::EPSILON {
                    continue;
                }
                let direction = delta.normalize();
                let total = (mass_a + mass_b).max(1.0);
                let share_left = base as f32 * (2.0 * mass_b / total);
                let share_right = base as f32 * (2.0 * mass_a / total);
                let left_damage =
                    ram_face_damage(share_left.round() as u32, direction, tanks[left].yaw_rad);
                let right_damage =
                    ram_face_damage(share_right.round() as u32, -direction, tanks[right].yaw_rad);
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
            (Some(hull), None) | (None, Some(hull)) => {
                let wall_id = if index_of(impact.a).is_some() { impact.b } else { impact.a };
                let Some(wall) = bodies.iter().find(|body| body.id == wall_id && body.solid) else {
                    continue;
                };
                apply_wall_damage(hull, wall, closing, tanks, damage_events, event_stamp);
            }
            (None, None) => {}
        }
    }
}

/// The bill for running into a standing solid (X6/X7): the same curve a ram pays, with the
/// hull's own mass for the reduced mass (an immovable wall is the infinite-mass limit) and the
/// face it met the wall with. A charge into a tenement at 14 m/s costs what a charge into a
/// parked heavy costs; a crawl into a hedge costs nothing (under the threshold).
fn apply_wall_damage(
    hull: usize,
    wall: &physics::ContactBody,
    closing: f32,
    tanks: &mut [TankState],
    damage_events: &mut Vec<DamageEvent>,
    event_stamp: &mut BattleEventStamp,
) {
    let mass = tanks[hull].spec.mass_kg.max(1.0);
    let base = ram_damage_hp(mass, closing);
    if base == 0 {
        return;
    }
    let delta = horizontal(wall.position - tanks[hull].position);
    if delta.length() <= f32::EPSILON {
        return;
    }
    let damage = ram_face_damage(base, delta.normalize(), tanks[hull].yaw_rad);
    if damage == 0 {
        return;
    }
    let id = tanks[hull].id;
    let hit_position =
        tanks[hull].position + delta.normalize() * tanks[hull].spec.hull_plan().half_length_m;
    let destroyed = apply_single_damage(hull, damage, tanks);
    event_stamp.push_damage(damage_events, ram_event(id, id, hit_position, damage, destroyed));
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
