use game_core::math::HullPose;
use game_core::{ModuleHealth, TankId, TankSpec, TeamId, TrackDamageMask};
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
    /// Hull angular velocity (yaw rate). Part of the rigid-body movement state so rotation carries
    /// inertia across ticks. `serde(default)` keeps older replay/snapshot fixtures (which predate
    /// the field) loading.
    #[serde(default)]
    pub hull_yaw_velocity_rad_s: f32,
    /// Authoritative hull pitch (+nose up) from the running-gear support plane, rate-limited in
    /// the drive step and frozen while airborne. `serde(default)` keeps older fixtures level.
    #[serde(default)]
    pub hull_pitch_rad: f32,
    /// Authoritative hull roll (+right side up); same lifecycle as `hull_pitch_rad`.
    #[serde(default)]
    pub hull_roll_rad: f32,
    pub hit_points: u32,
    pub reload_remaining_s: f32,
    pub aim_dispersion_mrad: f32,
    pub dispersion_shot_index: u32,
    /// Side-specific track damage. Zero means both tracks can provide traction.
    #[serde(default)]
    pub tracks: TrackDamageMask,
    /// Live hit points of the five module slots; at zero a module stops working.
    pub modules: ModuleHealth,
}

impl TankState {
    /// The hull's full authoritative orientation — the one frame the muzzle chain, armor
    /// normals and the hitbox all hang off.
    pub fn hull_pose(&self) -> HullPose {
        HullPose {
            yaw_rad: self.yaw_rad,
            pitch_rad: self.hull_pitch_rad,
            roll_rad: self.hull_roll_rad,
        }
    }
}
