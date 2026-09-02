//! Fall damage: a hull the terrain catches after a flight absorbs the landing through its
//! suspension. Gentle hops are free; past [`SAFE_LANDING_MPS`] the hull takes hit-point damage
//! and the suspension takes double, mirroring how ramming charges the running gear.

use game_core::{DamageCause, DamageEvent, ModuleSlot};

use crate::event_stamp::BattleEventStamp;
use crate::tank_state::TankState;

/// Downward speed a landing absorbs for free (≈ a 1.5 m drop). Harder slams hurt.
pub const SAFE_LANDING_MPS: f32 = 6.0;
/// Hit points per (m/s over the safe landing speed)²: quadratic, like ram severity.
const LANDING_DAMAGE_FACTOR: f32 = 1.8;
/// A single landing never deletes a healthy tank outright; ammo-rack drama is not fall damage.
const LANDING_DAMAGE_MAX_HP: f32 = 260.0;

/// How far off the ground's plane a hull has to arrive before it lands on ONE TRACK rather than
/// flat. Five degrees puts the far belt a quarter of a metre clear on a T-54's gauge, which is a
/// genuine one-track arrival and not a rounding difference.
const ONE_TRACK_ARRIVAL_RAD: f32 = 0.09;

/// Fraction of the tipping angle a landing may reach.
///
/// This is the shape of the rollover decision, and the arithmetic behind it is in
/// `docs/contact-and-tracks-program.md`: every path to putting a tank on its roof is out of reach
/// except one, an asymmetric fall, and that one is left REACHABLE but recoverable. A hull swings
/// most of the way to its tipping angle, the suspension pays for it, and it comes back down. The
/// tank is never lost to terrain.
const MAX_LANDING_ROLL_FRACTION: f32 = 0.9;

/// The roll excursion an asymmetric landing kicks into the hull, in radians, signed like
/// `hull_roll_rad`.
///
/// Nothing new is stored for it. The excursion is ADDED to the authoritative roll, and the
/// attitude spring (`physics::HullSpring`, Inny Poziom G7) settles it back to the support plane
/// in one nod — the spring's state is authoritative and on the wire, so replays and the client
/// predictor stay exact through it.
///
/// The size comes from the fall, not from taste. The landing impulse acts at the track that
/// touches first, a half-gauge out from the centreline, and the hull resists it with its own
/// planar inertia — so the angular rate is `m·v·b / I`, and the excursion is the share of the
/// TIPPING energy that rate represents, mapped onto the tipping angle. A hull arriving flat gets
/// nothing; one arriving on a single track from three metres up gets nearly all of it.
fn landing_roll_excursion(tank: &TankState, impact_mps: f32, mismatch_rad: f32) -> f32 {
    let Some(stability) = game_core::stock_stability(tank.spec.kind) else { return 0.0 };
    let gauge = tank.spec.contact_footprint().half_gauge_x;
    if gauge <= 0.0 {
        return 0.0;
    }
    // How much of the landing lands on ONE track: flat arrivals have no lever to turn about.
    let one_track = (mismatch_rad.abs() / ONE_TRACK_ARRIVAL_RAD).clamp(0.0, 1.0);
    if one_track <= 0.0 {
        return 0.0;
    }
    // Angular rate about the belt that touched first — the same planar inertia the contact solver
    // gives a hull, taken about the contact edge.
    let plan = tank.spec.hull_plan();
    let mass = tank.spec.mass_kg.max(1.0);
    let width = 2.0 * plan.half_width_m;
    let length = 2.0 * plan.half_length_m;
    let inertia_edge = mass * (width * width + length * length) / 12.0
        + mass
            * (stability.tip_edge_m * stability.tip_edge_m
                + stability.com_height_m * stability.com_height_m);
    let rate = mass * impact_mps * gauge * one_track / inertia_edge.max(1.0);

    // The rate that would just reach the tipping point, from the same energy balance the program's
    // rollover table is built on: lift the centre of mass from where it rests to over the edge.
    let lift =
        (stability.tip_edge_m.hypot(stability.com_height_m) - stability.com_height_m).max(0.0);
    let tipping_rate =
        (2.0 * mass * game_core::math::GRAVITY_MPS2 * lift / inertia_edge.max(1.0)).max(0.0).sqrt();
    if tipping_rate <= 0.0 {
        return 0.0;
    }
    // Energy goes as the square of the rate, so that is the fraction of the way over it gets.
    let reach = (rate / tipping_rate).powi(2).clamp(0.0, MAX_LANDING_ROLL_FRACTION);
    let tipping_angle = (stability.tip_edge_m / stability.com_height_m.max(1.0e-3)).atan();
    // Toward the side that touched first: the ground's low side, which is where the hull is
    // already leaning relative to the plane.
    tipping_angle * reach * -mismatch_rad.signum()
}

/// Charge one tank for the landing the terrain just absorbed. `impact_mps` is the
/// [`physics::GroundStep::landing_impact_mps`] of the tick; below the safe threshold this is a
/// no-op, above it the hull and suspension pay and a self-inflicted damage event is emitted.
pub(crate) fn apply_landing_impact(
    tank: &mut TankState,
    impact_mps: f32,
    roll_mismatch_rad: f32,
    damage_events: &mut Vec<DamageEvent>,
    event_stamp: &mut BattleEventStamp,
) {
    if impact_mps <= 0.0 {
        return;
    }
    // The roll excursion is not gated on the damage threshold: a hull can come down hard on one
    // track from a height that costs it nothing, and it should still lurch.
    tank.hull_roll_rad += landing_roll_excursion(tank, impact_mps, roll_mismatch_rad);
    if impact_mps <= SAFE_LANDING_MPS {
        return;
    }
    let severity = impact_mps - SAFE_LANDING_MPS;
    let damage = (severity * severity * LANDING_DAMAGE_FACTOR)
        .round()
        .clamp(1.0, LANDING_DAMAGE_MAX_HP) as u32;
    let target_was_alive = tank.hit_points > 0;
    tank.hit_points = tank.hit_points.saturating_sub(damage);
    tank.modules.damage(ModuleSlot::Suspension, damage.saturating_mul(2));
    event_stamp.push_damage(
        damage_events,
        DamageEvent {
            source: tank.id,
            target: tank.id,
            hit_position: tank.position,
            damage_hp: damage,
            penetrated: false,
            cause: DamageCause::Impact,
            module: Some(ModuleSlot::Suspension),
            target_destroyed: target_was_alive && tank.hit_points == 0,
            ..Default::default()
        },
    );
}
