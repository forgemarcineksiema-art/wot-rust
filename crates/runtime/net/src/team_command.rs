//! The command wheel's wire (protocol v51, interface program W-5): a word or a ping from one
//! crew, relayed by the server to its team — and only as many as the server admits, so a
//! modded client cannot flood a team's ears.

use game_core::TankId;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// The commands a crew can send. Append-only: the wire discriminant is the index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TeamCommand {
    Attack,
    Help,
    Reloading,
    Affirmative,
    Negative,
    BackToBase,
    FollowMe,
    /// A map ping; carries `map_position`.
    Ping,
}

impl TeamCommand {
    pub const ALL: [TeamCommand; 8] = [
        TeamCommand::Attack,
        TeamCommand::Help,
        TeamCommand::Reloading,
        TeamCommand::Affirmative,
        TeamCommand::Negative,
        TeamCommand::BackToBase,
        TeamCommand::FollowMe,
        TeamCommand::Ping,
    ];
}

/// A relayed command: who said it, what, about whom or where, and when.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TeamCommandRelay {
    pub from: TankId,
    pub command: TeamCommand,
    pub target: Option<TankId>,
    pub map_position: Option<[f32; 2]>,
    pub server_tick: u64,
}

/// How many commands one crew may send per window. The limit is the SERVER's (interface
/// program H16: „a limit that a client cannot mod away").
pub const TEAM_COMMANDS_PER_WINDOW: usize = 5;
/// The window, in seconds.
pub const TEAM_COMMAND_WINDOW_S: f32 = 60.0;

/// One crew's admission clock. Ticks in, not seconds: the server reasons in its own ticks.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TeamCommandLimiter {
    admitted_ticks: VecDeque<u64>,
}

impl TeamCommandLimiter {
    /// Admit a command at `now_tick` if fewer than the window's allowance were admitted in the
    /// last `TEAM_COMMAND_WINDOW_S` seconds at `tick_hz`.
    pub fn admit(&mut self, now_tick: u64, tick_hz: u32) -> bool {
        let window_ticks = (TEAM_COMMAND_WINDOW_S * tick_hz as f32).round() as u64;
        while self
            .admitted_ticks
            .front()
            .is_some_and(|tick| now_tick.saturating_sub(*tick) >= window_ticks)
        {
            self.admitted_ticks.pop_front();
        }
        if self.admitted_ticks.len() >= TEAM_COMMANDS_PER_WINDOW {
            return false;
        }
        self.admitted_ticks.push_back(now_tick);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_limiter_admits_five_in_a_minute_and_refuses_the_sixth() {
        let mut limiter = TeamCommandLimiter::default();
        let hz = 60;
        for i in 0..TEAM_COMMANDS_PER_WINDOW as u64 {
            assert!(limiter.admit(i * 10, hz), "command {i} within the allowance");
        }
        assert!(!limiter.admit(50, hz), "the sixth in the same minute is refused");
        assert!(!limiter.admit(59 * hz as u64, hz), "still refused just inside the window");
        assert!(limiter.admit(60 * hz as u64, hz), "the oldest has aged out after sixty seconds");
    }

    #[test]
    fn every_command_has_a_stable_discriminant() {
        for (index, command) in TeamCommand::ALL.iter().enumerate() {
            let bytes = bincode::serialize(command).expect("encode");
            assert_eq!(bytes, (index as u32).to_le_bytes(), "{command:?} moved on the wire");
        }
    }
}
