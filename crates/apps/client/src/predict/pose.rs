//! The predictor's render-facing pose: the blend between the previous and current fixed tick
//! that lets a 60 Hz sim render without judder under a faster or phase-drifting present clock.
//! Split from `predict.rs` to stay within the reviewability budget.

use game_core::math::lerp_angle;
use glam::Vec3;

use super::LocalPredictor;

/// A render-interpolatable snapshot of the predicted hull/turret pose. The visual tank and
/// the camera blend the previous tick's pose toward the current one so a 60 Hz sim renders
/// smoothly under a faster (or merely phase-drifting) vsync present clock.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PredictedPose {
    pub position: Vec3,
    pub yaw_rad: f32,
    /// Predicted authoritative hull pitch (+nose up) — the render and the aim math tilt with it.
    pub hull_pitch_rad: f32,
    /// Predicted authoritative hull roll (+right side up).
    pub hull_roll_rad: f32,
    pub turret_yaw_rad: f32,
    pub gun_pitch_rad: f32,
}

impl LocalPredictor {
    /// The pose at the end of the most recently simulated tick.
    pub(super) fn current_pose(&self) -> PredictedPose {
        PredictedPose {
            position: self.drive.kinematic.position,
            yaw_rad: self.drive.kinematic.yaw_rad,
            hull_pitch_rad: self.drive.kinematic.pitch_rad,
            hull_roll_rad: self.drive.kinematic.roll_rad,
            turret_yaw_rad: self.drive.aiming.turret_yaw_rad,
            gun_pitch_rad: self.drive.aiming.gun_pitch_rad,
        }
    }

    /// Render pose blended `alpha` of the way from the previous tick to the current one.
    /// `alpha` is the fixed-tick accumulator remainder in `[0, 1]`; this is what lets a
    /// 60 Hz sim render without judder under a faster or phase-drifting present clock.
    pub fn interpolated_pose(&self, alpha: f32) -> PredictedPose {
        let alpha = alpha.clamp(0.0, 1.0);
        let current = self.current_pose();
        PredictedPose {
            position: self.previous.position.lerp(current.position, alpha),
            yaw_rad: lerp_angle(self.previous.yaw_rad, current.yaw_rad, alpha),
            hull_pitch_rad: lerp_angle(self.previous.hull_pitch_rad, current.hull_pitch_rad, alpha),
            hull_roll_rad: lerp_angle(self.previous.hull_roll_rad, current.hull_roll_rad, alpha),
            turret_yaw_rad: lerp_angle(self.previous.turret_yaw_rad, current.turret_yaw_rad, alpha),
            gun_pitch_rad: lerp_angle(self.previous.gun_pitch_rad, current.gun_pitch_rad, alpha),
        }
    }
}
