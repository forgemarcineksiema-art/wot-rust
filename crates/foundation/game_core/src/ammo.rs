//! The ammo rack: how many rounds of each shell type a tank carries into battle. This is the
//! vision's "honest ammo" — no premium rounds; the loadout decision is WHICH rounds fill the
//! rack (and how many of each), made in the garage, and battle switching costs a full reload
//! (sim-side rule).

use serde::{Deserialize, Serialize};

use crate::VehicleKind;

/// Number of loadable shell types per gun — the ammo-rack slot count, in
/// `GunSpec::ammo_options()` order (stock AP, the gun's special round, HE).
pub const MAX_AMMO_SLOTS: usize = 3;

/// Fallback rack size for fixtures and the test-only prototype; real vehicles author theirs in
/// [`VehicleKind::ammo_capacity`].
pub const fn default_ammo_capacity() -> u16 {
    40
}

impl VehicleKind {
    /// The vehicle's HISTORICAL rack capacity — a real, characterful stat, not a flat constant.
    /// A big German hull hauls a deep rack; the IS-3 pays for its silhouette and 122 mm alpha
    /// with 28 stowed rounds. At battle reload cadence a shallow rack genuinely can run dry,
    /// which is the honest-ammo trade working as designed.
    pub fn ammo_capacity(self) -> u16 {
        match self {
            VehicleKind::PrototypeMedium => default_ammo_capacity(),
            VehicleKind::T54_1951 => 34,
            VehicleKind::T55A => 43,
            VehicleKind::TigerI => 92,
            VehicleKind::TigerII => 84,
            VehicleKind::Jagdtiger => 40,
            VehicleKind::PantherII => 79,
            VehicleKind::IS3 => 28,
        }
    }
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
    use crate::VehicleKind;

    #[test]
    fn rack_capacities_are_authored_and_characterful_not_one_flat_constant() {
        // The honest-ammo pillar: capacity is a real per-vehicle stat. Lock the historical
        // ordering extremes — the deep German racks against the famously shallow IS-3 — and that
        // every playable vehicle's assembled spec actually carries its authored capacity.
        assert!(
            VehicleKind::IS3.ammo_capacity() < VehicleKind::T54_1951.ammo_capacity(),
            "the IS-3 pays for its 122 mm with the shallowest rack"
        );
        assert!(
            VehicleKind::T54_1951.ammo_capacity() < VehicleKind::TigerI.ammo_capacity(),
            "the Tiger's hull hauls a far deeper rack than the low T-54"
        );
        let distinct: std::collections::HashSet<u16> =
            VehicleKind::PLAYABLE.iter().map(|kind| kind.ammo_capacity()).collect();
        assert!(distinct.len() >= 5, "capacities are authored per vehicle, not one constant");
        for kind in VehicleKind::PLAYABLE {
            let spec = kind.spec();
            assert_eq!(spec.ammo_capacity, kind.ammo_capacity(), "{kind:?} spec capacity");
            assert_eq!(spec.ammo.total(), spec.ammo_capacity, "{kind:?} default fill sums");
        }
    }

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
