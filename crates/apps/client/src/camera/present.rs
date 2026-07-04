//! The PRESENTED camera: cosmetic per-frame filtering applied on top of the logical camera
//! before it reaches the renderer. Two jobs, both pure comfort:
//!
//! - **Mode transitions.** Switching TPP <-> sniper used to hard-cut. The presented camera
//!   blends eye/target/FOV over a short beat, so the view *travels into* the optics instead of
//!   teleporting. The LOGICAL camera (aiming, sight solving) never blends — a transition is
//!   something you watch, not something you aim through.
//! - **Boom collision smoothing.** The third-person boom snaps SHORTER instantly (clipping into
//!   a wall is never acceptable for even one frame) but recovers its length smoothly when the
//!   obstacle passes, instead of popping back.

use glam::Vec3;
use renderer_api::Camera;

use super::controller::BattleCameraController;
use super::{BattleCameraEnvironment, BattleCameraMode, CameraSubject};

/// Seconds the TPP <-> sniper transition takes. Short enough to never delay aiming, long enough
/// that the eye tracks where the view went.
const MODE_BLEND_S: f32 = 0.14;
/// Boom length recovery rate (m/s) once a camera obstacle clears.
const BOOM_RECOVER_MPS: f32 = 14.0;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(super) struct PresentedCamera {
    last_mode: Option<BattleCameraMode>,
    last_camera: Option<Camera>,
    /// Live mode transition: the camera we are blending FROM and its age.
    blend: Option<(Camera, f32)>,
    /// Smoothed third-person boom length (meters).
    boom_m: Option<f32>,
}

impl BattleCameraController {
    /// The camera actually handed to the renderer this frame: the logical camera filtered
    /// through boom smoothing and the mode-transition blend. Call once per presented frame.
    pub fn present(
        &mut self,
        subject: &CameraSubject,
        environment: &BattleCameraEnvironment<'_>,
        dt: f32,
    ) -> Camera {
        let dt = dt.clamp(0.0, 0.1);
        let mut camera = self.render_camera(subject, environment);

        if self.mode() == BattleCameraMode::ThirdPerson {
            camera = self.presented.smooth_boom(camera, dt);
        } else {
            self.presented.boom_m = None;
        }

        let mode = self.mode();
        let state = &mut self.presented;
        if state.last_mode.is_some_and(|last| last != mode)
            && let Some(from) = state.last_camera
        {
            state.blend = Some((from, 0.0));
        }
        state.last_mode = Some(mode);

        if let Some((from, age)) = &mut state.blend {
            *age += dt;
            let t = (*age / MODE_BLEND_S).clamp(0.0, 1.0);
            let eased = t * t * (3.0 - 2.0 * t); // smoothstep: no velocity pop at either end
            camera = lerp_camera(*from, camera, eased);
            if t >= 1.0 {
                state.blend = None;
            }
        }
        state.last_camera = Some(camera);
        camera
    }
}

impl PresentedCamera {
    /// Shorten instantly (a clipping camera is worse than a popping one), recover smoothly.
    fn smooth_boom(&mut self, camera: Camera, dt: f32) -> Camera {
        let target = Vec3::from_array(camera.target);
        let eye = Vec3::from_array(camera.eye);
        let raw = (eye - target).length();
        if raw <= f32::EPSILON {
            self.boom_m = None;
            return camera;
        }
        let boom = match self.boom_m {
            Some(previous) if raw > previous => (previous + BOOM_RECOVER_MPS * dt).min(raw),
            _ => raw,
        };
        self.boom_m = Some(boom);
        Camera { eye: (target + (eye - target) / raw * boom).to_array(), ..camera }
    }
}

fn lerp_camera(from: Camera, to: Camera, t: f32) -> Camera {
    let mix3 =
        |a: [f32; 3], b: [f32; 3]| Vec3::from_array(a).lerp(Vec3::from_array(b), t).to_array();
    Camera {
        eye: mix3(from.eye, to.eye),
        target: mix3(from.target, to.target),
        vertical_fov_degrees: from.vertical_fov_degrees
            + (to.vertical_fov_degrees - from.vertical_fov_degrees) * t,
    }
}
