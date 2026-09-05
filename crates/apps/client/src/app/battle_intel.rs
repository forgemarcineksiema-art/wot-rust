//! What the battle tells every crew beyond the snapshot (protocol v51): the kills on the whole
//! field and the team's relayed commands. The session delivers them per tick; this inbox keeps
//! the recent ones for the HUD — the kill feed (H3), the top bar's frag counter (H1) and the
//! command wheel's echoes (H16) read from here, never from a snapshot they would have to guess.

use std::collections::VecDeque;

/// How many kills the feed remembers. A 7v7 has at most thirteen deaths a crew can watch.
const KILL_MEMORY: usize = 16;
/// How many relays are kept for the wheel's echo strip.
const COMMAND_MEMORY: usize = 8;

#[derive(Debug, Default)]
pub(crate) struct BattleIntel {
    kills: VecDeque<game_core::KillEvent>,
    team_commands: VecDeque<net::TeamCommandRelay>,
}

impl BattleIntel {
    /// Fold one session tick's deliveries in, oldest first, bounded.
    pub(crate) fn ingest(
        &mut self,
        kills: Vec<game_core::KillEvent>,
        team_commands: Vec<net::TeamCommandRelay>,
    ) {
        for kill in kills {
            if self.kills.len() == KILL_MEMORY {
                self.kills.pop_front();
            }
            self.kills.push_back(kill);
        }
        for relay in team_commands {
            if self.team_commands.len() == COMMAND_MEMORY {
                self.team_commands.pop_front();
            }
            self.team_commands.push_back(relay);
        }
    }

    /// The kills seen so far, oldest first.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn kills(&self) -> impl Iterator<Item = &game_core::KillEvent> {
        self.kills.iter()
    }

    /// The team's relayed commands, oldest first.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn team_commands(&self) -> impl Iterator<Item = &net::TeamCommandRelay> {
        self.team_commands.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_core::{DamageCause, KillEvent, TankId};

    fn kill(tick: u64) -> KillEvent {
        KillEvent {
            victim: TankId(tick),
            killer: None,
            cause: DamageCause::Fire,
            occurred_tick: tick,
        }
    }

    #[test]
    fn the_inbox_keeps_the_newest_kills_in_order_and_forgets_the_oldest() {
        let mut intel = BattleIntel::default();
        intel.ingest((0..(KILL_MEMORY as u64 + 3)).map(kill).collect(), Vec::new());
        let ticks: Vec<u64> = intel.kills().map(|k| k.occurred_tick).collect();
        assert_eq!(ticks.len(), KILL_MEMORY);
        assert_eq!(ticks.first(), Some(&3), "the three oldest were forgotten");
        assert_eq!(ticks.last(), Some(&(KILL_MEMORY as u64 + 2)));
        assert_eq!(intel.team_commands().count(), 0);
    }
}
