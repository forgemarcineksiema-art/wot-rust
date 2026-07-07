use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrackSide {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TrackDamageMask(u8);

impl TrackDamageMask {
    pub const LEFT_BIT: u8 = 1 << 0;
    pub const RIGHT_BIT: u8 = 1 << 1;
    pub const BOTH_BITS: u8 = Self::LEFT_BIT | Self::RIGHT_BIT;

    pub const LEFT: Self = Self(Self::LEFT_BIT);
    pub const RIGHT: Self = Self(Self::RIGHT_BIT);
    pub const BOTH: Self = Self(Self::BOTH_BITS);

    pub const fn healthy() -> Self {
        Self(0)
    }

    pub const fn from_bits(bits: u8) -> Self {
        Self(bits & Self::BOTH_BITS)
    }

    pub const fn bits(self) -> u8 {
        self.0
    }

    pub const fn is_broken(self, side: TrackSide) -> bool {
        self.0 & side.bit() != 0
    }

    pub fn damage(&mut self, side: TrackSide) {
        self.0 |= side.bit();
    }

    pub fn damage_both(&mut self) {
        self.0 |= Self::BOTH_BITS;
    }

    /// Crew repair: the side is whole again (see `sim::repair` for the timing).
    pub fn repair(&mut self, side: TrackSide) {
        self.0 &= !side.bit();
    }

    pub const fn all_broken(self) -> bool {
        self.0 & Self::BOTH_BITS == Self::BOTH_BITS
    }

    pub const fn any_broken(self) -> bool {
        self.0 & Self::BOTH_BITS != 0
    }
}

impl TrackSide {
    pub const fn bit(self) -> u8 {
        match self {
            Self::Left => TrackDamageMask::LEFT_BIT,
            Self::Right => TrackDamageMask::RIGHT_BIT,
        }
    }
}
