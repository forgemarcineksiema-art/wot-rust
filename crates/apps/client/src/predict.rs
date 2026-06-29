use game_core::math::lerp_angle;
use game_core::{MODULE_SLOT_COUNT, ModuleSlot, TankSpec, TrackDamageMask};
use glam::Vec3;
use net::TankSnapshot;
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
/// A render-interpolatable snapshot of the predicted hull/turret pose. The visual tank and
/// the camera blend the previous tick's pose toward the current one so a 60 Hz sim renders
/// smoothly under a faster (or merely phase-drifting) vsync present clock.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PredictedPose {
    pub position: Vec3,
    pub yaw_rad: f32,
    pub turret_yaw_rad: f32,
    pub gun_pitch_rad: f32,
}

pub struct LocalPredictor {
    spec: TankSpec,
    drive: TankDriveState,
    hit_points: u32,
    module_hit_points: [u32; MODULE_SLOT_COUNT],
    destroyed_modules_mask: u8,
    track_damage_mask: TrackDamageMask,
    seeded: bool,
    /// Pose at the start of the most recent tick, kept so rendering can interpolate the
    /// gap between the previous and current tick instead of snapping at 60 Hz boundaries.
    previous: PredictedPose,
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
            seeded: false,
            previous: PredictedPose {
                position: Vec3::ZERO,
                yaw_rad: 0.0,
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

    /// Snap the predicted hull to an authoritative snapshot. In lockstep this matches the
    /// prediction (a no-op), but it also seeds the first frame and corrects rare divergence
    /// without a per-snapshot lerp that would visibly shake the camera.
    pub fn sync_to(&mut self, authoritative: &TankSnapshot) {
        if self.spec.kind != authoritative.vehicle {
            self.spec = authoritative.vehicle.spec();
        }
        self.drive.kinematic.position = Vec3::from_array(authoritative.position);
        self.drive.kinematic.yaw_rad = authoritative.yaw_rad;
        self.drive.aiming = AimingState {
            turret_yaw_rad: self.spec.effective_turret_yaw_rad(authoritative.turret_yaw_rad),
            turret_yaw_velocity_rad_s: authoritative.turret_yaw_velocity_rad_s,
            gun_pitch_rad: authoritative.gun_pitch_rad,
        };
        // Seed the live dispersion from the authority, then evolve it locally at 60 Hz.
        self.drive.aim_dispersion_mrad = authoritative.aim_dispersion_mrad;
        self.hit_points = authoritative.hit_points;
        self.module_hit_points = authoritative.module_hit_points;
        self.destroyed_modules_mask = authoritative.destroyed_modules_mask;
        self.track_damage_mask = TrackDamageMask::from_bits(authoritative.track_damage_mask);
        // On the very first sync there is no real "previous" tick to blend from, so anchor
        // it to the seed pose -- otherwise the first frames would lerp in from the origin.
        // On later corrections we leave `previous` alone so the render smoothly absorbs the
        // correction over the next frames instead of snapping the camera.
        if !self.seeded {
            self.previous = self.current_pose();
        }
        self.seeded = true;
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
        let modules = DriveModuleStatus {
            tracks,
            engine_ok: !self.module_destroyed(ModuleSlot::Engine),
            turret_ok: !self.module_destroyed(ModuleSlot::Turret),
            gun_damage_fraction,
        };
        let world = TankDriveWorld { heightmap: Some(heightmap), cover, tank_obstacles };
        step_tank_drive(&mut self.drive, &self.spec, modules, world, command.clamped(), dt);
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

    pub fn turret_yaw(&self) -> f32 {
        self.drive.aiming.turret_yaw_rad
    }

    pub fn gun_pitch(&self) -> f32 {
        self.drive.aiming.gun_pitch_rad
    }

    /// Locally predicted aim dispersion in milliradians, evolved at 60 Hz from the last snapshot
    /// so the reticle's aim circle tracks the server between 20 Hz updates instead of lagging it.
    pub fn aim_dispersion_mrad(&self) -> f32 {
        self.drive.aim_dispersion_mrad
    }

    /// The pose at the end of the most recently simulated tick.
    fn current_pose(&self) -> PredictedPose {
        PredictedPose {
            position: self.drive.kinematic.position,
            yaw_rad: self.drive.kinematic.yaw_rad,
            turret_yaw_rad: self.drive.aiming.turret_yaw_rad,
            gun_pitch_rad: self.drive.aiming.gun_pitch_rad,
        }
    }

    /// Render pose blended `alpha` of the way from the previous tick to the current one.
    /// `alpha` is the fixed-tick accumulator remainder in `[0, 1]`; this is what lets a
    /// 60 Hz sim render without judder under a faster or phase-drifting present clock.
    pub fn interpolated_pose(&self, alpha: f32) -> PredictedPose {
        let alpha = alpha.clamp(0.0, 1.0);
        let current = self.current_pose();
        PredictedPose {
            position: self.previous.position.lerp(current.position, alpha),
            yaw_rad: lerp_angle(self.previous.yaw_rad, current.yaw_rad, alpha),
            turret_yaw_rad: lerp_angle(self.previous.turret_yaw_rad, current.turret_yaw_rad, alpha),
            gun_pitch_rad: lerp_angle(self.previous.gun_pitch_rad, current.gun_pitch_rad, alpha),
        }
    }
}

#[cfg(test)]
mod interpolation_tests;
#[cfg(test)]
mod obstacle_tests;
#[cfg(test)]
mod parity_tests;
#[cfg(test)]
mod tests;
