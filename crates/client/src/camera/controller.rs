use game_core::math::{gun_direction, horizontal_forward, wrap_angle};
use glam::Vec3;
use renderer_api::Camera;

use super::{
    BattleCameraEnvironment, BattleCameraInput, BattleCameraMode, BattleCameraSettings,
    CameraSubject,
};

const MIN_SNIPER_FOV_DEGREES: f32 = 3.0;
const MAX_SNIPER_FOV_DEGREES: f32 = 18.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BattleCameraController {
    settings: BattleCameraSettings,
    mode: BattleCameraMode,
    orbit_yaw_rad: f32,
    pitch_rad: f32,
    distance_m: f32,
    sniper_fov_degrees: f32,
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
        if self.mode == BattleCameraMode::Sniper {
            self.sniper_fov_degrees = (self.sniper_fov_degrees + input.zoom_delta_m)
                .clamp(MIN_SNIPER_FOV_DEGREES, MAX_SNIPER_FOV_DEGREES);
        } else {
            self.distance_m = (self.distance_m + input.zoom_delta_m)
                .clamp(self.settings.min_distance_m, self.settings.max_distance_m);
        }
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
        let tank = subject.position_vec();
        let yaw = subject.view_yaw_rad;
        let forward = horizontal_forward(yaw);
        let right = horizontal_right(forward);
        let target = tank
            + Vec3::Y * self.settings.third_person_target_height_m
            + forward * self.settings.third_person_target_forward_offset_m;
        let horizontal_distance = self.distance_m * self.pitch_rad.cos().max(0.1);
        let vertical_offset = self.distance_m * self.pitch_rad.sin();
        let desired_eye = target - forward * horizontal_distance
            + right * self.settings.third_person_lateral_offset_m
            + Vec3::Y * vertical_offset;
        let collided_eye = self.resolve_boom_collision(target, desired_eye, environment);
        let eye = self.apply_terrain_clearance(collided_eye, environment);

        Camera {
            eye: eye.to_array(),
            target: target.to_array(),
            vertical_fov_degrees: self.settings.third_person_fov_degrees,
        }
    }

    fn sniper_camera(
        &self,
        subject: &CameraSubject,
        environment: &BattleCameraEnvironment<'_>,
    ) -> Camera {
        let eye_yaw = subject.hull_yaw_rad + subject.turret_yaw_rad;
        let forward = horizontal_forward(eye_yaw);
        let eye = subject.position_vec()
            + Vec3::Y * self.settings.sniper_eye_height_m
            + forward * self.settings.sniper_forward_offset_m;
        let eye = self.apply_terrain_clearance(eye, environment);
        let aim = gun_direction(subject.desired_yaw_rad, subject.desired_pitch_rad);
        let target = eye + aim * 1_000.0;

        Camera {
            eye: eye.to_array(),
            target: target.to_array(),
            vertical_fov_degrees: self.sniper_fov_degrees,
        }
    }

    fn resolve_boom_collision(
        &self,
        target: Vec3,
        desired_eye: Vec3,
        environment: &BattleCameraEnvironment<'_>,
    ) -> Vec3 {
        let segment = desired_eye - target;
        let length = segment.length();
        if length <= f32::EPSILON {
            return desired_eye;
        }

        let direction = segment / length;
        let mut previous = target;
        for step in 1..=32 {
            let t = step as f32 / 32.0;
            let point = target + segment * t;
            if environment.obstacles.iter().any(|obstacle| obstacle.contains_point(point)) {
                return previous - direction * self.settings.obstacle_clearance_m;
            }
            previous = point;
        }
        desired_eye
    }

    fn apply_terrain_clearance(
        &self,
        mut eye: Vec3,
        environment: &BattleCameraEnvironment<'_>,
    ) -> Vec3 {
        if let Some(terrain) = environment.terrain
            && let Some(height) = terrain.sample_height(eye.x, eye.z)
        {
            eye.y = eye.y.max(height + self.settings.terrain_clearance_m);
        }
        eye
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
