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
const SPEED_FOV_BOOST_DEG: f32 = 2.5;
/// Speed (m/s) at which the FOV boost saturates.
const SPEED_FOV_AT_MPS: f32 = 14.0;
/// Blend rate (1/s) easing the FOV toward its speed target. Slow on purpose: a fast blend reads
/// as a zoom lurch on every W/S tap instead of the world gradually opening up.
const FOV_BLEND_PER_S: f32 = 1.6;
/// Hardest the anchor may trail the hull (meters). A spring lag reads as weight; beyond this it
/// reads as the camera losing the tank — hard stops and spawn teleports clamp here.
const MAX_ANCHOR_LAG_M: f32 = 0.6;
/// Sniper vertical damper frequency (rad/s): ~50 ms — soaks the per-frame jolt of a rut at 3
/// degrees of FOV without ever reading as float.
const SNIPER_Y_OMEGA: f32 = 45.0;
/// Sniper damper authority (meters): the smoothed eye may deviate at most this far vertically.
const SNIPER_Y_MAX_M: f32 = 0.12;
/// Downward anchor velocity injected per m/s of absorbed landing speed.
const KICK_PER_IMPACT_MPS: f32 = 0.22;
/// Hardest camera kick a single landing can inject (m/s of anchor velocity).
const MAX_KICK_MPS: f32 = 3.0;
/// Anchor velocity of the player's own shot: rearward along the aim (the rig absorbs a share of
/// the recoil) plus a smaller settle-down component. One firm nudge, not a screen shake.
const FIRE_KICK_BACK_MPS: f32 = 0.9;
const FIRE_KICK_DOWN_MPS: f32 = 0.5;

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

    /// Taking a hit rocks the rig. `push` is the direction the impact shoves the hull (hit point
    /// toward hull centre; only its horizontal part is used) and `severity` is the damage relative
    /// to the full health pool — a 0-damage bounce still lands a readable clang. Third person gets
    /// a directional shove plus a downward settle through the follow spring; sniper keeps only the
    /// vertical channel, which its micro-damper caps at [`SNIPER_Y_MAX_M`] — the scope dips for a
    /// beat but the aim never smears sideways.
    pub fn damage_shudder(&mut self, push: Vec3, severity: f32) {
        let severity = severity.clamp(0.0, 1.0);
        if self.mode() == BattleCameraMode::Sniper {
            // Displace the damped anchor directly: at omega 45 a velocity impulse is eaten within
            // a frame, but a displacement reads as a dip the damper then recovers from.
            if let Some(anchor) = self.smoothing.anchor.as_mut() {
                anchor.y -= 0.035 + 0.075 * severity;
            }
            self.smoothing.anchor_vel.y -= 0.4 + 0.8 * severity;
            return;
        }
        let horizontal = Vec3::new(push.x, 0.0, push.z).normalize_or_zero();
        self.smoothing.anchor_vel += horizontal * (0.5 + 2.0 * severity);
        self.smoothing.anchor_vel.y -= 0.35 + 1.1 * severity;
    }

    /// The player's own shot nudges the follow rig back along the aim and slightly down; the
    /// critically damped spring returns it in one settle. Sniper stays rigid — at 3 degrees of
    /// FOV even this nudge would smear the sight picture.
    pub fn fire_kick(&mut self, view_yaw_rad: f32) {
        if self.mode() == BattleCameraMode::Sniper {
            return;
        }
        let forward = game_core::math::horizontal_forward(view_yaw_rad);
        self.smoothing.anchor_vel -= forward * FIRE_KICK_BACK_MPS;
        self.smoothing.anchor_vel.y -= FIRE_KICK_DOWN_MPS;
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
            // HORIZONTAL speed only: heightmap steps and suspension bounce are not "speed", and
            // feeding them into the FOV cue made the view pulse on every rut.
            let delta = target - prev;
            let inst = Vec3::new(delta.x, 0.0, delta.z).length() / dt;
            s.speed_mps += (inst.min(30.0) - s.speed_mps) * (8.0 * dt).clamp(0.0, 1.0);
        }
        s.prev_subject = Some(target);

        if sniper {
            // Sniper is rigid in the aim plane, but a short vertical-only damper soaks the
            // per-frame jolt of ruts — at 3 degrees of FOV a 1:1 hull jolt slams the whole sight.
            let anchor = s.anchor.unwrap_or(target);
            let accel = SNIPER_Y_OMEGA * SNIPER_Y_OMEGA * (target.y - anchor.y)
                - 2.0 * SNIPER_Y_OMEGA * s.anchor_vel.y;
            let vel_y = s.anchor_vel.y + accel * dt;
            let damped_y =
                (anchor.y + vel_y * dt).clamp(target.y - SNIPER_Y_MAX_M, target.y + SNIPER_Y_MAX_M);
            s.anchor = Some(Vec3::new(target.x, damped_y, target.z));
            s.anchor_vel = Vec3::new(0.0, vel_y, 0.0);
            s.fov_boost_deg = 0.0;
            return;
        }
        let anchor = s.anchor.unwrap_or(target);
        // Critically damped spring: the rig trails acceleration, not steady cruise.
        let accel =
            FOLLOW_OMEGA * FOLLOW_OMEGA * (target - anchor) - 2.0 * FOLLOW_OMEGA * s.anchor_vel;
        s.anchor_vel += accel * dt;
        let mut next = anchor + s.anchor_vel * dt;
        // The spring may trail, never lose: clamp the total lag (hard stop, spawn teleport).
        let offset = next - target;
        if offset.length() > MAX_ANCHOR_LAG_M {
            next = target + offset.normalize() * MAX_ANCHOR_LAG_M;
        }
        s.anchor = Some(next);
        let fov_target = SPEED_FOV_BOOST_DEG * (s.speed_mps / SPEED_FOV_AT_MPS).clamp(0.0, 1.0);
        s.fov_boost_deg += (fov_target - s.fov_boost_deg) * (FOV_BLEND_PER_S * dt).clamp(0.0, 1.0);
    }
}
