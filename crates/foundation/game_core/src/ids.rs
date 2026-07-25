use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct TankId(pub u64);

/// Stable identity of one projectile for its complete authoritative lifetime.
///
/// The high bits identify the firing tank and the low bits are that tank's monotonically
/// wrapping shot sequence. This is presentation/network identity only; combat never trusts it
/// for ownership or damage authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct ShellId(pub u64);

impl ShellId {
    pub fn from_shot(tank: TankId, sequence: u32) -> Self {
        // Mix the complete owner id and sequence instead of truncating either field. A collision
        // after billions of shots is harmless to combat and vanishingly unlikely in one snapshot.
        Self(crate::math::splitmix64(tank.0 ^ (sequence as u64).rotate_left(32)))
    }
}

/// Stable identity of one authoritative battle event.
///
/// Zero means "unstamped legacy/local event"; the authoritative simulation assigns positive ids
/// from one monotonically increasing space shared by damage and shell impacts. The ordering traits
/// let replication retain and merge bounded event tails without guessing from snapshot arrival
/// order.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
pub struct BattleEventId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TeamId(pub u16);

impl TeamId {
    /// This team's bit in a spotting/visibility bitmask (team 1 -> bit 0). Teams beyond 8 clamp
    /// into the top bit; the roster never approaches that. Shared by the server spotting pass and
    /// the client HUD so both index the mask the same way.
    pub fn spotting_bit(self) -> u8 {
        1u8 << (self.0 as u32).saturating_sub(1).min(7)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_identity_changes_with_owner_and_shot_sequence() {
        let first = ShellId::from_shot(TankId(7), 0);
        assert_ne!(first, ShellId::default());
        assert_ne!(first, ShellId::from_shot(TankId(7), 1));
        assert_ne!(first, ShellId::from_shot(TankId(8), 0));
        assert_eq!(first, ShellId::from_shot(TankId(7), 0));
    }
}
