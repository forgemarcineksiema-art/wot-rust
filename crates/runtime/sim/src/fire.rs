//! Fire as a wound, not a decoration.
//!
//! Until now `engine_fire` and `fuel_fire` were set on a penetration and read by nobody in the
//! simulation: a burning tank lost nothing. This module makes a fire cost hit points, credits them
//! to whoever lit it, and lets the crew put it out — the three properties that turn a particle
//! effect into a decision ("push while I burn, or break contact and smother it?").
//!
//! **The burn is deterministic and knowable.** No roll decides how long it lasts or how much it
//! takes: a fire the crew fights costs exactly `rate × FIRE_FIGHT_S`, and the player can learn
//! that number. What varies is only what the fire finds — a full-health hull survives it, a
//! wounded one does not.
//!
//! Ignition itself is deliberately hard to earn; that rule lives at the source, in
//! [`crate::combat`], and is gated on spall-level energy at a flammable component rather than on
//! merely brushing one.

use game_core::{DamageCause, DamageEvent};

use crate::TankState;
use crate::event_stamp::BattleEventStamp;

/// How long the crew needs to smother a fire. Long enough that a burning tank has to make a
/// decision about the next twelve seconds, short enough that it is a wound and not a sentence.
pub const FIRE_FIGHT_S: f32 = 12.0;

/// The hull drains in pulses this far apart, like drowning does — but slower than drowning's, on
/// purpose. Drowning is a four-second spiral; a fire lasts twelve, so half-second bites would
/// scroll two dozen entries through the damage log for a single fire. One bite a second is twelve
/// readable numbers, each big enough to mean something.
pub const FIRE_PULSE_INTERVAL_S: f32 = 1.0;

/// Hit points per second off a burning engine deck — an oil fire in a compartment built to
/// contain it. A fought engine fire therefore costs 108 hp: a real wound on a 1 550 hp hull,
/// nowhere near a death sentence on its own.
pub const ENGINE_FIRE_HP_PER_S: f32 = 9.0;

/// Hit points per second off burning fuel. Hotter than the engine's own fire — this is the one the
/// crews actually feared — but still a wound: 180 hp across a fought fire.
pub const FUEL_FIRE_HP_PER_S: f32 = 15.0;

/// Damage per second for whatever is currently alight. A tank burning on both counts takes the
/// WORSE of the two, not their sum — it is one vehicle on fire, and the fiercest source sets how
/// fast it is losing the hull.
pub fn burn_rate_hp_per_s(engine_fire: bool, fuel_fire: bool) -> f32 {
    match (engine_fire, fuel_fire) {
        (_, true) => FUEL_FIRE_HP_PER_S,
        (true, false) => ENGINE_FIRE_HP_PER_S,
        (false, false) => 0.0,
    }
}

/// Advance every fire by one tick: burn the hull, then let the crew win at [`FIRE_FIGHT_S`].
///
/// Damage is emitted as a real [`DamageEvent`] with [`DamageCause::Fire`] so the kill feed, the
/// damage log and the killer's credit all treat a burn-out exactly like any other death.
pub(crate) fn step_fire(
    tanks: &mut [TankState],
    dt: f32,
    damage_events: &mut Vec<DamageEvent>,
    event_stamp: &mut BattleEventStamp,
) {
    for tank in tanks.iter_mut() {
        if tank.hit_points == 0 {
            extinguish(tank);
            continue;
        }
        let rate = burn_rate_hp_per_s(tank.engine_fire, tank.fuel_fire);
        if rate <= 0.0 {
            tank.fire_s = 0.0;
            tank.fire_source = None;
            continue;
        }

        // Burn first, then check the crew: a fire lit this tick still gets its first bite, and a
        // fire the crew beats on this tick has already taken its last.
        //
        // The hull drains in PULSES, like drowning, and for the same two reasons. Per-tick damage
        // at 60 Hz would be 0.3 hp an 18 hp/s fire — which rounds to nothing every tick, so the
        // fire burns hard and takes zero (the exact bug this module exists to end) — and emitting a
        // `DamageEvent` per tick floods the damage log and the reliable combat lane: a measured
        // seven-minute battle produced 2 001 events against 53 before pulses were introduced.
        //
        // The toll is read off the clock rather than accumulated, so the burn is frame-rate
        // independent by construction and a fire always costs exactly `rate x FIRE_FIGHT_S`.
        let before_s = tank.fire_s;
        tank.fire_s += dt;
        let burned_through = |seconds: f32| {
            let pulses = (seconds / FIRE_PULSE_INTERVAL_S).floor();
            (rate * pulses * FIRE_PULSE_INTERVAL_S).floor() as u32
        };
        let toll = burned_through(tank.fire_s).saturating_sub(burned_through(before_s));
        if toll > 0 {
            let dealt = toll.min(tank.hit_points);
            tank.hit_points -= dealt;
            let source = tank.fire_source.unwrap_or(tank.id);
            event_stamp.push_damage(
                damage_events,
                DamageEvent {
                    source,
                    target: tank.id,
                    hit_position: tank.position,
                    damage_hp: dealt,
                    penetrated: false,
                    cause: DamageCause::Fire,
                    module: None,
                    ..Default::default()
                },
            );
        }

        if tank.hit_points == 0 || tank.fire_s >= FIRE_FIGHT_S {
            extinguish(tank);
        }
    }
}

/// How long a lit ammunition rack cooks before it resolves — shorter than a fought fire on
/// purpose: a rack is a fuze, and ten seconds is a decision window, not a sentence.
pub const RACK_COOKOFF_S: f32 = 10.0;

/// Hit points per second off burning charges while the rack cooks. Hotter than fuel — this is
/// propellant, not diesel — and the surviving case still pays it in full: winning the cook-off
/// costs 200 hp, which is what "my rack was on fire" should feel like afterwards.
pub const RACK_FIRE_HP_PER_S: f32 = 20.0;

/// Advance every cooking rack by one tick: burn, then resolve at [`RACK_COOKOFF_S`].
///
/// THE WHOLE RULE, one sentence each: a lit rack cooks for ten seconds; if the rack module is
/// still functional at the deadline the crew pulls the burning charges and wins, and if it is
/// DESTROYED there is nothing left to pull — the charges detonate, the hull dies, and the turret
/// leaves the ring exactly as an instant rack kill throws it.
///
/// Deterministic end to end: no roll decides the outcome, only the state of the rack the player
/// can read on the module row. A cook-off is deliberately NOT folded into the one-rate fire rule
/// (`burn_rate_hp_per_s`): that rule prices one vehicle burning, and this is a fuze with a drain —
/// a tank with burning fuel AND a cooking rack is two events, both of which it earned.
pub(crate) fn step_rack_cookoff(
    tanks: &mut [TankState],
    dt: f32,
    damage_events: &mut Vec<DamageEvent>,
    event_stamp: &mut BattleEventStamp,
) {
    for tank in tanks.iter_mut() {
        if !tank.rack_fire {
            continue;
        }
        if tank.hit_points == 0 {
            tank.rack_fire = false;
            tank.rack_fire_source = None;
            tank.rack_fire_s = 0.0;
            continue;
        }
        let source = tank.rack_fire_source.unwrap_or(tank.id);

        // The same pulse arithmetic as the compartment fires: the toll is read off the clock, so
        // the burn is frame-rate independent and costs exactly `rate x time` however it is stepped.
        let before_s = tank.rack_fire_s;
        tank.rack_fire_s += dt;
        let burned_through = |seconds: f32| {
            let pulses = (seconds / FIRE_PULSE_INTERVAL_S).floor();
            (RACK_FIRE_HP_PER_S * pulses * FIRE_PULSE_INTERVAL_S).floor() as u32
        };
        let toll = burned_through(tank.rack_fire_s).saturating_sub(burned_through(before_s));
        if toll > 0 {
            let dealt = toll.min(tank.hit_points);
            tank.hit_points -= dealt;
            event_stamp.push_damage(
                damage_events,
                DamageEvent {
                    source,
                    target: tank.id,
                    hit_position: tank.position,
                    damage_hp: dealt,
                    penetrated: false,
                    cause: DamageCause::Fire,
                    module: Some(game_core::ModuleSlot::AmmoRack),
                    target_destroyed: tank.hit_points == 0,
                    ..Default::default()
                },
            );
        }
        if tank.hit_points == 0 {
            tank.rack_fire = false;
            tank.rack_fire_source = None;
            continue;
        }

        if tank.rack_fire_s >= RACK_COOKOFF_S {
            tank.rack_fire = false;
            tank.rack_fire_source = None;
            tank.rack_fire_s = 0.0;
            if tank.modules.is_functional(game_core::ModuleSlot::AmmoRack) {
                // The crew pulled the burning charges: the rack held, so there was something
                // left to pull. The drain above is the price of the ten seconds.
                continue;
            }
            // Nothing left to pull. The detonation is the lighting shell's kill, ten seconds late.
            let dealt = tank.hit_points;
            tank.hit_points = 0;
            tank.turret_detached = true;
            event_stamp.push_damage(
                damage_events,
                DamageEvent {
                    source,
                    target: tank.id,
                    hit_position: tank.position,
                    damage_hp: dealt,
                    penetrated: false,
                    cause: DamageCause::AmmoRack,
                    module: Some(game_core::ModuleSlot::AmmoRack),
                    target_destroyed: true,
                    ..Default::default()
                },
            );
        }
    }
}

/// Light `tank`'s ammunition rack on behalf of `source`. Re-igniting a cooking rack does NOT
/// restart the clock, for the same reason re-igniting a fire does not: the crew is fighting all
/// of it, and a second hit must not hand the arsonist a fresh fuze.
pub(crate) fn ignite_rack(tank: &mut TankState, source: game_core::TankId) {
    if tank.rack_fire || tank.hit_points == 0 {
        return;
    }
    tank.rack_fire = true;
    tank.rack_fire_s = 0.0;
    tank.rack_fire_source = Some(source);
}

/// The fire is out — by the crew's work or because there is no longer a crew.
fn extinguish(tank: &mut TankState) {
    tank.engine_fire = false;
    tank.fuel_fire = false;
    tank.fire_source = None;
    tank.fire_s = 0.0;
}

/// Light `tank` on fire on behalf of `source`, recording the credit. Re-igniting an already
/// burning tank does NOT restart the clock — a crew fighting a fire is fighting all of it, and a
/// second hit must not hand the arsonist a fresh twelve seconds of free damage.
pub(crate) fn ignite(tank: &mut TankState, source: game_core::TankId, engine: bool, fuel: bool) {
    if !engine && !fuel {
        return;
    }
    let was_burning = tank.engine_fire || tank.fuel_fire;
    tank.engine_fire |= engine;
    tank.fuel_fire |= fuel;
    if !was_burning {
        tank.fire_source = Some(source);
        tank.fire_s = 0.0;
    }
}
