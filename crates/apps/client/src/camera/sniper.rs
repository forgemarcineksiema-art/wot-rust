//! The sniper (gunner's optics) camera, split from `controller.rs` for the reviewability
//! budget. Geometry rules: the eye sits ON the turret-ring axis (no lateral slide during
//! traverse), rides the FULL hull attitude (yaw + authoritative pitch/roll — on a slope the
//! optics are where the tank holds them), and takes the vertical micro-damper's smoothed
//! height so ruts do not slam a 3-degree sight picture 1:1. The aim direction itself stays
//! rigid: damping position jolts is comfort, damping the aim would be lag.

use game_core::MountFrames;
use game_core::math::gun_direction;
use glam::Vec3;
use renderer_api::Camera;

use super::controller::BattleCameraController;
use super::{BattleCameraEnvironment, CameraSubject, collision};

/// Sniper sight height above the gun trunnion — roughly where the gunner's optics sit.
const SNIPER_SIGHT_ABOVE_TRUNNION_M: f32 = 0.35;

impl BattleCameraController {
    pub(super) fn sniper_camera(
        &self,
        subject: &CameraSubject,
        environment: &BattleCameraEnvironment<'_>,
    ) -> Camera {
        let mounts = MountFrames::for_vehicle(subject.vehicle);
        let ring = mounts.turret_ring.translation;
        let sight_height = mounts.gun_trunnion.translation.y + SNIPER_SIGHT_ABOVE_TRUNNION_M;
        let basis = game_core::math::hull_basis(
            subject.hull_yaw_rad,
            subject.hull_pitch_rad,
            subject.hull_roll_rad,
        );
        // The smoothed anchor (vertical micro-damper, see `CameraSmoothing::advance`) supplies
        // the base height; x/z snap to the hull so aiming stays rigid.
        let base = self.smoothing.anchor.unwrap_or(subject.position_vec());
        let eye = base + basis * Vec3::new(ring.x, sight_height, ring.z);
        let eye = collision::apply_terrain_clearance(
            eye,
            environment,
            self.settings().terrain_clearance_m,
        );
        let aim = gun_direction(subject.desired_yaw_rad, subject.desired_pitch_rad);
        let target = eye + aim * 1_000.0;

        Camera {
            eye: eye.to_array(),
            target: target.to_array(),
            vertical_fov_degrees: self.sniper_fov_degrees(),
        }
    }
}
