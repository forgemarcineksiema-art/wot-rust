use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotSchedule {
    server_tick_hz: u32,
    snapshot_hz: u32,
    server_tick_interval: u64,
}

impl SnapshotSchedule {
    pub fn fixed(server_tick_hz: u32, snapshot_hz: u32) -> Self {
        assert!(server_tick_hz > 0, "server tick rate must be greater than zero");
        assert!(snapshot_hz > 0, "snapshot rate must be greater than zero");
        assert!(snapshot_hz <= server_tick_hz, "snapshot rate must not exceed server tick rate");
        assert!(
            server_tick_hz.is_multiple_of(snapshot_hz),
            "snapshot rate must divide server tick rate for a fixed schedule"
        );

        Self {
            server_tick_hz,
            snapshot_hz,
            server_tick_interval: u64::from(server_tick_hz / snapshot_hz),
        }
    }

    pub fn server_tick_hz(self) -> u32 {
        self.server_tick_hz
    }

    pub fn snapshot_hz(self) -> u32 {
        self.snapshot_hz
    }

    pub fn server_tick_interval(self) -> u64 {
        self.server_tick_interval
    }

    pub fn should_emit(self, server_tick: u64) -> bool {
        server_tick > 0 && server_tick.is_multiple_of(self.server_tick_interval)
    }
}
