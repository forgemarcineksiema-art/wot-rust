//! The sniper (gunner's optics) camera, split from `controller.rs` for the reviewability
//! budget. Geometry rules: the eye sits ON the turret-ring axis (no lateral slide during
//! traverse), rides the FULL hull attitude (yaw + authoritative pitch/roll — on a slope the
//! optics are where the tank holds them), and takes the vertical micro-damper's smoothed
//! height so ruts do not slam a 3-degree sight picture 1:1. The aim direction itself stays
//! rigid: damping position jolts is comfort, damping the aim would be lag.
//!
//! The aim is a **world-space sight ray** (`desired_yaw`/`desired_pitch` are world azimuth and
//! world elevation — see `aim::DesiredAim`), so `gun_direction` already gives the correct look
//! on any slope: the sight points where the player aims in the world, and the gun (solved
//! elsewhere, hull-aware) carries the hull tilt. Composing the hull into the *view* here would
//! double-count it and send the sight into the sky on a nose-down hull.

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::{BattleCameraMode, CameraSubject};

    fn subject(hull_pitch_rad: f32, hull_roll_rad: f32, desired_pitch_rad: f32) -> CameraSubject {
        CameraSubject {
            position: [0.0, 0.0, 0.0],
            vehicle: game_core::VehicleKind::T54_1951,
            hull_yaw_rad: 0.0,
            hull_pitch_rad,
            hull_roll_rad,
            turret_yaw_rad: 0.0,
            gun_pitch_rad: 0.0,
            view_yaw_rad: 0.0,
            desired_yaw_rad: 0.0,
            desired_pitch_rad,
        }
    }

    /// The sight ray is world-space: the sniper view elevation equals the desired world pitch no
    /// matter how the hull is tilted. This is the regression for "on a slope the sniper flies to
    /// the sky" — the view must not gain the hull's pitch.
    #[test]
    fn sniper_view_elevation_is_world_space_and_ignores_hull_tilt() {
        let mut controller = BattleCameraController::default();
        controller.set_mode(BattleCameraMode::Sniper);
        let env = BattleCameraEnvironment::empty();
        let want_pitch = -0.10_f32;

        let elevation = |subject: &CameraSubject| {
            let cam = controller.render_camera(subject, &env);
            let dir = (Vec3::from_array(cam.target) - Vec3::from_array(cam.eye)).normalize();
            dir.y.asin()
        };

        let level = elevation(&subject(0.0, 0.0, want_pitch));
        let nose_down = elevation(&subject(-0.20, 0.0, want_pitch));
        let rolled = elevation(&subject(0.0, 0.25, want_pitch));

        assert!((level - want_pitch).abs() < 1.0e-4, "flat: {level}");
        assert!(
            (nose_down - want_pitch).abs() < 1.0e-4,
            "nose-down hull must not lift the sight: {nose_down}"
        );
        assert!(
            (rolled - want_pitch).abs() < 1.0e-4,
            "roll must not tilt the sight elevation: {rolled}"
        );
    }
}
