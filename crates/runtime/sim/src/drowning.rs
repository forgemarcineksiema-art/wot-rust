//! Drowning: what deep water does to a hull that wading physics could not stop. Past
//! [`DROWN_DEPTH_M`] the engine air intake is under the surface — the engine floods and dies
//! after a short grace, then the hull drains to zero in damage pulses. Leaving deep water
//! before the flood deadline resets the clock; a flooded engine stays dead (modules do not
//! self-repair). Server-authoritative only — the client predicts wading drag but never damage.

use game_core::{DamageCause, DamageEvent, ModuleSlot};
use terrain::{HeightMap, WaterView};

use crate::event_stamp::BattleEventStamp;
use crate::tank_state::TankState;

/// Water over the hull deep enough to flood the engine intake and drown the vehicle.
pub const DROWN_DEPTH_M: f32 = 1.5;
/// Grace before the submerged engine floods (seconds under `DROWN_DEPTH_M`).
pub const ENGINE_FLOOD_S: f32 = 2.0;
/// After the engine floods, the hull takes a drowning pulse this often...
pub const DROWN_PULSE_INTERVAL_S: f32 = 0.5;
/// ...losing this fraction of max hit points per pulse: 8 pulses = dead 4 s after the flood,
/// ~6 s after entering deep water.
const DROWN_PULSE_HP_FRACTION: f32 = 1.0 / 8.0;

/// Advance every living tank's submersion clock one tick and charge the drowning bills.
/// No-op on dry maps (`water: None`) — replay-locked.
pub(crate) fn step_drowning(
    tanks: &mut [TankState],
    heightmap: Option<&HeightMap>,
    water: WaterView<'_>,
    dt: f32,
    damage_events: &mut Vec<DamageEvent>,
    event_stamp: &mut BattleEventStamp,
) {
    let Some(heightmap) = heightmap else {
        return;
    };
    if water.table.is_none() && water.sheets.is_empty() {
        return;
    }
    for tank in tanks.iter_mut() {
        if tank.hit_points == 0 {
            continue;
        }
        let depth = heightmap
            .sample_height(tank.position.x, tank.position.z)
            .map_or(0.0, |ground| water.depth_at(ground, tank.position.x, tank.position.z));
        if depth < DROWN_DEPTH_M {
            tank.submerged_s = 0.0;
            continue;
        }
        let before = tank.submerged_s;
        tank.submerged_s += dt;

        // The flood deadline: crossing it kills the engine once.
        if before < ENGINE_FLOOD_S
            && tank.submerged_s >= ENGINE_FLOOD_S
            && tank.modules.is_functional(ModuleSlot::Engine)
        {
            tank.modules.damage(ModuleSlot::Engine, u32::MAX);
            event_stamp.push_damage(
                damage_events,
                DamageEvent {
                    source: tank.id,
                    target: tank.id,
                    hit_position: tank.position,
                    damage_hp: 0,
                    penetrated: false,
                    cause: DamageCause::Drowning,
                    module: Some(ModuleSlot::Engine),
                    ..Default::default()
                },
            );
        }

        // Past the flood, the hull drains in pulses: one per interval boundary crossed.
        let pulses_before = drown_pulses(before);
        let pulses_now = drown_pulses(tank.submerged_s);
        for _ in pulses_before..pulses_now {
            let pulse = ((tank.spec.hit_points as f32 * DROWN_PULSE_HP_FRACTION).round() as u32)
                .max(1)
                .min(tank.hit_points);
            let target_was_alive = tank.hit_points > 0;
            tank.hit_points -= pulse;
            event_stamp.push_damage(
                damage_events,
                DamageEvent {
                    source: tank.id,
                    target: tank.id,
                    hit_position: tank.position,
                    damage_hp: pulse,
                    penetrated: false,
                    cause: DamageCause::Drowning,
                    module: None,
                    target_destroyed: target_was_alive && tank.hit_points == 0,
                    ..Default::default()
                },
            );
            if tank.hit_points == 0 {
                break;
            }
        }
    }
}

/// How many damage pulses a submersion clock of `submerged_s` has earned past the flood.
fn drown_pulses(submerged_s: f32) -> u32 {
    if submerged_s <= ENGINE_FLOOD_S {
        return 0;
    }
    ((submerged_s - ENGINE_FLOOD_S) / DROWN_PULSE_INTERVAL_S).floor() as u32
}
