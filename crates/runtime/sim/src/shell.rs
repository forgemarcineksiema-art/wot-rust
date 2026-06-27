use game_core::{ShellSpec, TankId};
use glam::Vec3;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ShellState {
    pub owner: TankId,
    pub position: Vec3,
    pub velocity_mps: Vec3,
    pub shell: ShellSpec,
    pub age_seconds: f32,
    pub traveled_m: f32,
    pub max_age_seconds: f32,
}
