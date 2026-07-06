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
    /// Rounds remaining per ammo slot (`GunSpec::ammo_options()` order). Pre-ammo fixtures load
    /// with the default stock-heavy fill (an empty rack would silently refuse their recorded
    /// shots); spawned tanks always start from `spec.ammo`.
    #[serde(default = "default_ammo_counts")]
    pub ammo_counts: [u16; game_core::MAX_AMMO_SLOTS],
    /// The ammo slot the next shot fires from. Switching restarts the reload.
    #[serde(default)]
    pub selected_ammo: u8,
    /// Bitmask of teams that can currently see this tank (bit `t` = `TeamId(t+1)`), recomputed by
    /// the LOS spotting pass. `serde(default)` keeps pre-spotting fixtures loading (unspotted).
    #[serde(default)]
    pub spotted_mask: u8,
}

fn default_ammo_counts() -> [u16; game_core::MAX_AMMO_SLOTS] {
    game_core::AmmoLoadout::default().counts
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

    /// The shell the currently selected ammo slot fires — `ammo_options()` order, clamped so a
    /// corrupt index degrades to the stock round instead of panicking.
    pub fn selected_shell(&self) -> game_core::ShellSpec {
        let options = self.spec.gun.ammo_options();
        let index = (self.selected_ammo as usize).min(options.len().saturating_sub(1));
        options[index]
    }

    /// Where the next shell leaves the barrel: the mount chain pivoted about trunnion and ring,
    /// scaled to the installed barrel's length. The authoritative fire path and the bot aim
    /// solver both read the muzzle from here so they can never disagree about the spawn point.
    pub fn muzzle_world_position(&self) -> Vec3 {
        // A non-stock gun fires from its own barrel tip: scale the muzzle by installed/stock
        // length so the shell spawn tracks the longer/shorter barrel.
        let stock_barrel = self.spec.kind.stock_barrel_length_m();
        let barrel_scale =
            if stock_barrel > 0.0 { self.spec.gun.barrel_length_m / stock_barrel } else { 1.0 };
        game_core::math::muzzle_world_position_scaled(
            &self.spec.mounts,
            self.position,
            self.hull_pose(),
            self.turret_yaw_rad,
            self.gun_pitch_rad,
            barrel_scale,
        )
    }
}
