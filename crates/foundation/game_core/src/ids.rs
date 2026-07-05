use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct TankId(pub u64);

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
