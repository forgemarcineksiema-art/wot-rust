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
    pub(super) speed_mps: f32,
    pub(super) fov_boost_deg: f32,
}

/// Follow-spring natural frequency (rad/s) for the HORIZONTAL axes: omega 16 with critical
/// damping is a ~0.13 s lag — tight, because losing the hull sideways is losing the game.
const FOLLOW_OMEGA: f32 = 16.0;
/// Vertical follow frequency (rad/s), softer than the horizontal but only moderately so. The
/// hull's Y is a rigid snap onto the support envelope of a 5 m heightfield — piecewise-linear
/// with a velocity kink at every cell edge — and at omega 16 the anchor passed 62% of a 2 Hz
/// bump train straight into the frame (both eye and target ride the anchor, so a hull bounce
/// translated the WHOLE image). Omega 7 was the first shot and read as a SPRING RIDE on every
/// hill (player verdict 2026-08-22: irritating); omega 9 still passes only ~34% of a 2 Hz bump
/// train while the release from a pinned leash settles under half a second. A hill is signal
/// the player steers by, not noise — the true float bound lives in the short vertical leash.
const FOLLOW_OMEGA_Y: f32 = 9.0;
/// Full-speed FOV widening (degrees) — the subtle "world opens up" cue at top speed.
const SPEED_FOV_BOOST_DEG: f32 = 2.5;
/// Speed (m/s) at which the FOV boost saturates.
const SPEED_FOV_AT_MPS: f32 = 14.0;
/// Blend rate (1/s) easing the FOV toward its speed target. Slow on purpose: a fast blend reads
/// as a zoom lurch on every W/S tap instead of the world gradually opening up.
const FOV_BLEND_PER_S: f32 = 1.6;
/// Hardest the anchor may trail the hull HORIZONTALLY (meters). A spring lag reads as weight;
/// beyond this it reads as the camera losing the tank — hard stops and spawn teleports clamp
/// here. Split from the vertical leash: the two used to share one 3D budget, so horizontal
/// lag under braking ate the whole allowance and the vertical channel clamped rigid exactly
/// when a bump needed it most.
const MAX_ANCHOR_LAG_M: f32 = 0.6;
/// Hardest the anchor may trail the hull VERTICALLY (meters). This is the GAMEPLAY bound of
/// the whole vertical channel: at the default 12 m boom and 48 deg FOV, 0.05 m is ~0.5% of
/// the frame height — the ceiling of what any vertical feel may ever do to the framing. The
/// 0.35 m leash shipped with the omega-7 spring let a crest float the frame through 0.7 m of
/// relative excursion (player verdict 2026-08-22: a spring ride on every hill); the player
/// then ordered the first re-cut, 0.10, shortened again to 0.05 the same day. The soft spring
/// still eats the cm-scale heightfield kinks inside this leash, rides pinned on any real
/// slope, and riding it is smooth by construction: while the position is pinned, the damping
/// term caps the integrator's velocity at omega * leash / 2 ≈ 0.23 m/s, so a release settles
/// gently from above instead of kicking.
const MAX_ANCHOR_LAG_Y_M: f32 = 0.05;
/// Sniper vertical damper frequency (rad/s): ~50 ms — soaks the per-frame jolt of a rut at 3
/// degrees of FOV without ever reading as float.
const SNIPER_Y_OMEGA: f32 = 45.0;
/// Sniper damper authority (meters): the smoothed eye may deviate at most this far vertically.
const SNIPER_Y_MAX_M: f32 = 0.12;
/// Downward anchor velocity injected per m/s of absorbed landing speed.
const KICK_PER_IMPACT_MPS: f32 = 0.22;
/// Hardest camera kick a single landing can inject (m/s of anchor velocity). The 0.05 m leash
/// bounds the dip regardless; 1.5 m/s keeps the pinned beat short — 3.0 kept the anchor pinned
/// for an extra beat right after a crest, stacking bounce on the hill float.
const MAX_KICK_MPS: f32 = 1.5;
/// Anchor velocity of the player's own shot: rearward along the aim (the rig absorbs a share of
/// the recoil) plus a smaller settle-down component. One firm nudge, not a screen shake.
const FIRE_KICK_BACK_MPS: f32 = 0.9;
const FIRE_KICK_DOWN_MPS: f32 = 0.5;
/// Fixed follow-spring substep (seconds), the 60 Hz tick the spring was tuned at. A slow frame is
/// integrated as a whole number of these plus a remainder, so the rig advances by the real time
/// elapsed instead of a single clamped step whose clock ran slower than the world (the rig
/// visibly lagging then lurching to catch up below ~20 FPS). Mirrors the engine attitude substep.
const FOLLOW_SUBSTEP_S: f32 = 1.0 / 60.0;
/// The most real time one presented frame may advance the spring (the stall clamp) — matches the
/// `present()` clamp, so the rig and the boom/mode blend share one ceiling.
const MAX_FRAME_S: f32 = 0.1;

impl BattleCameraController {
    /// Test hooks: still the rig, then read how hard something shoved it.
    #[cfg(test)]
    pub fn zero_motion_for_test(&mut self) {
        self.smoothing.anchor_vel = Vec3::ZERO;
    }

    #[cfg(test)]
    pub fn anchor_speed_for_test(&self) -> f32 {
        self.smoothing.anchor_vel.length()
    }

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

    /// Step the camera feel once per presented frame: the follow anchor springs after the hull
    /// and the FOV boost eases toward the hull speed. `hull_speed_mps` comes from the tick
    /// domain (the predictor's rigid body) — differencing presented positions against the render
    /// clock made the estimate pulse even at a steady cruise. Sniper mode snaps rigid (aiming
    /// tolerates no lag).
    pub fn advance(&mut self, subject_position: [f32; 3], hull_speed_mps: f32, dt: f32) {
        let target = Vec3::from_array(subject_position);
        let sniper = self.mode() == BattleCameraMode::Sniper;
        self.smoothing.speed_mps = hull_speed_mps.abs().min(30.0);
        // Integrate the follow spring in fixed 60 Hz substeps up to the stall clamp — at 60+ FPS
        // exactly one substep (unchanged feel), below it the correct real-time settle instead of
        // the old single clamped step that lagged.
        let mut remaining = dt.clamp(0.0, MAX_FRAME_S);
        while remaining > 1.0e-6 {
            let step = remaining.min(FOLLOW_SUBSTEP_S);
            self.advance_substep(target, sniper, step);
            remaining -= step;
        }
    }

    /// One follow-spring substep at `dt` (see [`Self::advance`] for the substep loop). Reads the
    /// speed already latched by `advance`; the FOV boost eases toward it per substep.
    fn advance_substep(&mut self, target: Vec3, sniper: bool, dt: f32) {
        let s = &mut self.smoothing;
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
        // Critically damped springs, per axis group: tight horizontally (the camera must never
        // lose the hull sideways), soft vertically (the hull's terrain-snapped Y is the bump
        // train — the vertical channel is the camera's suspension).
        let delta = target - anchor;
        let accel = Vec3::new(
            FOLLOW_OMEGA * FOLLOW_OMEGA * delta.x - 2.0 * FOLLOW_OMEGA * s.anchor_vel.x,
            FOLLOW_OMEGA_Y * FOLLOW_OMEGA_Y * delta.y - 2.0 * FOLLOW_OMEGA_Y * s.anchor_vel.y,
            FOLLOW_OMEGA * FOLLOW_OMEGA * delta.z - 2.0 * FOLLOW_OMEGA * s.anchor_vel.z,
        );
        s.anchor_vel += accel * dt;
        let mut next = anchor + s.anchor_vel * dt;
        // The spring may trail, never lose: clamp the lag PER AXIS GROUP (hard stop, spawn
        // teleport, a long slope pinning the soft vertical). The two groups used to share one
        // 3D budget, so horizontal lag under braking ate the allowance and the vertical
        // channel arrived at a bump already pinned rigid. The velocity is deliberately left
        // to the spring: while a position is pinned, the damping term self-limits it to
        // omega * leash / 2, so a release settles instead of kicking.
        let offset = next - target;
        let horizontal = glam::Vec2::new(offset.x, offset.z);
        if horizontal.length() > MAX_ANCHOR_LAG_M {
            let held = horizontal.normalize() * MAX_ANCHOR_LAG_M;
            next.x = target.x + held.x;
            next.z = target.z + held.y;
        }
        if offset.y.abs() > MAX_ANCHOR_LAG_Y_M {
            next.y = target.y + offset.y.signum() * MAX_ANCHOR_LAG_Y_M;
        }
        s.anchor = Some(next);
        let fov_target = SPEED_FOV_BOOST_DEG * (s.speed_mps / SPEED_FOV_AT_MPS).clamp(0.0, 1.0);
        s.fov_boost_deg += (fov_target - s.fov_boost_deg) * (FOV_BLEND_PER_S * dt).clamp(0.0, 1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_ANCHOR_LAG_M, MAX_FRAME_S};
    use crate::camera::controller::BattleCameraController;

    /// The low-FPS fix, camera side: one slow presented frame springs the follow anchor by the
    /// same real time as the equivalent 60 Hz frames. A 0.1 s frame is exactly six 1/60 s
    /// substeps, so the rig must land bit-identical to six 1/60 s steps — no lag-then-lurch.
    #[test]
    fn a_slow_frame_springs_the_anchor_the_same_as_the_equivalent_60hz_frames() {
        let mut one_slow = BattleCameraController::default();
        let mut many_fast = BattleCameraController::default();
        // Seed both anchors at the origin, then drive the subject away so the spring has to work.
        one_slow.advance([0.0, 0.0, 0.0], 8.0, 1.0 / 60.0);
        many_fast.advance([0.0, 0.0, 0.0], 8.0, 1.0 / 60.0);

        one_slow.advance([0.3, 0.0, 0.4], 8.0, 6.0 / 60.0);
        for _ in 0..6 {
            many_fast.advance([0.3, 0.0, 0.4], 8.0, 1.0 / 60.0);
        }

        let a = one_slow.smoothing.anchor.expect("anchor seeded");
        let b = many_fast.smoothing.anchor.expect("anchor seeded");
        assert!((a - b).length() < 1.0e-6, "the follow anchor diverged: {a:?} vs {b:?}");
        assert!(
            (one_slow.smoothing.fov_boost_deg - many_fast.smoothing.fov_boost_deg).abs() < 1.0e-6,
            "the FOV boost diverged"
        );
    }

    /// The stall clamp: a one-second hitch advances the spring by at most MAX_FRAME_S — a monster
    /// stall settles by a bounded amount, it never fast-forwards the rig a full second. Locked as
    /// a dt=1.0 frame landing bit-identical to a dt=MAX_FRAME_S one, with the anchor kept on its
    /// lag leash throughout.
    #[test]
    fn a_monster_hitch_clamps_to_the_stall_ceiling() {
        // A modest continuous move, so the spring is genuinely mid-flight (not a spawn teleport,
        // which is meant to snap): the clamp equivalence is what this locks.
        let mut hitched = BattleCameraController::default();
        let mut capped = BattleCameraController::default();
        hitched.advance([0.0, 0.0, 0.0], 8.0, 1.0 / 60.0);
        capped.advance([0.0, 0.0, 0.0], 8.0, 1.0 / 60.0);

        hitched.advance([0.4, 0.0, 0.3], 8.0, 1.0);
        capped.advance([0.4, 0.0, 0.3], 8.0, MAX_FRAME_S);

        let a = hitched.smoothing.anchor.expect("anchor");
        let b = capped.smoothing.anchor.expect("anchor");
        assert!(
            (a - b).length() < 1.0e-6,
            "a 1 s hitch must clamp to the stall ceiling: {a:?} {b:?}"
        );
        let target = glam::Vec3::new(0.4, 0.0, 0.3);
        assert!(
            (a - target).length() <= MAX_ANCHOR_LAG_M + 1.0e-4,
            "the anchor never leaves its leash"
        );
    }
}
