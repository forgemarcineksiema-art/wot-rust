//! Camera plumbing for the app: building the camera subject from the local tank and producing
//! both the LOGICAL camera (aiming) and the PRESENTED camera (rendering). Split from
//! `render.rs` for the reviewability budget.

use net::TankSnapshot;
use renderer_api::Camera;

use super::ClientApp;
use crate::{BattleCameraEnvironment, CameraSubject};

impl ClientApp {
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
