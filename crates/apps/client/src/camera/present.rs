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
/// Fraction of the sprung hull's dynamic dive the presented TPP camera rides (Immersja B1).
/// Deliberately well under 1: the hull SHOWS the motion, the camera only corroborates it — a
/// camera that matched the hull would read as a bobblehead. No new state lives here: the
/// input is already spring-filtered upstream (`engine::attitude`) and already retires through
/// the predictor's freeze, so the presented camera stays a pure function of frozen-safe
/// inputs. The HEAVE half of B1 is retired: the follow anchor's own soft vertical spring
/// (smoothing.rs, omega 7) is the camera's suspension now — stacking a second, differently
/// phased vertical filter on top of it re-added the very bounce the anchor removes.
const SPRUNG_DIVE_FRAC: f32 = 0.35;
/// Defensive cap, independent of the upstream spring cap (0.035 rad): a runaway input may
/// nod the view, never throw it.
const SPRUNG_DIVE_CAP_RAD: f32 = 0.02;
/// Ride tremor (Immersja B2): the two beat rates of the vertical shiver (deliberately
/// inharmonic so it never reads as a tone), and the hard amplitude cap — a tremor shivers
/// the view, it never throws it. No RNG anywhere: the phase is a plain accumulator, so the
/// presented camera stays a deterministic function of its input sequence.
/// Both beats sit WELL below the 30 Hz Nyquist rate of a 60 FPS display. The second beat
/// shipped at 29.7 Hz — sampled barely twice per cycle, it rendered as an alternating
/// up/down per frame with a slow amplitude envelope (and aliased into wobble below 60 FPS):
/// the "camera vibrates on rough ground" complaint was mostly this, not the terrain. 8.3 Hz
/// keeps the pair inharmonic (no common tone with 11) and leaves ~7 samples per cycle.
const SHAKE_BEAT_A_HZ: f32 = 11.0;
const SHAKE_BEAT_B_HZ: f32 = 8.3;
const SHAKE_CAP_M: f32 = 0.05;
/// How fast the terrain-clearance lift lets go once the ground falls away (m/s). The lift
/// RISES instantly — the eye never renders from under the terrain — but the release used to
/// be instant too, and an eye-only drop rotates the whole view (the target stays on the
/// tank): a passed crest snapped the sight down in one frame. 3 m/s over a 12 m boom is a
/// gentle ~14 deg/s recovery.
const CLEARANCE_RELEASE_MPS: f32 = 3.0;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(super) struct PresentedCamera {
    last_mode: Option<BattleCameraMode>,
    last_camera: Option<Camera>,
    /// Live mode transition: the camera and mode we are blending FROM and the blend's age.
    blend: Option<(Camera, BattleCameraMode, f32)>,
    /// Smoothed third-person boom length (meters).
    boom_m: Option<f32>,
    /// Ride-tremor clock (Immersja B2): a plain accumulator, deterministic per frame sequence.
    shake_phase: f32,
    /// The live terrain-clearance lift on the presented eye (meters): raised instantly by the
    /// ground, released at [`CLEARANCE_RELEASE_MPS`] once the ground falls away.
    clearance_lift_m: f32,
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

        let mut camera;
        if self.mode() == BattleCameraMode::ThirdPerson {
            // The presented TPP starts from the UNCLAMPED pose and applies the terrain
            // clearance LAST, as its own state: the boom rescale and the feel shifts no longer
            // push a already-lifted eye back under the ground, and the lift can release
            // smoothly. The logical camera (aiming) keeps the hard clamp inside
            // `render_camera`, bit-identical to before.
            camera = self.third_person_camera_unclamped(subject, environment);
            camera = self.presented.smooth_boom(camera, dt);
            camera = ride_sprung_hull(camera, subject);
            camera = self.presented.ride_tremor(camera, subject.ride_shake_m, dt);
            camera = self.presented.settle_terrain_clearance(
                camera,
                environment,
                self.settings().terrain_clearance_m,
                dt,
            );
        } else {
            camera = self.render_camera(subject, environment);
            self.presented.boom_m = None;
            self.presented.clearance_lift_m = 0.0;
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
    /// The ride tremor (Immersja B2): what the suspension could not swallow shivers the
    /// whole rig vertically — two inharmonic beats, amplitude handed in by the client
    /// (roughness × speed, slope removed), hard-capped. The phase clock only advances
    /// while there IS a tremor, so a standing tank's presented camera stays bit-stable.
    fn ride_tremor(&mut self, camera: Camera, amplitude_m: f32, dt: f32) -> Camera {
        let amplitude = amplitude_m.clamp(0.0, SHAKE_CAP_M);
        if amplitude <= 0.0 {
            return camera;
        }
        self.shake_phase = (self.shake_phase + dt).rem_euclid(60.0);
        let t = self.shake_phase * std::f32::consts::TAU;
        let shiver = (t * SHAKE_BEAT_A_HZ).sin() * 0.65 + (t * SHAKE_BEAT_B_HZ + 1.3).sin() * 0.35;
        let lift = Vec3::Y * (amplitude * shiver);
        Camera {
            eye: (Vec3::from_array(camera.eye) + lift).to_array(),
            target: (Vec3::from_array(camera.target) + lift).to_array(),
            ..camera
        }
    }

    /// The presented eye-over-terrain clearance, applied LAST so it sees the eye every other
    /// stage produced. The lift itself is the state: `needed` (how far under `ground +
    /// clearance` the raw eye sits) raises it instantly — the eye never renders from under the
    /// terrain — and once the ground falls away it releases at a bounded rate instead of
    /// dropping (and rotating) the whole view in one frame. Plain driving is untouched: with
    /// the eye clear of the ground `needed` is 0 and the lift decays to 0.
    fn settle_terrain_clearance(
        &mut self,
        camera: Camera,
        environment: &BattleCameraEnvironment<'_>,
        clearance_m: f32,
        dt: f32,
    ) -> Camera {
        let eye = Vec3::from_array(camera.eye);
        let needed = environment
            .terrain
            .and_then(|terrain| terrain.sample_height(eye.x, eye.z))
            .map_or(0.0, |ground| (ground + clearance_m - eye.y).max(0.0));
        let released = self.clearance_lift_m - CLEARANCE_RELEASE_MPS * dt;
        self.clearance_lift_m = needed.max(released).max(0.0);
        if self.clearance_lift_m <= 0.0 {
            return camera;
        }
        Camera { eye: (eye + Vec3::Y * self.clearance_lift_m).to_array(), ..camera }
    }

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

/// The presented TPP rig rides a fraction of the sprung hull's dynamic dive (Immersja B1):
/// braking dips the nose of the VIEW the way it dips the nose of the TANK, tilting about the
/// camera's right axis. Presented layer only: the logical camera (aiming) and the sniper
/// never pass through here. Vertical ride lives in the follow anchor's own soft spring, not
/// here (see `SPRUNG_DIVE_FRAC` for why the heave channel retired).
fn ride_sprung_hull(camera: Camera, subject: &CameraSubject) -> Camera {
    let dive = (subject.sprung_dive_rad * SPRUNG_DIVE_FRAC)
        .clamp(-SPRUNG_DIVE_CAP_RAD, SPRUNG_DIVE_CAP_RAD);
    if dive == 0.0 {
        return camera;
    }
    let eye = Vec3::from_array(camera.eye);
    let target = Vec3::from_array(camera.target);
    let dir = target - eye;
    let right = dir.cross(Vec3::Y).normalize_or_zero();
    if right == Vec3::ZERO {
        // Looking straight along the vertical axis: no stable right axis to tilt about.
        return camera;
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
