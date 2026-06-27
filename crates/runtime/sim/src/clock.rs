use serde::{Deserialize, Serialize};

use crate::FixedTimestep;

pub const DEFAULT_SIMULATION_TICK_HZ: u32 = 60;
pub const DEFAULT_SERVER_TICK_HZ: u32 = 60;
pub const DEFAULT_SNAPSHOT_HZ: u32 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SimulationClock {
    tick: u64,
    timestep: FixedTimestep,
}

impl SimulationClock {
    pub fn new(timestep: FixedTimestep) -> Self {
        Self { tick: 0, timestep }
    }

    pub fn tick(self) -> u64 {
        self.tick
    }

    pub fn timestep(self) -> FixedTimestep {
        self.timestep
    }

    pub fn advance_tick(&mut self) {
        self.tick += 1;
    }

    pub fn advance_ticks(&mut self, count: u32) {
        self.tick += u64::from(count);
    }
}
