use game_core::{ModuleHealth, TankId, TankSpec, TeamId};
use glam::Vec3;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TankState {
    pub id: TankId,
    pub team: TeamId,
    pub spec: TankSpec,
    pub position: Vec3,
    pub yaw_rad: f32,
    pub turret_yaw_rad: f32,
    pub turret_yaw_velocity_rad_s: f32,
    pub gun_pitch_rad: f32,
    pub velocity_mps: Vec3,
    pub hit_points: u32,
    pub reload_remaining_s: f32,
    pub aim_dispersion_mrad: f32,
    pub dispersion_shot_index: u32,
    /// Live hit points of the five module slots; at zero a module stops working.
    pub modules: ModuleHealth,
}
