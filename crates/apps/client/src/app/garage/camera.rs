//! The garage inspection camera: a turntable orbit the player drags, plus three feel touches —
//! close inspection (the boom pulls in to ~4 m for running-gear detail), spring-eased module focus
//! (clicking a loadout slot flies the camera to a framing of that module), and a slow idle
//! auto-orbit that drifts the view when the player stops touching it. Manual drag or zoom always
//! wins: it cancels any focus/return and resets the idle clock.

use glam::Vec3;
use renderer_api::Camera;

use super::draft::FitSlot;
use super::{GarageState, HERO_ORBIT_DISTANCE, HERO_ORBIT_PITCH, HERO_ORBIT_YAW};
use crate::scene::hangar::{hangar_camera_pivot, hangar_interior};

const MIN_PITCH: f32 = -0.05;
const MAX_PITCH: f32 = 1.20;
/// Closest boom: pulled in from the old 8.5 m so the player can inspect the running gear.
const MIN_DISTANCE: f32 = 4.0;
const MAX_DISTANCE: f32 = 20.0;
/// Keep the eye at least this far off any wall/roof so the camera never clips through the shell.
const ROOM_MARGIN: f32 = 0.6;
const ORBIT_SENSITIVITY: f32 = 0.005;
const ZOOM_STEP_M: f32 = 1.2;
/// Exponential ease rate of the focus/return spring (per second).
const SPRING_RATE: f32 = 6.0;
/// The camera is "arrived" (spring released) when within these of the target.
const ARRIVE_ANGLE: f32 = 0.01;
const ARRIVE_DIST: f32 = 0.05;
/// Idle seconds before the auto-orbit drift begins, and its yaw speed.
const AUTO_ORBIT_IDLE_S: f32 = 10.0;
const AUTO_ORBIT_SPEED: f32 = 0.08;

/// A framing the camera eases toward: orbit angles, boom length, and a look-point offset off the
/// turntable centre (to lift the pivot onto the turret or drop it to the running gear).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct CameraTarget {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub pivot_offset: Vec3,
}

impl CameraTarget {
    fn hero() -> Self {
        Self {
            yaw: HERO_ORBIT_YAW,
            pitch: HERO_ORBIT_PITCH,
            distance: HERO_ORBIT_DISTANCE,
            pivot_offset: Vec3::ZERO,
        }
    }

    /// The framing that best shows a given fitting slot: the suspension wants a low side pass over
    /// the wheels, the gun a near-level look down the barrel, the engine deck a rear-high angle,
    /// and so on. Distances stay within the boom clamp.
    fn for_slot(slot: FitSlot) -> Self {
        match slot {
            FitSlot::Turret => Self {
                yaw: 0.7,
                pitch: 0.55,
                distance: 5.5,
                pivot_offset: Vec3::new(0.0, 0.7, 0.0),
            },
            FitSlot::Gun => Self {
                yaw: 0.18,
                pitch: 0.12,
                distance: 6.5,
                pivot_offset: Vec3::new(0.0, 0.25, 1.6),
            },
            FitSlot::Hull => Self {
                yaw: 0.55,
                pitch: 0.14,
                distance: 6.0,
                pivot_offset: Vec3::new(0.0, -0.1, 0.4),
            },
            FitSlot::Engine => Self {
                yaw: 3.35,
                pitch: 0.42,
                distance: 6.0,
                pivot_offset: Vec3::new(0.0, 0.2, -1.6),
            },
            FitSlot::Suspension => Self {
                yaw: 1.55,
                pitch: -0.03,
                distance: 4.5,
                pivot_offset: Vec3::new(0.0, -0.55, 0.0),
            },
            FitSlot::Radio => Self::hero(),
        }
    }
}

/// The unit eye direction (pivot -> eye) for the given orbit angles.
fn orbit_direction(yaw: f32, pitch: f32) -> Vec3 {
    Vec3::new(pitch.cos() * yaw.sin(), pitch.sin(), pitch.cos() * yaw.cos())
}

/// The largest boom length that keeps the eye inside the hangar shell (with `ROOM_MARGIN`) looking
/// from `pivot` along `dir` — this is what stops a zoom-out from clipping through the walls or roof.
fn room_limited_distance(pivot: Vec3, dir: Vec3, desired: f32) -> f32 {
    let (half, height) = hangar_interior();
    let m = ROOM_MARGIN;
    // Per axis: `(pos, dir, lo, hi)`. The boundary `+dir` reaches is `hi`, `-dir` reaches `lo`.
    let slabs = [
        (pivot.x, dir.x, -(half - m), half - m),
        (pivot.z, dir.z, -(half - m), half - m),
        (pivot.y, dir.y, m, height - m),
    ];
    let mut limit = desired;
    for (p, d, lo, hi) in slabs {
        if d.abs() >= 1.0e-4 {
            limit = limit.min((if d > 0.0 { hi } else { lo } - p) / d);
        }
    }
    limit.max(MIN_DISTANCE)
}

/// Shortest signed difference `to - from` for an angle, so the yaw spring takes the near way round.
fn angle_delta(from: f32, to: f32) -> f32 {
    let mut d = to - from;
    while d > std::f32::consts::PI {
        d -= std::f32::consts::TAU;
    }
    while d < -std::f32::consts::PI {
        d += std::f32::consts::TAU;
    }
    d
}

impl GarageState {
    pub(in crate::app) fn orbit_camera(&self) -> Camera {
        let pivot = hangar_camera_pivot() + self.pivot_offset;
        let dir = orbit_direction(self.orbit_yaw, self.orbit_pitch);
        let distance = room_limited_distance(pivot, dir, self.orbit_distance);
        let eye = pivot + dir * distance;
        Camera { eye: eye.to_array(), target: pivot.to_array(), vertical_fov_degrees: 32.0 }
    }

    pub(in crate::app) fn apply_drag(&mut self, dx: f32, dy: f32) {
        if !self.dragging {
            return;
        }
        // Manual drag overrides any focus/return spring and wakes the idle clock.
        self.camera_target = None;
        self.idle_seconds = 0.0;
        self.orbit_yaw += dx * ORBIT_SENSITIVITY;
        self.orbit_pitch = (self.orbit_pitch - dy * ORBIT_SENSITIVITY).clamp(MIN_PITCH, MAX_PITCH);
        // Tilting to the horizon shrinks the room's reach — pull the boom in so the eye stays inside.
        self.orbit_distance = self.room_limited_orbit(self.orbit_distance);
    }

    pub(in crate::app) fn apply_zoom(&mut self, notches: f32) {
        self.camera_target = None;
        self.idle_seconds = 0.0;
        let desired =
            (self.orbit_distance - notches * ZOOM_STEP_M).clamp(MIN_DISTANCE, MAX_DISTANCE);
        // Zoom out only as far as the room allows — no distance banked "outside" the wall.
        self.orbit_distance = self.room_limited_orbit(desired);
    }

    /// The desired boom length, capped so the eye stays inside the hangar at the current framing.
    fn room_limited_orbit(&self, desired: f32) -> f32 {
        let pivot = hangar_camera_pivot() + self.pivot_offset;
        room_limited_distance(pivot, orbit_direction(self.orbit_yaw, self.orbit_pitch), desired)
    }

    /// Fly the camera to frame a fitting slot's module.
    pub(in crate::app) fn focus_module(&mut self, slot: FitSlot) {
        self.camera_target = Some(CameraTarget::for_slot(slot));
        self.idle_seconds = 0.0;
    }

    /// Ease the camera back to the default hero framing (Escape / clicking empty scene).
    pub(in crate::app) fn return_to_hero_view(&mut self) {
        self.camera_target = Some(CameraTarget::hero());
        self.idle_seconds = 0.0;
    }

    /// Snap (no spring) to the hero framing — used when switching vehicles, which already resets a
    /// lot of state; the instant cut reads as "new tank, fresh look".
    pub(super) fn snap_to_hero_view(&mut self) {
        self.orbit_yaw = HERO_ORBIT_YAW;
        self.orbit_pitch = HERO_ORBIT_PITCH;
        self.orbit_distance = HERO_ORBIT_DISTANCE;
        self.pivot_offset = Vec3::ZERO;
        self.camera_target = None;
        self.idle_seconds = 0.0;
    }

    /// Whether the camera is away from the resting hero framing — an active focus/return spring,
    /// a lifted look point, or a manually orbited view. Escape uses this to back out before close.
    pub(in crate::app) fn is_camera_off_hero(&self) -> bool {
        self.camera_target.is_some()
            || self.pivot_offset.length() > 0.02
            || angle_delta(self.orbit_yaw, HERO_ORBIT_YAW).abs() > 0.05
            || (self.orbit_distance - HERO_ORBIT_DISTANCE).abs() > 0.2
            || (self.orbit_pitch - HERO_ORBIT_PITCH).abs() > 0.05
    }

    /// Advance the camera one frame: run the focus/return spring, or drift the idle auto-orbit.
    pub(in crate::app) fn tick_camera(&mut self, dt: f32) {
        self.idle_seconds += dt;
        if let Some(target) = self.camera_target {
            let k = (SPRING_RATE * dt).clamp(0.0, 1.0);
            self.orbit_yaw += angle_delta(self.orbit_yaw, target.yaw) * k;
            self.orbit_pitch += (target.pitch - self.orbit_pitch) * k;
            self.orbit_distance += (target.distance - self.orbit_distance) * k;
            self.pivot_offset += (target.pivot_offset - self.pivot_offset) * k;
            let arrived = angle_delta(self.orbit_yaw, target.yaw).abs() < ARRIVE_ANGLE
                && (self.orbit_pitch - target.pitch).abs() < ARRIVE_ANGLE
                && (self.orbit_distance - target.distance).abs() < ARRIVE_DIST
                && self.pivot_offset.distance(target.pivot_offset) < ARRIVE_DIST;
            if arrived {
                self.camera_target = None;
            }
        } else if !self.dragging && self.idle_seconds > AUTO_ORBIT_IDLE_S {
            self.orbit_yaw += AUTO_ORBIT_SPEED * dt;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spun(mut garage: GarageState, seconds: f32) -> GarageState {
        // 60 Hz worth of ticks to let a spring settle or the drift accumulate.
        for _ in 0..(seconds * 60.0) as u32 {
            garage.tick_camera(1.0 / 60.0);
        }
        garage
    }

    #[test]
    fn focusing_the_suspension_flies_the_boom_in_low_over_the_running_gear() {
        let mut garage = GarageState::default();
        garage.focus_module(FitSlot::Suspension);
        let garage = spun(garage, 2.0);

        let want = CameraTarget::for_slot(FitSlot::Suspension);
        assert!((garage.orbit_distance - want.distance).abs() < 0.1, "boom pulls in close");
        assert!(garage.orbit_pitch < 0.05, "pitch drops to a low side pass");
        assert!(garage.pivot_offset.y < -0.3, "the look point drops to the wheels");
        assert!(garage.camera_target.is_none(), "the spring releases once arrived");
    }

    #[test]
    fn zooming_out_never_pushes_the_eye_through_the_hangar_shell() {
        let (half, height) = hangar_interior();
        // Across the full orbit — every yaw, from a near-horizontal to a steep pitch — scrolling all
        // the way out must leave the eye inside the room, never clipping a wall or the roof.
        for yaw_step in 0..12 {
            let yaw = yaw_step as f32 / 12.0 * std::f32::consts::TAU;
            for &pitch in &[MIN_PITCH, 0.1, HERO_ORBIT_PITCH, 0.8, MAX_PITCH] {
                let mut garage =
                    GarageState { orbit_yaw: yaw, orbit_pitch: pitch, ..GarageState::default() };
                for _ in 0..40 {
                    garage.apply_zoom(-1.0); // scroll out to the limit
                }
                let [x, y, z] = garage.orbit_camera().eye;
                assert!(x.abs() < half, "eye left the room on x (yaw {yaw}, pitch {pitch}): {x}");
                assert!(z.abs() < half, "eye left the room on z (yaw {yaw}, pitch {pitch}): {z}");
                assert!(
                    y > 0.0 && y < height,
                    "eye left the room on y (yaw {yaw}, pitch {pitch}): {y}"
                );
            }
        }
    }

    #[test]
    fn the_close_boom_reaches_nearer_than_the_old_limit() {
        let mut garage = GarageState::default();
        for _ in 0..40 {
            garage.apply_zoom(1.0); // zoom all the way in
        }
        assert!((garage.orbit_distance - MIN_DISTANCE).abs() < 1.0e-4);
        // Closer inspection than the previous 8.5 m floor.
        assert!(garage.orbit_distance < 8.5);
    }

    #[test]
    fn returning_to_hero_eases_back_to_the_default_framing() {
        let mut garage = GarageState::default();
        garage.focus_module(FitSlot::Engine);
        let mut garage = spun(garage, 2.0);
        assert!((garage.orbit_yaw - HERO_ORBIT_YAW).abs() > 0.5, "the engine focus moved the yaw");

        garage.return_to_hero_view();
        let garage = spun(garage, 2.0);
        assert!((garage.orbit_yaw - HERO_ORBIT_YAW).abs() < 0.02, "eased back to hero yaw");
        assert!(garage.pivot_offset.length() < 0.02, "the look point returns to centre");
    }

    #[test]
    fn the_camera_auto_orbits_only_after_idling() {
        let mut garage = GarageState::default();
        // Just under the idle threshold: no drift yet.
        let mut warm = spun(GarageState::default(), AUTO_ORBIT_IDLE_S - 1.0);
        let before = warm.orbit_yaw;
        warm.tick_camera(0.5);
        assert!((warm.orbit_yaw - before).abs() < 1.0e-4, "no drift before the idle threshold");

        // Past the threshold the yaw drifts.
        garage = spun(garage, AUTO_ORBIT_IDLE_S + 2.0);
        assert!(garage.orbit_yaw > HERO_ORBIT_YAW, "idle auto-orbit drifts the view");
    }

    #[test]
    fn a_manual_drag_cancels_the_spring_and_resets_the_idle_clock() {
        let mut garage = GarageState::default();
        garage.focus_module(FitSlot::Turret);
        garage.idle_seconds = 5.0;
        garage.begin_drag();

        garage.apply_drag(20.0, 0.0);

        assert!(garage.camera_target.is_none(), "drag wins over the focus spring");
        assert_eq!(garage.idle_seconds, 0.0, "drag wakes the idle clock");
    }
}
