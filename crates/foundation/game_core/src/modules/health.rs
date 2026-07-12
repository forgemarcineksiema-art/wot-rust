use serde::{Deserialize, Serialize};

use super::{MODULE_SLOT_COUNT, ModuleSlot, VehicleModules};

/// Live battle hit points of each module slot. At zero a module is destroyed and stops
/// working — the simulation gates movement, traverse and firing on these (see `sim`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleHealth {
    engine: u32,
    suspension: u32,
    turret: u32,
    gun: u32,
    #[serde(default = "default_ammo_rack_hp")]
    ammo_rack: u32,
    radio: u32,
}

const fn default_ammo_rack_hp() -> u32 {
    240
}

impl ModuleHealth {
    /// Full health, taken from each installed module's `hit_points`.
    pub fn from_loadout(modules: &VehicleModules) -> Self {
        Self {
            engine: modules.engine.hit_points,
            suspension: modules.suspension.hit_points,
            turret: modules.turret.hit_points,
            gun: modules.gun.hit_points,
            ammo_rack: modules.gun.hit_points + modules.turret.hit_points / 2,
            radio: modules.radio.hit_points,
        }
    }

    pub fn hit_points(&self, slot: ModuleSlot) -> u32 {
        match slot {
            ModuleSlot::Engine => self.engine,
            ModuleSlot::Suspension => self.suspension,
            ModuleSlot::Turret => self.turret,
            ModuleSlot::Gun => self.gun,
            ModuleSlot::AmmoRack => self.ammo_rack,
            ModuleSlot::Radio => self.radio,
        }
    }

    pub fn hit_points_by_slot(&self) -> [u32; MODULE_SLOT_COUNT] {
        ModuleSlot::ALL.map(|slot| self.hit_points(slot))
    }

    pub fn is_functional(&self, slot: ModuleSlot) -> bool {
        self.hit_points(slot) > 0
    }

    /// Crew field repair: raise a slot to `hp` if it currently sits below it. Never lowers —
    /// a running module is not "repaired" down to the patch level.
    pub fn restore_to(&mut self, slot: ModuleSlot, hp: u32) {
        let live = match slot {
            ModuleSlot::Engine => &mut self.engine,
            ModuleSlot::Suspension => &mut self.suspension,
            ModuleSlot::Turret => &mut self.turret,
            ModuleSlot::Gun => &mut self.gun,
            ModuleSlot::AmmoRack => &mut self.ammo_rack,
            ModuleSlot::Radio => &mut self.radio,
        };
        *live = (*live).max(hp);
    }

    /// Apply `amount` damage to a slot, saturating at zero.
    pub fn damage(&mut self, slot: ModuleSlot, amount: u32) {
        let hp = match slot {
            ModuleSlot::Engine => &mut self.engine,
            ModuleSlot::Suspension => &mut self.suspension,
            ModuleSlot::Turret => &mut self.turret,
            ModuleSlot::Gun => &mut self.gun,
            ModuleSlot::AmmoRack => &mut self.ammo_rack,
            ModuleSlot::Radio => &mut self.radio,
        };
        *hp = hp.saturating_sub(amount);
    }

    /// Bit `i` (in [`ModuleSlot::ALL`] order) is set when that slot is destroyed. A compact
    /// form for replicating module status over the wire.
    pub fn destroyed_mask(&self) -> u8 {
        let mut mask = 0u8;
        for slot in ModuleSlot::ALL {
            if !self.is_functional(slot) {
                mask |= slot.destroyed_mask_bit();
            }
        }
        mask
    }

    /// Bit `i` is set when that slot is DAMAGED — alive but below its full pool (`self` is the
    /// live health, `full` the as-built baseline). Destroyed slots (0 HP) are NOT set here; they
    /// live in [`Self::destroyed_mask`]. Together the two masks describe the three-state module
    /// panel; the client can equally derive the same from live vs full HP (both already known),
    /// so this is the server-side convenience mirror of [`Self::destroyed_mask`].
    pub fn damaged_mask(&self, full: &ModuleHealth) -> u8 {
        let mut mask = 0u8;
        for slot in ModuleSlot::ALL {
            if module_condition(self.hit_points(slot), full.hit_points(slot))
                == ModuleCondition::Damaged
            {
                mask |= slot.destroyed_mask_bit();
            }
        }
        mask
    }
}

/// The three states the HUD paints a module in. Derived from live vs full pool so the client and
/// server agree without a dedicated wire field: full = whole, zero = knocked out, anything
/// between = a wounded module still doing its job at a penalty.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleCondition {
    Healthy,
    Damaged,
    Destroyed,
}

/// Classify a module from its live and full hit points. Zero reads Destroyed; a pool at or above
/// full reads Healthy (tolerates a live value nudged over the baseline); anything strictly
/// between reads Damaged.
pub fn module_condition(live_hp: u32, full_hp: u32) -> ModuleCondition {
    if live_hp == 0 {
        ModuleCondition::Destroyed
    } else if live_hp >= full_hp {
        ModuleCondition::Healthy
    } else {
        ModuleCondition::Damaged
    }
}

/// A barely-alive gun reloads this many times slower than a whole one.
const GUN_RELOAD_CEILING: f32 = 1.6;

/// Reload-time multiplier for a wounded gun: `1.0` at full pool, easing linearly UP to
/// [`GUN_RELOAD_CEILING`] as the pool drains toward 1 HP — the mirror of the engine/suspension
/// power floors (a damaged breech loads slower, it does not stop). A destroyed gun (0 HP) does
/// not fire at all, so the value there is moot. Shared by the server and the client predictor.
pub fn gun_reload_multiplier(live_hp: u32, full_hp: u32) -> f32 {
    let fraction = (live_hp as f32 / full_hp.max(1) as f32).clamp(0.0, 1.0);
    1.0 + (GUN_RELOAD_CEILING - 1.0) * (1.0 - fraction)
}

/// A wounded-but-running engine still delivers this floor of its drive power at 1 HP.
const ENGINE_POWER_FLOOR: f32 = 0.55;
/// A wounded suspension keeps this floor of its turn rate and yaw spool at 1 HP.
const SUSPENSION_AGILITY_FLOOR: f32 = 0.6;

/// Fraction of drive power a damaged engine still delivers: `1.0` at full pool, easing linearly
/// to the floor as the pool drains. Destruction is not a fraction — the drive gate
/// (`is_functional`) removes throttle entirely. Shared by the server and the client predictor.
pub fn engine_power_fraction(live_hp: u32, full_hp: u32) -> f32 {
    damaged_fraction(live_hp, full_hp, ENGINE_POWER_FLOOR)
}

/// Fraction of turn agility a damaged suspension still delivers; see [`engine_power_fraction`].
pub fn suspension_agility_fraction(live_hp: u32, full_hp: u32) -> f32 {
    damaged_fraction(live_hp, full_hp, SUSPENSION_AGILITY_FLOOR)
}

fn damaged_fraction(live_hp: u32, full_hp: u32, floor: f32) -> f32 {
    let fraction = (live_hp as f32 / full_hp.max(1) as f32).clamp(0.0, 1.0);
    floor + (1.0 - floor) * fraction
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn condition_reads_three_states_from_live_vs_full() {
        assert_eq!(module_condition(0, 150), ModuleCondition::Destroyed);
        assert_eq!(module_condition(150, 150), ModuleCondition::Healthy);
        assert_eq!(module_condition(37, 150), ModuleCondition::Damaged);
        // A live pool over its baseline still reads whole, never a phantom "damaged".
        assert_eq!(module_condition(151, 150), ModuleCondition::Healthy);
    }

    #[test]
    fn a_wounded_gun_reloads_slower_a_whole_one_does_not() {
        assert_eq!(gun_reload_multiplier(150, 150), 1.0);
        // The crew-patched floor (25% pool) is a clear, bounded penalty.
        let patched = gun_reload_multiplier(37, 150);
        assert!(patched > 1.3 && patched < 1.5, "patched-gun reload mult {patched}");
        // Draining toward death never exceeds the ceiling.
        assert!(gun_reload_multiplier(1, 150) <= GUN_RELOAD_CEILING);
        assert!(gun_reload_multiplier(1, 150) > patched);
    }

    #[test]
    fn damaged_mask_flags_only_wounded_slots_not_whole_or_dead_ones() {
        let full = ModuleHealth {
            engine: 200,
            suspension: 200,
            turret: 200,
            gun: 150,
            ammo_rack: 225,
            radio: 60,
        };
        let mut live = full;
        live.gun = 37; // damaged
        live.ammo_rack = 0; // destroyed -> belongs to destroyed_mask, not damaged_mask
        let mask = live.damaged_mask(&full);
        assert_eq!(
            mask & ModuleSlot::Gun.destroyed_mask_bit(),
            ModuleSlot::Gun.destroyed_mask_bit()
        );
        assert_eq!(mask & ModuleSlot::AmmoRack.destroyed_mask_bit(), 0);
        assert_eq!(mask & ModuleSlot::Engine.destroyed_mask_bit(), 0);
    }
}
