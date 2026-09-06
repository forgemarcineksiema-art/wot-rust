//! Who saw whom, and when (protocol v52, interface program W-7): one line of the spotting log
//! a crew receives with the battle's end — an enemy observer, the range at the moment its
//! line of sight opened, and the span of ticks it held. Delivered AFTER the battle: a live
//! client never holds an observer's identity, so nothing on the HUD can leak it.

use game_core::TankId;
use serde::{Deserialize, Serialize};

/// One span of one enemy's sight on the recipient's hull. Append-only.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SpottingRecord {
    /// The enemy hull whose eyes (or whose team's radio, once relayed) held the crew.
    pub observer: TankId,
    /// Hull to hull, in metres, when the span opened.
    pub distance_m: f32,
    pub from_tick: u64,
    pub to_tick: u64,
}
