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
    /// True once the shell has glanced off a plate and continued: a ricochet gets exactly one
    /// second life (slower, blunted), the next surface resolves it for good.
    #[serde(default)]
    pub ricocheted_once: bool,
}
