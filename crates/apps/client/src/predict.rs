use game_core::{MODULE_SLOT_COUNT, ModuleSlot, TankSpec, TrackDamageMask};
use glam::Vec3;
use physics::{TankKinematicState, TankObstacle};
use sim::{
    AimingState, DriveModuleStatus, TankCommand, TankDriveState, TankDriveWorld, TrackDriveStatus,
    recover_dispersion, step_tank_drive,
};
use terrain::{HeightMap, StaticCoverObject};

/// Lockstep client-side prediction of the local tank's hull: stepped with the same fixed
/// dt and the same input as the authoritative server, so it matches the server tick-for-tick
/// and yields a smooth 60 Hz local position between 20 Hz snapshots — without interpolation
/// lag and, crucially, without per-snapshot reconciliation jitter.
pub struct LocalPredictor {
    spec: TankSpec,
    drive: TankDriveState,
    hit_points: u32,
    module_hit_points: [u32; MODULE_SLOT_COUNT],
    destroyed_modules_mask: u8,
    track_damage_mask: TrackDamageMask,
    /// The ammo slot the player believes is loaded: set optimistically on the 1/2/3 keys so the
    /// reticle's ballistics answer the same frame, reconciled from snapshots.
    selected_ammo: u8,
    /// Hardest landing (m/s of absorbed fall speed) since the render loop last consumed it.
    pending_landing_impact_mps: f32,
    seeded: bool,
    /// Pose at the start of the most recent tick, kept so rendering can interpolate the
    /// gap between the previous and current tick instead of snapping at 60 Hz boundaries.
    previous: PredictedPose,
    /// Signed forward speed at the start of the most recent tick — the motion twin of
    /// `previous`, so the presented speed interpolates alongside the presented pose.
    previous_forward_speed_mps: f32,
    /// Forward acceleration over the most recent tick (m/s², tick-domain, exact).
    tick_accel_long_mps2: f32,
}

impl LocalPredictor {
    pub fn new(spec: &TankSpec) -> Self {
        Self {
            spec: spec.clone(),
            drive: TankDriveState {
                kinematic: TankKinematicState::default(),
                aiming: AimingState::default(),
                aim_dispersion_mrad: spec.gun.dispersion_mrad,
            },
            hit_points: spec.hit_points,
            module_hit_points: spec.module_health.hit_points_by_slot(),
            destroyed_modules_mask: 0,
            track_damage_mask: TrackDamageMask::healthy(),
            selected_ammo: spec.ammo.initial_selected,
            pending_landing_impact_mps: 0.0,
            seeded: false,
            previous_forward_speed_mps: 0.0,
            tick_accel_long_mps2: 0.0,
            previous: PredictedPose {
                position: Vec3::ZERO,
                yaw_rad: 0.0,
                hull_pitch_rad: 0.0,
                hull_roll_rad: 0.0,
                turret_yaw_rad: 0.0,
                gun_pitch_rad: 0.0,
            },
        }
    }

    pub fn is_seeded(&self) -> bool {
        self.seeded
    }

    pub fn reset_to_spec(&mut self, spec: &TankSpec) {
        *self = Self::new(spec);
    }

    pub fn spec(&self) -> &TankSpec {
        &self.spec
    }

    pub fn selected_ammo(&self) -> u8 {
        self.selected_ammo
    }

    /// Adopt an ammo switch optimistically; the server applies the same request on the next
    /// command, so lockstep keeps them agreeing (invalid slots are clamped like the sim does).
    pub fn set_selected_ammo(&mut self, slot: u8) {
        self.selected_ammo = slot.min(game_core::MAX_AMMO_SLOTS as u8 - 1);
    }

    /// The shell the currently selected slot fires, from the predictor's (custom) loadout.
    pub fn selected_shell(&self) -> game_core::ShellSpec {
        let options = self.spec.gun.ammo_options();
        options[(self.selected_ammo as usize).min(options.len() - 1)]
    }

    /// Advance one fixed tick with the same input being sent to the server.
    pub fn step(
        &mut self,
        command: TankCommand,
        heightmap: &HeightMap,
        cover: &[StaticCoverObject],
        tank_obstacles: &[TankObstacle],
        dt: f32,
    ) {
        // Record the pre-step pose so the renderer can interpolate previous -> current over
        // the sub-tick remainder. Captured before any early return so it always tracks the
        // last fully-resolved tick.
        self.previous = self.current_pose();
        let speed_before = self.drive.kinematic.forward_speed();
        self.previous_forward_speed_mps = speed_before;
        self.step_drive(command, heightmap, cover, tank_obstacles, dt);
        self.tick_accel_long_mps2 =
            (self.drive.kinematic.forward_speed() - speed_before) / dt.max(1.0e-6);
    }

    fn step_drive(
        &mut self,
        command: TankCommand,
        heightmap: &HeightMap,
        cover: &[StaticCoverObject],
        tank_obstacles: &[TankObstacle],
        dt: f32,
    ) {
        // Mirror the server: dispersion recovers every tick, even for a dead hull.
        let gun_damage_fraction = self.module_damage_fraction(ModuleSlot::Gun);
        recover_dispersion(
            &mut self.drive.aim_dispersion_mrad,
            &self.spec,
            gun_damage_fraction,
            dt,
        );
        if self.hit_points == 0 {
            self.drive.kinematic.velocity = Vec3::ZERO;
            self.drive.kinematic.yaw_rate_rad_s = 0.0;
            return;
        }
        let tracks = if self.module_destroyed(ModuleSlot::Suspension) {
            TrackDriveStatus::broken()
        } else {
            TrackDriveStatus::from_track_damage(self.track_damage_mask)
        };
        let modules = DriveModuleStatus::from_module_hp(tracks, self.module_hit_points, &self.spec);
        let footprint = self.spec.contact_footprint();
        let world = TankDriveWorld {
            heightmap: Some(heightmap),
            cover,
            tank_obstacles,
            footprint: Some(&footprint),
        };
        let ground =
            step_tank_drive(&mut self.drive, &self.spec, modules, world, command.clamped(), dt);
        self.pending_landing_impact_mps =
            self.pending_landing_impact_mps.max(ground.landing_impact_mps);
    }

    /// The hardest landing since the last call, consumed by the render loop for the camera slam.
    pub fn take_landing_impact_mps(&mut self) -> f32 {
        std::mem::take(&mut self.pending_landing_impact_mps)
    }

    fn module_destroyed(&self, slot: ModuleSlot) -> bool {
        self.destroyed_modules_mask & slot.destroyed_mask_bit() != 0
    }

    fn module_damage_fraction(&self, slot: ModuleSlot) -> f32 {
        let full_hp = self.spec.module_health.hit_points(slot).max(1) as f32;
        let live_hp = self.module_hit_points[slot.wire_index()] as f32;
        (1.0 - live_hp / full_hp).clamp(0.0, 1.0)
    }

    pub fn position(&self) -> Vec3 {
        self.drive.kinematic.position
    }

    pub fn yaw(&self) -> f32 {
        self.drive.kinematic.yaw_rad
    }

    pub fn speed_mps(&self) -> f32 {
        self.drive.kinematic.speed()
    }

    /// Tick-domain hull motion for the presentation cues (sprung attitude, camera feel), with
    /// the forward speed blended `alpha` into the current tick alongside `interpolated_pose`.
    /// This is the motion source of truth for the local tank: the rigid body knows its velocity,
    /// so the presentation never has to differentiate presented positions against the render
    /// clock (which is what used to jitter the hull).
    pub fn motion(&self, alpha: f32) -> engine::TankMotion {
        let alpha = alpha.clamp(0.0, 1.0);
        let current = self.drive.kinematic.forward_speed();
        engine::TankMotion {
            forward_speed_mps: self.previous_forward_speed_mps
                + (current - self.previous_forward_speed_mps) * alpha,
            accel_long_mps2: self.tick_accel_long_mps2,
            yaw_rate_rad_s: self.drive.kinematic.yaw_rate_rad_s,
        }
    }

    pub fn turret_yaw(&self) -> f32 {
        self.drive.aiming.turret_yaw_rad
    }

    pub fn gun_pitch(&self) -> f32 {
        self.drive.aiming.gun_pitch_rad
    }

    /// The predicted hull orientation as the shared frame the muzzle chain hangs off.
    pub fn hull_pose(&self) -> game_core::math::HullPose {
        game_core::math::HullPose {
            yaw_rad: self.drive.kinematic.yaw_rad,
            pitch_rad: self.drive.kinematic.pitch_rad,
            roll_rad: self.drive.kinematic.roll_rad,
        }
    }

    /// Locally predicted aim dispersion in milliradians, evolved at 60 Hz from the last snapshot
    /// so the reticle's aim circle tracks the server between 20 Hz updates instead of lagging it.
    pub fn aim_dispersion_mrad(&self) -> f32 {
        self.drive.aim_dispersion_mrad
    }
}

mod pose;
mod sync;

pub use pose::PredictedPose;

#[cfg(test)]
mod interpolation_tests;
#[cfg(test)]
mod obstacle_tests;
#[cfg(test)]
mod parity_tests;
#[cfg(test)]
mod tests;
