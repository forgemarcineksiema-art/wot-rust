use game_core::math::{horizontal_forward, wrap_angle};
use glam::Vec3;
use renderer_api::Camera;

use super::smoothing::CameraSmoothing;
use super::{
    BattleCameraEnvironment, BattleCameraInput, BattleCameraMode, BattleCameraSettings,
    CameraSubject, collision, zoom,
};

/// The death spectate (Inna Liga D9): the boom flows out to this multiple of the map's max...
const DEATH_BOOM_FACTOR: f32 = 1.3;
/// ...over this many seconds, while the view drifts in a slow orbit around the wreck.
const DEATH_BOOM_FLOW_S: f32 = 2.0;
const DEATH_ORBIT_RATE_RAD_S: f32 = 0.12;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BattleCameraController {
    settings: BattleCameraSettings,
    mode: BattleCameraMode,
    orbit_yaw_rad: f32,
    pitch_rad: f32,
    distance_m: f32,
    sniper_fov_degrees: f32,
    /// Seconds since the player's death took the screen, or `None` alive (D9). While set, the
    /// boom flows wide and the view orbits — the player WATCHES the wreck epilogue instead of
    /// staring through a frozen gunsight.
    pub(super) death_orbit_s: Option<f32>,
    pub(super) smoothing: CameraSmoothing,
    pub(super) presented: super::present::PresentedCamera,
}

impl BattleCameraController {
    pub fn new(settings: BattleCameraSettings) -> Self {
        Self {
            settings,
            mode: BattleCameraMode::ThirdPerson,
            orbit_yaw_rad: 0.0,
            pitch_rad: settings.third_person_pitch_rad,
            distance_m: settings.third_person_distance_m,
            sniper_fov_degrees: settings.sniper_fov_degrees,
            death_orbit_s: None,
            smoothing: CameraSmoothing::default(),
            presented: super::present::PresentedCamera::default(),
        }
    }

    /// Enter or leave the death spectate (D9). Entering leaves the scope (a dead gunner sights
    /// nothing) and lifts the eye to a watching pitch; leaving hands the rig back untouched.
    pub fn set_death_orbit(&mut self, dead: bool) {
        match (dead, self.death_orbit_s) {
            (true, None) => {
                self.death_orbit_s = Some(0.0);
                self.mode = BattleCameraMode::ThirdPerson;
                self.pitch_rad = self
                    .pitch_rad
                    .max(0.30)
                    .clamp(self.settings.min_pitch_rad, self.settings.max_pitch_rad);
            }
            (false, Some(_)) => self.death_orbit_s = None,
            _ => {}
        }
    }

    pub fn death_spectate(&self) -> bool {
        self.death_orbit_s.is_some()
    }

    /// Advance the death orbit one presented frame: the age drives the boom flow and the view
    /// drifts slowly around the wreck. Called from `present` with the frame dt.
    pub(super) fn tick_death_orbit(&mut self, dt: f32) {
        if let Some(age) = &mut self.death_orbit_s {
            *age += dt;
            self.orbit_yaw_rad = wrap_angle(self.orbit_yaw_rad + DEATH_ORBIT_RATE_RAD_S * dt);
        }
    }

    /// The boom the rig actually flies: the dialed distance alive, flowing out to 1.3x the
    /// map's ceiling over ~2 s once dead — far enough to take in the whole wreck epilogue.
    fn effective_boom_m(&self) -> f32 {
        match self.death_orbit_s {
            None => self.distance_m,
            Some(age) => {
                let t = (age / DEATH_BOOM_FLOW_S).clamp(0.0, 1.0);
                let wide = self.settings.max_distance_m * DEATH_BOOM_FACTOR;
                self.distance_m + (wide - self.distance_m) * t
            }
        }
    }

    pub fn settings(&self) -> BattleCameraSettings {
        self.settings
    }

    pub fn mode(&self) -> BattleCameraMode {
        self.mode
    }

    pub fn pitch_rad(&self) -> f32 {
        self.pitch_rad
    }

    pub fn distance_m(&self) -> f32 {
        self.distance_m
    }

    pub fn orbit_yaw_rad(&self) -> f32 {
        self.orbit_yaw_rad
    }

    pub fn sniper_fov_degrees(&self) -> f32 {
        self.sniper_fov_degrees
    }

    pub fn set_orbit_yaw(&mut self, yaw: f32) {
        self.orbit_yaw_rad = yaw;
    }

    pub fn set_pitch(&mut self, pitch_rad: f32) {
        self.pitch_rad = pitch_rad.clamp(self.settings.min_pitch_rad, self.settings.max_pitch_rad);
    }

    pub fn set_mode(&mut self, mode: BattleCameraMode) {
        self.mode = mode;
    }

    /// Reset the sniper magnification to the default entry step. Key entry (`V`, `Shift`) always
    /// opens here so the zoom is predictable for muscle memory; the wheel then dials from it. The
    /// wheel's own third-person->sniper handover keeps its widest-step entry (see `zoom`).
    pub fn reset_sniper_zoom(&mut self) {
        self.sniper_fov_degrees = self.settings.sniper_fov_degrees;
    }

    /// Mouse-look scale that keeps on-screen cursor speed roughly constant across views: 1.0 in
    /// third person, the FOV ratio in sniper (a 3 degree view must turn ~20x slower than the
    /// wide boom view or maximum zoom is uncontrollable).
    pub fn look_sensitivity_scale(&self) -> f32 {
        match self.mode {
            BattleCameraMode::ThirdPerson => 1.0,
            BattleCameraMode::Sniper => {
                self.sniper_fov_degrees / self.settings.third_person_fov_degrees
            }
        }
    }

    pub fn apply_input(&mut self, input: BattleCameraInput) {
        // Wrap into (-PI, PI] so a long session cannot grow the angle without bound
        // and degrade sin/cos precision (the sim cares about f32 stability).
        self.orbit_yaw_rad = wrap_angle(self.orbit_yaw_rad + input.orbit_yaw_delta_rad);
        self.pitch_rad = (self.pitch_rad + input.pitch_delta_rad)
            .clamp(self.settings.min_pitch_rad, self.settings.max_pitch_rad);
        let zoomed = zoom::apply_zoom(
            zoom::ZoomState {
                mode: self.mode,
                distance_m: self.distance_m,
                sniper_fov_degrees: self.sniper_fov_degrees,
            },
            input.zoom_delta_m,
            self.settings.min_distance_m,
            self.settings.max_distance_m,
        );
        self.mode = zoomed.mode;
        self.distance_m = zoomed.distance_m;
        self.sniper_fov_degrees = zoomed.sniper_fov_degrees;
    }

    pub fn render_camera(
        &self,
        subject: &CameraSubject,
        environment: &BattleCameraEnvironment<'_>,
    ) -> Camera {
        match self.mode {
            BattleCameraMode::ThirdPerson => self.third_person_camera(subject, environment),
            BattleCameraMode::Sniper => self.sniper_camera(subject, environment),
        }
    }

    fn third_person_camera(
        &self,
        subject: &CameraSubject,
        environment: &BattleCameraEnvironment<'_>,
    ) -> Camera {
        // The rig follows the SMOOTHED anchor, not the raw hull: heightmap steps and hard stops
        // reach the eye as an eased settle instead of a 1:1 jolt.
        let tank = self.smoothing.anchor.unwrap_or(subject.position_vec());
        let yaw = subject.view_yaw_rad;
        let forward = horizontal_forward(yaw);
        let right = horizontal_right(forward);
        // Any lateral offset shifts the *whole* sight lane (target and eye alike): an eye-only
        // offset — or a zoom-dependent one — rotates the view with the boom length and drags
        // the scene sideways on every scroll. The default is CENTERED (offset 0).
        let target = tank
            + Vec3::Y * self.settings.third_person_target_height_m
            + forward * self.settings.third_person_target_forward_offset_m
            + right * self.settings.third_person_lateral_offset_m;
        let boom = self.effective_boom_m();
        let horizontal_distance = boom * self.pitch_rad.cos().max(0.1);
        let vertical_offset = boom * self.pitch_rad.sin();
        let desired_eye = target - forward * horizontal_distance + Vec3::Y * vertical_offset;
        let collided_eye = collision::resolve_boom_collision(
            target,
            desired_eye,
            environment,
            self.settings.obstacle_clearance_m,
            self.settings.terrain_clearance_m,
        );
        let eye = collision::apply_terrain_clearance(
            collided_eye,
            environment,
            self.settings.terrain_clearance_m,
        );

        Camera {
            eye: eye.to_array(),
            target: target.to_array(),
            vertical_fov_degrees: self.settings.third_person_fov_degrees
                + self.smoothing.fov_boost_deg,
        }
    }

    /// Relative magnification of the current sniper step against the default battle view, for
    /// the HUD's zoom readout; `None` in third person.
    pub fn zoom_factor(&self) -> Option<f32> {
        (self.mode == BattleCameraMode::Sniper)
            .then(|| self.settings.third_person_fov_degrees / self.sniper_fov_degrees.max(0.1))
    }
}

impl Default for BattleCameraController {
    fn default() -> Self {
        Self::new(BattleCameraSettings::default())
    }
}

fn horizontal_right(forward: Vec3) -> Vec3 {
    Vec3::new(forward.z, 0.0, -forward.x)
}
