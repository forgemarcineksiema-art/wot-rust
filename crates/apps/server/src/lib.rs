use net::SnapshotSchedule;
use sim::{DEFAULT_SERVER_TICK_HZ, DEFAULT_SNAPSHOT_HZ, FixedTimestep};

mod battle;
mod bot_aim;
mod bot_combat;
mod bot_routes;
mod bots;
mod local;
mod setup;

pub use battle::{
    BattleMode, BattleOutcome, BattleSeed, DrawReason, RANDOM_BATTLE_TIME_LIMIT_S,
    RandomBattleConfig,
};
pub use local::{AuthoritativeTick, LocalAuthoritativeServer};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ServerTickConfig {
    server_tick_hz: u32,
    timestep: FixedTimestep,
    snapshot_schedule: SnapshotSchedule,
}

impl ServerTickConfig {
    pub fn new(server_tick_hz: u32, snapshot_hz: u32) -> Self {
        Self {
            server_tick_hz,
            timestep: FixedTimestep::from_hz(server_tick_hz),
            snapshot_schedule: SnapshotSchedule::fixed(server_tick_hz, snapshot_hz),
        }
    }

    pub fn server_tick_hz(self) -> u32 {
        self.server_tick_hz
    }

    pub fn timestep(self) -> FixedTimestep {
        self.timestep
    }

    pub fn snapshot_schedule(self) -> SnapshotSchedule {
        self.snapshot_schedule
    }
}

impl Default for ServerTickConfig {
    fn default() -> Self {
        Self::new(DEFAULT_SERVER_TICK_HZ, DEFAULT_SNAPSHOT_HZ)
    }
}
