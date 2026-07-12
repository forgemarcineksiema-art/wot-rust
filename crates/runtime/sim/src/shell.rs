use game_core::{ShellId, ShellSpec, TankId};
use glam::Vec3;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ShellState {
    pub id: ShellId,
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
    /// Most recently perforated hull. It is skipped while the projectile exits, preventing the
    /// next tick from treating the interior as a fresh entry hit; another vehicle can still be
    /// struck with the remaining kinetic penetration.
    #[serde(default)]
    pub last_penetrated_target: Option<TankId>,
}
