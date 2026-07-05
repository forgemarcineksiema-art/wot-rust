//! The ammo rack: how many rounds of each shell type a tank carries into battle. This is the
//! vision's "honest ammo" — no premium rounds; the loadout decision is WHICH rounds fill the
//! rack, made in the garage, and battle switching costs a full reload (sim-side rule).

use serde::{Deserialize, Serialize};

/// Number of loadable shell types per gun — the ammo-rack slot count, in
/// `GunSpec::ammo_options()` order (stock AP, APCR, HE).
pub const MAX_AMMO_SLOTS: usize = 3;

/// Flat default rack size; per-vehicle tuning is a follow-up.
pub const fn default_ammo_capacity() -> u16 {
    40
}

/// What the rack carries into battle: per-slot round counts plus the slot selected at spawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AmmoLoadout {
    /// Rounds per ammo slot, `GunSpec::ammo_options()` order.
    pub counts: [u16; MAX_AMMO_SLOTS],
    /// Slot index selected when the battle starts.
    pub initial_selected: u8,
}

impl AmmoLoadout {
    /// The stock-heavy default fill: 60/25/15 percent of capacity, rounding remainder to stock,
    /// so the sum always equals `capacity`.
    pub fn default_for(capacity: u16) -> Self {
        let apcr = (f32::from(capacity) * 0.25).floor() as u16;
        let he = (f32::from(capacity) * 0.15).floor() as u16;
        let stock = capacity - apcr - he;
        Self { counts: [stock, apcr, he], initial_selected: 0 }
    }

    pub fn total(&self) -> u16 {
        self.counts.iter().sum()
    }
}

impl Default for AmmoLoadout {
    fn default() -> Self {
        Self::default_for(default_ammo_capacity())
    }
}

#[cfg(test)]
mod tests {
    use super::AmmoLoadout;

    #[test]
    fn the_default_ammo_loadout_fills_the_whole_rack_stock_heavy() {
        let loadout = AmmoLoadout::default_for(40);
        assert_eq!(loadout.total(), 40, "the split must sum to capacity exactly");
        assert!(
            loadout.counts[0] > loadout.counts[1] && loadout.counts[1] > loadout.counts[2],
            "stock-heavy: AP > APCR > HE, got {:?}",
            loadout.counts
        );
        assert_eq!(loadout.initial_selected, 0, "the stock round starts loaded");

        // Odd capacities still sum exactly (the rounding remainder lands on stock).
        assert_eq!(AmmoLoadout::default_for(37).total(), 37);
    }
}
