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
