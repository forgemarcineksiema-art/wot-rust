use serde::{Deserialize, Serialize};

/// The damageable / swappable module slots on a vehicle. The hull is the fixed chassis and
/// is not a slot (it holds the vehicle's main hit-point pool instead).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModuleSlot {
    Engine,
    Suspension,
    Turret,
    Gun,
    AmmoRack,
    Radio,
}

pub const MODULE_SLOT_COUNT: usize = 6;

impl ModuleSlot {
    /// Every slot, in a stable order (also the wire order for module-health bitsets).
    pub const ALL: [ModuleSlot; MODULE_SLOT_COUNT] = [
        ModuleSlot::Engine,
        ModuleSlot::Suspension,
        ModuleSlot::Turret,
        ModuleSlot::Gun,
        ModuleSlot::AmmoRack,
        ModuleSlot::Radio,
    ];

    pub fn wire_index(self) -> usize {
        Self::ALL.iter().position(|slot| *slot == self).expect("module slot must be listed in ALL")
    }

    pub fn destroyed_mask_bit(self) -> u8 {
        1 << self.wire_index()
    }
}
