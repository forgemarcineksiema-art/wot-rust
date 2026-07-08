//! Camera plumbing for the app: building the camera subject from the local tank and producing
//! both the LOGICAL camera (aiming) and the PRESENTED camera (rendering). Split from
//! `render.rs` for the reviewability budget.

use glam::Vec3;
use net::TankSnapshot;
use renderer_api::Camera;

use super::ClientApp;
use crate::{BattleCameraEnvironment, CameraSubject};

impl ClientApp {
    /// A world sight-ray seed `(yaw, pitch)` from the point the crosshair currently rests on: the
    /// direction muzzle -> current aim point. Entering the sniper view seeds from this so the
    /// crosshair stays on the same world point across the switch (no jump to the barrel line or the
    /// sky). Call it while still in the *outgoing* camera mode.
    pub(super) fn world_sight_seed(&self) -> Option<(f32, f32)> {
        let tank = self.local_render_tank()?;
        // Seed the direction from the SNIPER EYE (turret-ring axis at optic height), not the
        // muzzle: the eye is where the sniper view actually opens, and it does not move with turret
        // traverse. Using the muzzle skews the opening view toward the barrel line when the turret
        // is mid-traverse or reversed (muzzle sits metres off the ring), so the sniper would look
        // "somewhere between the gun and straight ahead" instead of at the crosshair's world point.
        let eye = crate::camera::sniper_eye_from_base(
            tank.vehicle,
            Vec3::from_array(tank.position),
            tank.hull_pose(),
        );
        let camera = self.camera_from_tank(tank);
        let aim = self.aim_world_point(&camera)?;
        let dir = (aim - eye).normalize_or_zero();
        (dir != Vec3::ZERO).then(|| (dir.x.atan2(dir.z), dir.y.clamp(-1.0, 1.0).asin()))
    }

    /// Clamp the world sight elevation to what the gun can reach on the current hull: convert the
    /// world sight ray to hull-relative gun angles, clamp the pitch to the gun's limits, and carry
    /// the clamped angle back to a world elevation. Keeps "crosshair == where the gun can shoot"
    /// true on slopes, where the hull tilt shifts the world reach.
    pub(super) fn clamp_desired_aim_to_gun_reach(&mut self) {
        let Some(tank) = self.local_render_tank() else { return };
        let hull = tank.hull_pose();
        let world_dir = game_core::math::gun_direction(
            self.desired_aim.yaw_rad(),
            self.desired_aim.pitch_rad(),
        );
        let (turret_yaw, gun_pitch) = game_core::math::world_direction_to_turret(hull, world_dir);
        let clamped = gun_pitch.clamp(sim::MIN_GUN_PITCH_RAD, sim::MAX_GUN_PITCH_RAD);
        if (clamped - gun_pitch).abs() > 1.0e-6 {
            let reached = game_core::math::gun_direction_world(hull, turret_yaw, clamped);
            self.desired_aim.set_pitch(reached.y.clamp(-1.0, 1.0).asin());
        }
    }

    /// The PRESENTED camera for this frame: the logical camera filtered through the mode
    /// transition blend and boom smoothing. Only the render path calls this; aiming keeps
    /// reading the unfiltered [`Self::camera_from_tank`].
    pub(super) fn presented_camera_for_player(&mut self, alpha: f32, dt: f32) -> Option<Camera> {
        let tank = self.interpolated_local_tank(alpha)?;
        let subject = self.camera_subject_from_tank(tank);
        let environment = BattleCameraEnvironment::with_obstacles(
            &self.battlefield.heightmap,
            &self.camera_obstacles,
        );
        Some(self.camera_controller.present(&subject, &environment, dt))
    }

    pub(super) fn camera_from_tank(&self, tank: TankSnapshot) -> Camera {
        let subject = self.camera_subject_from_tank(tank);
        let environment = BattleCameraEnvironment::with_obstacles(
            &self.battlefield.heightmap,
            &self.camera_obstacles,
        );
        self.camera_controller.render_camera(&subject, &environment)
    }

    pub(super) fn camera_subject_from_tank(&self, tank: TankSnapshot) -> CameraSubject {
        let gun_pitch = tank.gun_pitch_rad;
        let turret_view_yaw = tank.yaw_rad + tank.turret_yaw_rad;
        let view_yaw = if self.input.free_look {
            self.camera_controller.orbit_yaw_rad()
        } else if self.camera_controller.mode() == crate::BattleCameraMode::ThirdPerson {
            self.desired_aim.yaw_rad()
        } else {
            turret_view_yaw
        };
        CameraSubject::from_snapshot(tank, gun_pitch)
            .with_view_yaw(view_yaw)
            .with_desired_aim(self.desired_aim.yaw_rad(), self.desired_aim.pitch_rad())
    }
}
