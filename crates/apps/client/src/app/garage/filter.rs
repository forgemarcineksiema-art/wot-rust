//! The carousel's filter chips (interface program G9): CLASS, NATION and TIER, each a ring the
//! click walks — ALL at its head, then every value the roster actually has — and the roster
//! that passes them. The chips persist with the garage save by slug, like the daylight, so a
//! save from a build with more classes degrades one chip instead of poisoning the file.

use game_core::{Nation, VehicleClass, VehicleKind};

use crate::ui_strings::garage as words;

/// The three chips, in their order on the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum Chip {
    Class,
    Nation,
    Tier,
}

impl Chip {
    pub const ALL: [Chip; 3] = [Chip::Class, Chip::Nation, Chip::Tier];

    /// The chip's label.
    pub fn word(self) -> &'static str {
        match self {
            Chip::Class => words::CHIP_CLASS,
            Chip::Nation => words::CHIP_NATION,
            Chip::Tier => words::CHIP_TIER,
        }
    }

    pub fn index(self) -> u8 {
        Self::ALL.iter().position(|c| *c == self).expect("a chip is in ALL") as u8
    }
}

/// What the carousel shows: `None` on a chip is ALL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(in crate::app) struct CarouselFilter {
    pub class: Option<VehicleClass>,
    pub nation: Option<Nation>,
    pub tier: Option<u8>,
}

impl CarouselFilter {
    pub fn passes(&self, kind: VehicleKind) -> bool {
        self.class.is_none_or(|class| kind.class() == class)
            && self.nation.is_none_or(|nation| kind.nation() == nation)
            && self.tier.is_none_or(|tier| kind.tier() == tier)
    }

    /// The playable hulls that pass every chip, in the roster's order.
    pub fn roster(&self) -> Vec<VehicleKind> {
        VehicleKind::PLAYABLE.into_iter().filter(|kind| self.passes(*kind)).collect()
    }

    /// Walk one chip's ring by `dir`: ALL → each value in turn → ALL again.
    pub fn cycle(&mut self, chip: Chip, dir: i8) {
        match chip {
            Chip::Class => self.class = chip_ring_step(&VehicleClass::ALL, self.class, dir),
            Chip::Nation => self.nation = chip_ring_step(&Nation::ALL, self.nation, dir),
            Chip::Tier => self.tier = chip_ring_step(&tiers_present(), self.tier, dir),
        }
    }

    /// The word a chip prints for its value.
    pub fn value_word(&self, chip: Chip) -> String {
        match chip {
            Chip::Class => self
                .class
                .map_or_else(|| words::CHIP_ALL.to_string(), |class| class.label().to_uppercase()),
            Chip::Nation => self.nation.map_or_else(
                || words::CHIP_ALL.to_string(),
                |nation| nation.label().to_uppercase(),
            ),
            Chip::Tier => self.tier.map_or_else(
                || words::CHIP_ALL.to_string(),
                |tier| game_core::tier_roman(tier).to_string(),
            ),
        }
    }

    /// Whether a chip is set (its value reads in the lamp).
    pub fn is_set(&self, chip: Chip) -> bool {
        match chip {
            Chip::Class => self.class.is_some(),
            Chip::Nation => self.nation.is_some(),
            Chip::Tier => self.tier.is_some(),
        }
    }
}

/// One step around a ring whose head is ALL (`None`) and whose stops are `values`.
fn chip_ring_step<T: Copy + PartialEq>(values: &[T], current: Option<T>, dir: i8) -> Option<T> {
    let stops = values.len() as isize + 1;
    let at = current
        .and_then(|value| values.iter().position(|v| *v == value))
        .map_or(0, |i| i as isize + 1);
    let next = (at + isize::from(dir)).rem_euclid(stops);
    if next == 0 { None } else { Some(values[next as usize - 1]) }
}

/// Every tier the roster has, ascending — the TIER chip walks these, not I–X.
pub(super) fn tiers_present() -> Vec<u8> {
    let mut tiers: Vec<u8> = VehicleKind::PLAYABLE.iter().map(|kind| kind.tier()).collect();
    tiers.sort_unstable();
    tiers.dedup();
    tiers
}

/// The chips' on-disk names: a slug per variant, like the vehicles and the daylight.
pub(super) fn class_slug(class: VehicleClass) -> &'static str {
    match class {
        VehicleClass::Medium => "medium",
        VehicleClass::Heavy => "heavy",
        VehicleClass::TankDestroyer => "td",
    }
}

pub(super) fn class_from_slug(slug: &str) -> Option<VehicleClass> {
    VehicleClass::ALL.into_iter().find(|class| class_slug(*class) == slug)
}

pub(super) fn nation_slug(nation: Nation) -> &'static str {
    match nation {
        Nation::Ussr => "ussr",
        Nation::Germany => "germany",
        Nation::Britain => "britain",
    }
}

pub(super) fn nation_from_slug(slug: &str) -> Option<Nation> {
    Nation::ALL.into_iter().find(|nation| nation_slug(*nation) == slug)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every chip's ring starts and ends at ALL, walks every value the roster has and nothing
    /// it has not, and shift walks it the other way; the slugs round-trip.
    #[test]
    fn every_chip_ring_walks_the_rosters_values_and_returns_to_all() {
        let mut filter = CarouselFilter::default();
        assert_eq!(filter.roster(), VehicleKind::PLAYABLE.to_vec());
        for chip in Chip::ALL {
            let mut seen = Vec::new();
            loop {
                filter.cycle(chip, 1);
                if !filter.is_set(chip) {
                    break;
                }
                seen.push(filter.value_word(chip));
                assert!(!filter.roster().is_empty(), "{chip:?} = {seen:?}: a stop with no hull");
            }
            let expected = match chip {
                Chip::Class => VehicleClass::ALL.len(),
                Chip::Nation => Nation::ALL.len(),
                Chip::Tier => tiers_present().len(),
            };
            assert_eq!(seen.len(), expected, "{chip:?} walks every stop once");
            filter.cycle(chip, -1);
            assert_eq!(Some(filter.value_word(chip)), seen.last().cloned(), "shift walks back");
            filter.cycle(chip, 1);
            assert!(!filter.is_set(chip));
        }
        for class in VehicleClass::ALL {
            assert_eq!(class_from_slug(class_slug(class)), Some(class));
        }
        for nation in Nation::ALL {
            assert_eq!(nation_from_slug(nation_slug(nation)), Some(nation));
        }
        assert_eq!(class_from_slug("hovercraft"), None);
        assert!(tiers_present().windows(2).all(|pair| pair[0] < pair[1]));
    }
}
