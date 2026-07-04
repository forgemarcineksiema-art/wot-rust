use game_core::MountFrames;
use game_core::math::{gun_direction, horizontal_forward, wrap_angle};
use glam::{Mat3, Vec3};
use renderer_api::Camera;

use super::smoothing::CameraSmoothing;
use super::{
    BattleCameraEnvironment, BattleCameraInput, BattleCameraMode, BattleCameraSettings,
    CameraSubject, collision, zoom,
};

/// Sniper sight height above the gun trunnion — roughly where the gunner's optics sit.
const SNIPER_SIGHT_ABOVE_TRUNNION_M: f32 = 0.35;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BattleCameraController {
    settings: BattleCameraSettings,
    mode: BattleCameraMode,
    orbit_yaw_rad: f32,
    pitch_rad: f32,
    distance_m: f32,
    sniper_fov_degrees: f32,
    pub(super) smoothing: CameraSmoothing,
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
            smoothing: CameraSmoothing::default(),
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
        // The over-shoulder offset shifts the *whole* sight lane (target and eye alike): with the
        // offset on the eye only, the eye->target direction rotated as the boom length changed,
        // so every zoom click swung the scene sideways and dragged the aim point with it.
        let target = tank
            + Vec3::Y * self.settings.third_person_target_height_m
            + forward * self.settings.third_person_target_forward_offset_m
            + right * self.settings.third_person_lateral_offset_m;
        let horizontal_distance = self.distance_m * self.pitch_rad.cos().max(0.1);
        let vertical_offset = self.distance_m * self.pitch_rad.sin();
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

    fn sniper_camera(
        &self,
        subject: &CameraSubject,
        environment: &BattleCameraEnvironment<'_>,
    ) -> Camera {
        // The eye sits *on* the per-vehicle turret-ring axis at sight height: a point on the
        // traverse axis does not translate as the turret slews, so the world cannot slide
        // sideways while the turret catches up to the aim (a forward-offset eye rode a lateral
        // arc on every mouse move). Height comes from the vehicle's trunnion, not a global.
        let mounts = MountFrames::for_vehicle(subject.vehicle);
        let ring = mounts.turret_ring.translation;
        let ring_axis =
            Mat3::from_rotation_y(subject.hull_yaw_rad) * Vec3::new(ring.x, 0.0, ring.z);
        let sight_height = mounts.gun_trunnion.translation.y + SNIPER_SIGHT_ABOVE_TRUNNION_M;
        let eye = subject.position_vec() + ring_axis + Vec3::Y * sight_height;
        let eye =
            collision::apply_terrain_clearance(eye, environment, self.settings.terrain_clearance_m);
        let aim = gun_direction(subject.desired_yaw_rad, subject.desired_pitch_rad);
        let target = eye + aim * 1_000.0;

        Camera {
            eye: eye.to_array(),
            target: target.to_array(),
            vertical_fov_degrees: self.sniper_fov_degrees,
        }
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
