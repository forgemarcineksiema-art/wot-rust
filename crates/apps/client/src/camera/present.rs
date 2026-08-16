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
/// Fractions of the sprung hull's dynamic dive/heave the presented TPP camera rides
/// (Immersja B1). Deliberately well under 1: the hull SHOWS the motion, the camera only
/// corroborates it — a camera that matched the hull would read as a bobblehead. No new
/// state lives here: both inputs are already spring-filtered upstream (`engine::attitude`)
/// and already retire through the predictor's freeze, so the presented camera stays a pure
/// function of frozen-safe inputs.
const SPRUNG_DIVE_FRAC: f32 = 0.35;
const SPRUNG_HEAVE_FRAC: f32 = 0.5;
/// Defensive caps, independent of the upstream spring caps (0.035 rad / 0.30 m): a runaway
/// input may nod the view, never throw it.
const SPRUNG_DIVE_CAP_RAD: f32 = 0.02;
const SPRUNG_HEAVE_CAP_M: f32 = 0.2;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(super) struct PresentedCamera {
    last_mode: Option<BattleCameraMode>,
    last_camera: Option<Camera>,
    /// Live mode transition: the camera and mode we are blending FROM and the blend's age.
    blend: Option<(Camera, BattleCameraMode, f32)>,
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
        self.tick_death_orbit(dt);
        let mut camera = self.render_camera(subject, environment);

        if self.mode() == BattleCameraMode::ThirdPerson {
            camera = self.presented.smooth_boom(camera, dt);
            camera = ride_sprung_hull(camera, subject);
        } else {
            self.presented.boom_m = None;
        }

        let mode = self.mode();
        let state = &mut self.presented;
        if let Some(last) = state.last_mode.filter(|last| *last != mode)
            && let Some(from) = state.last_camera
        {
            state.blend = Some((from, last, 0.0));
        }
        state.last_mode = Some(mode);

        if let Some((from, _, age)) = &mut state.blend {
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

    /// How much of the sniper dressing (scope surround) shows this frame, 0..1. Rides the SAME
    /// clock as the presented camera blend, so the optics housing irises in as the view travels
    /// into the scope and lifts away as it leaves — instead of hard-cutting while the camera is
    /// still mid-flight.
    pub fn scope_dressing(&self) -> f32 {
        let mode = self.presented.last_mode.unwrap_or(self.mode());
        let eased = self.presented.blend.map(|(_, _, age)| {
            let t = (age / MODE_BLEND_S).clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        });
        match (mode, eased) {
            (BattleCameraMode::Sniper, Some(eased)) => eased,
            (BattleCameraMode::Sniper, None) => 1.0,
            (BattleCameraMode::ThirdPerson, Some(eased))
                if matches!(self.presented.blend, Some((_, BattleCameraMode::Sniper, _))) =>
            {
                1.0 - eased
            }
            _ => 0.0,
        }
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

/// The presented TPP rig rides a fraction of the sprung hull (Immersja B1): heave lifts the
/// whole rig, the dynamic dive/squat tilts the view about the camera's right axis — braking
/// dips the nose of the VIEW the way it dips the nose of the TANK. Presented layer only:
/// the logical camera (aiming) and the sniper never pass through here.
fn ride_sprung_hull(camera: Camera, subject: &CameraSubject) -> Camera {
    let dive = (subject.sprung_dive_rad * SPRUNG_DIVE_FRAC)
        .clamp(-SPRUNG_DIVE_CAP_RAD, SPRUNG_DIVE_CAP_RAD);
    let heave =
        (subject.sprung_heave_m * SPRUNG_HEAVE_FRAC).clamp(-SPRUNG_HEAVE_CAP_M, SPRUNG_HEAVE_CAP_M);
    if dive == 0.0 && heave == 0.0 {
        return camera;
    }
    let eye = Vec3::from_array(camera.eye) + Vec3::Y * heave;
    let target = Vec3::from_array(camera.target) + Vec3::Y * heave;
    let dir = target - eye;
    let right = dir.cross(Vec3::Y).normalize_or_zero();
    if right == Vec3::ZERO {
        // Looking straight along the vertical axis: no stable right axis, keep the heave only.
        return Camera { eye: eye.to_array(), target: target.to_array(), ..camera };
    }
    let tilted = glam::Quat::from_axis_angle(right, dive) * dir;
    Camera { eye: eye.to_array(), target: (eye + tilted).to_array(), ..camera }
}

/// Blend two cameras by **view direction**, not by target position. Lerping the target position
/// between a near TPP look-point (~5 m) and the far sniper target (~1000 m) swept the view angle
/// wildly — the sight appeared to fly up on entry. Interpolating the *direction* (short-arc nlerp)
/// plus the eye and the look distance keeps the crosshair travelling straight to its mark.
fn lerp_camera(from: Camera, to: Camera, t: f32) -> Camera {
    let from_eye = Vec3::from_array(from.eye);
    let to_eye = Vec3::from_array(to.eye);
    let from_dir = (Vec3::from_array(from.target) - from_eye).normalize_or_zero();
    let to_dir = (Vec3::from_array(to.target) - to_eye).normalize_or_zero();
    let dir = from_dir.lerp(to_dir, t).normalize_or_zero();
    let dist = (Vec3::from_array(from.target) - from_eye).length()
        + ((Vec3::from_array(to.target) - to_eye).length()
            - (Vec3::from_array(from.target) - from_eye).length())
            * t;
    let eye = from_eye.lerp(to_eye, t);
    // FOV blends in MAGNIFICATION space (1/fov): perceived zoom is the reciprocal of the FOV, so
    // a linear FOV sweep reads as a violent snap at the wide end and a crawl at the narrow end.
    // Interpolating the magnification keeps the zoom RATE constant to the eye across the blend.
    let from_mag = 1.0 / from.vertical_fov_degrees.max(0.1);
    let to_mag = 1.0 / to.vertical_fov_degrees.max(0.1);
    Camera {
        eye: eye.to_array(),
        target: (eye + dir * dist).to_array(),
        vertical_fov_degrees: 1.0 / (from_mag + (to_mag - from_mag) * t),
    }
}
