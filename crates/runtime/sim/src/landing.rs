//! Fall damage: a hull the terrain catches after a flight absorbs the landing through its
//! suspension. Gentle hops are free; past [`SAFE_LANDING_MPS`] the hull takes hit-point damage
//! and the suspension takes double, mirroring how ramming charges the running gear.

use game_core::{DamageCause, DamageEvent, ModuleSlot};

use crate::tank_state::TankState;

/// Downward speed a landing absorbs for free (≈ a 1.5 m drop). Harder slams hurt.
pub const SAFE_LANDING_MPS: f32 = 6.0;
/// Hit points per (m/s over the safe landing speed)²: quadratic, like ram severity.
const LANDING_DAMAGE_FACTOR: f32 = 1.8;
/// A single landing never deletes a healthy tank outright; ammo-rack drama is not fall damage.
const LANDING_DAMAGE_MAX_HP: f32 = 260.0;

/// Charge one tank for the landing the terrain just absorbed. `impact_mps` is the
/// [`physics::GroundStep::landing_impact_mps`] of the tick; below the safe threshold this is a
/// no-op, above it the hull and suspension pay and a self-inflicted damage event is emitted.
pub(crate) fn apply_landing_impact(
    tank: &mut TankState,
    impact_mps: f32,
    damage_events: &mut Vec<DamageEvent>,
) {
    if impact_mps <= SAFE_LANDING_MPS {
        return;
    }
    let severity = impact_mps - SAFE_LANDING_MPS;
    let damage =
        (severity * severity * LANDING_DAMAGE_FACTOR).round().clamp(1.0, LANDING_DAMAGE_MAX_HP)
            as u32;
    tank.hit_points = tank.hit_points.saturating_sub(damage);
    tank.modules.damage(ModuleSlot::Suspension, damage.saturating_mul(2));
    damage_events.push(DamageEvent {
        source: tank.id,
        target: tank.id,
        hit_position: tank.position,
        damage_hp: damage,
        penetrated: false,
        cause: DamageCause::Impact,
        module: Some(ModuleSlot::Suspension),
        ..Default::default()
    });
}
