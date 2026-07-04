//! The camera feel layer: a critically damped follow anchor (the rig trails the hull instead of
//! being bolted to it), the hull-speed estimate it derives, and the speed-driven FOV widening.
//! Split from `controller.rs` to keep each module within the reviewability budget.

use glam::Vec3;

use super::BattleCameraMode;
use super::controller::BattleCameraController;

/// Per-frame camera feel state: the critically damped follow anchor (the eye rig lags the hull by
/// ~0.13 s instead of being bolted to it), the speed estimate it derives, and the speed-driven
/// FOV widening. Stepped ONCE per presented frame by [`BattleCameraController::advance`], so the
/// render and the aim ray read the identical camera.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(super) struct CameraSmoothing {
    pub(super) anchor: Option<Vec3>,
    pub(super) anchor_vel: Vec3,
    pub(super) prev_subject: Option<Vec3>,
    pub(super) speed_mps: f32,
    pub(super) fov_boost_deg: f32,
}

/// Follow-spring natural frequency (rad/s): omega 16 with critical damping is a ~0.13 s lag.
const FOLLOW_OMEGA: f32 = 16.0;
/// Full-speed FOV widening (degrees) — the subtle "world opens up" cue at top speed.
const SPEED_FOV_BOOST_DEG: f32 = 4.0;
/// Speed (m/s) at which the FOV boost saturates.
const SPEED_FOV_AT_MPS: f32 = 14.0;
/// Downward anchor velocity injected per m/s of absorbed landing speed.
const KICK_PER_IMPACT_MPS: f32 = 0.22;
/// Hardest camera kick a single landing can inject (m/s of anchor velocity).
const MAX_KICK_MPS: f32 = 3.0;

impl BattleCameraController {
    /// Landing slam: inject downward velocity into the follow anchor; the critically damped
    /// spring turns it into a single dip-and-recover. Sniper mode stays rigid (aiming tolerates
    /// no camera theatrics), matching [`BattleCameraController::advance`].
    pub fn impact_kick(&mut self, impact_mps: f32) {
        if self.mode() == BattleCameraMode::Sniper {
            return;
        }
        self.smoothing.anchor_vel.y -= (impact_mps * KICK_PER_IMPACT_MPS).min(MAX_KICK_MPS);
    }

    /// Step the camera feel once per presented frame: the follow anchor springs after the hull,
    /// the hull speed is estimated from its raw motion, and the FOV boost eases toward the speed.
    /// Sniper mode snaps rigid (aiming tolerates no lag).
    pub fn advance(&mut self, subject_position: [f32; 3], dt: f32) {
        let target = Vec3::from_array(subject_position);
        let dt = dt.clamp(1.0e-3, 0.05);
        let sniper = self.mode() == BattleCameraMode::Sniper;
        let s = &mut self.smoothing;
        if let Some(prev) = s.prev_subject {
            let inst = (target - prev).length() / dt;
            s.speed_mps += (inst.min(30.0) - s.speed_mps) * (8.0 * dt).clamp(0.0, 1.0);
        }
        s.prev_subject = Some(target);

        if sniper {
            s.anchor = Some(target);
            s.anchor_vel = Vec3::ZERO;
            s.fov_boost_deg = 0.0;
            return;
        }
        let anchor = s.anchor.unwrap_or(target);
        // Critically damped spring: the rig trails acceleration, not steady cruise.
        let accel = FOLLOW_OMEGA * FOLLOW_OMEGA * (target - anchor)
            - 2.0 * FOLLOW_OMEGA * s.anchor_vel;
        s.anchor_vel += accel * dt;
        s.anchor = Some(anchor + s.anchor_vel * dt);
        let fov_target = SPEED_FOV_BOOST_DEG * (s.speed_mps / SPEED_FOV_AT_MPS).clamp(0.0, 1.0);
        s.fov_boost_deg += (fov_target - s.fov_boost_deg) * (4.0 * dt).clamp(0.0, 1.0);
    }

}
