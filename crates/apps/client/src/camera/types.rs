use std::borrow::Cow;

use glam::Vec3;
use net::TankSnapshot;
use terrain::{BattlefieldMap, HeightMap, StaticCoverObject};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattleCameraMode {
    ThirdPerson,
    Sniper,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BattleCameraSettings {
    pub third_person_distance_m: f32,
    pub third_person_target_height_m: f32,
    pub third_person_target_forward_offset_m: f32,
    pub third_person_lateral_offset_m: f32,
    pub third_person_pitch_rad: f32,
    pub third_person_fov_degrees: f32,
    pub sniper_fov_degrees: f32,
    pub terrain_clearance_m: f32,
    pub obstacle_clearance_m: f32,
    pub min_pitch_rad: f32,
    pub max_pitch_rad: f32,
    pub min_distance_m: f32,
    pub max_distance_m: f32,
}

impl Default for BattleCameraSettings {
    fn default() -> Self {
        Self {
            third_person_distance_m: 12.0,
            third_person_target_height_m: 2.75,
            third_person_target_forward_offset_m: 4.0,
            third_person_lateral_offset_m: 1.35,
            third_person_pitch_rad: 0.42,
            third_person_fov_degrees: 62.0,
            sniper_fov_degrees: 8.0,
            terrain_clearance_m: 0.45,
            obstacle_clearance_m: 0.35,
            min_pitch_rad: -0.25,
            max_pitch_rad: 0.9,
            min_distance_m: 3.0,
            max_distance_m: 18.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BattleCameraInput {
    pub orbit_yaw_delta_rad: f32,
    pub pitch_delta_rad: f32,
    pub zoom_delta_m: f32,
}

impl Default for BattleCameraInput {
    fn default() -> Self {
        Self { orbit_yaw_delta_rad: 0.0, pitch_delta_rad: 0.0, zoom_delta_m: 0.0 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraSubject {
    pub position: [f32; 3],
    /// Vehicle kind: the sniper camera reads the per-vehicle sight mount from it.
    pub vehicle: game_core::VehicleKind,
    pub hull_yaw_rad: f32,
    /// Authoritative hull pitch/roll (protocol v14): the sniper eye rides the full attitude.
    pub hull_pitch_rad: f32,
    pub hull_roll_rad: f32,
    pub turret_yaw_rad: f32,
    pub gun_pitch_rad: f32,
    pub view_yaw_rad: f32,
    pub desired_yaw_rad: f32,
    pub desired_pitch_rad: f32,
}

impl CameraSubject {
    pub fn from_snapshot(snapshot: TankSnapshot, gun_pitch_rad: f32) -> Self {
        let desired_yaw_rad = snapshot.yaw_rad + snapshot.turret_yaw_rad;
        Self {
            position: snapshot.position,
            vehicle: snapshot.vehicle,
            hull_yaw_rad: snapshot.yaw_rad,
            hull_pitch_rad: snapshot.hull_pitch_rad,
            hull_roll_rad: snapshot.hull_roll_rad,
            turret_yaw_rad: snapshot.turret_yaw_rad,
            gun_pitch_rad,
            view_yaw_rad: desired_yaw_rad,
            desired_yaw_rad,
            desired_pitch_rad: gun_pitch_rad,
        }
    }

    pub fn with_view_yaw(mut self, yaw_rad: f32) -> Self {
        self.view_yaw_rad = yaw_rad;
        self
    }

    pub fn with_desired_aim(mut self, yaw_rad: f32, pitch_rad: f32) -> Self {
        self.desired_yaw_rad = yaw_rad;
        self.desired_pitch_rad = pitch_rad;
        self
    }

    pub(crate) fn position_vec(self) -> Vec3 {
        Vec3::from_array(self.position)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CameraObstacle {
    pub label: String,
    pub center: [f32; 3],
    pub half_extents: [f32; 3],
}

impl CameraObstacle {
    pub fn aabb(label: impl Into<String>, center: [f32; 3], half_extents: [f32; 3]) -> Self {
        Self { label: label.into(), center, half_extents }
    }

    /// Fraction `t` in `[0, 1]` where the segment `from -> to` first enters this box, or `None`
    /// on a miss. An exact slab test: boom-length point sampling stepped clean over obstacles
    /// thinner than one sample stride.
    pub(crate) fn segment_entry(&self, from: Vec3, to: Vec3) -> Option<f32> {
        let center = Vec3::from_array(self.center);
        let half = Vec3::from_array(self.half_extents);
        let direction = to - from;
        let mut t_enter = 0.0_f32;
        let mut t_exit = 1.0_f32;
        for axis in 0..3 {
            let origin = from[axis] - center[axis];
            let speed = direction[axis];
            if speed.abs() <= f32::EPSILON {
                if origin.abs() > half[axis] {
                    return None;
                }
                continue;
            }
            let t_low = (-half[axis] - origin) / speed;
            let t_high = (half[axis] - origin) / speed;
            let (near, far) = if t_low <= t_high { (t_low, t_high) } else { (t_high, t_low) };
            t_enter = t_enter.max(near);
            t_exit = t_exit.min(far);
            if t_enter > t_exit {
                return None;
            }
        }
        Some(t_enter)
    }
}

#[derive(Debug, Clone)]
pub struct BattleCameraEnvironment<'a> {
    pub terrain: Option<&'a HeightMap>,
    pub obstacles: Cow<'a, [CameraObstacle]>,
}

impl<'a> BattleCameraEnvironment<'a> {
    pub fn empty() -> Self {
        Self { terrain: None, obstacles: Cow::Borrowed(&[]) }
    }

    pub fn with_terrain(terrain: &'a HeightMap) -> Self {
        Self { terrain: Some(terrain), obstacles: Cow::Borrowed(&[]) }
    }

    pub fn with_obstacles(terrain: &'a HeightMap, obstacles: &'a [CameraObstacle]) -> Self {
        Self { terrain: Some(terrain), obstacles: Cow::Borrowed(obstacles) }
    }

    pub fn for_battlefield(battlefield: &'a BattlefieldMap) -> Self {
        let mut environment = Self::with_terrain(&battlefield.heightmap);
        for cover in &battlefield.static_cover {
            environment.add_obstacle(CameraObstacle::from_static_cover(cover));
        }
        environment
    }

    pub fn add_obstacle(&mut self, obstacle: CameraObstacle) {
        self.obstacles.to_mut().push(obstacle);
    }
}

impl CameraObstacle {
    pub fn from_static_cover(cover: &StaticCoverObject) -> Self {
        Self::aabb(&cover.name, cover.center, cover.half_extents_m)
    }
}
