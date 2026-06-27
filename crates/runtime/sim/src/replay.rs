use game_core::{TankId, TankSpec, TeamId};
use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::{FixedTimestep, SimulationState, TankCommand, TankState};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Replay {
    pub name: String,
    pub tick_rate_hz: u32,
    pub spawn: ReplaySpawn,
    pub frames: Vec<ReplayFrame>,
    pub expected: ReplayExpected,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ReplaySpawn {
    pub team: u16,
    pub position: [f32; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ReplayFrame {
    pub tank_id: u64,
    pub command: TankCommand,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ReplayExpected {
    pub tick: u64,
    pub tank_id: u64,
    pub position_z_min: f32,
    pub turret_yaw_min: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReplayReport {
    pub final_tick: u64,
    pub tanks: Vec<TankState>,
}

impl ReplayReport {
    pub fn tank(&self, id: TankId) -> Option<&TankState> {
        self.tanks.iter().find(|tank| tank.id == id)
    }
}

pub fn run_replay(replay: &Replay) -> ReplayReport {
    let mut state = SimulationState::new();
    state.spawn_tank(
        TeamId(replay.spawn.team),
        TankSpec::medium_test_tank(),
        Vec3::from_array(replay.spawn.position),
    );

    let timestep = FixedTimestep::from_hz(replay.tick_rate_hz);
    for frame in &replay.frames {
        state.apply_commands(&[(TankId(frame.tank_id), frame.command)], timestep);
    }

    ReplayReport { final_tick: state.tick(), tanks: state.tanks().to_vec() }
}
