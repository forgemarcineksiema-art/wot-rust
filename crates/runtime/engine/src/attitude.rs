//! Presentation-only sprung-hull attitude: the hull pitches and rolls with the terrain and with
//! its own motion (nose dive under braking, squat under acceleration, lean in turns), filtered
//! through a spring-damper so bumps produce the tank's signature single visible re-settle.
//!
//! This is a render-side cue derived from the replicated pose plus the local heightmap — it is
//! not gameplay state, never leaves the presentation world, and cannot desync anything: the
//! authoritative hull stays the planar rigid body in `physics`.

use bevy_ecs::prelude::*;

/// Spring natural frequency (rad/s) for pitch/roll — ~1.1 Hz reads as tonnes of sprung steel.
const ATTITUDE_OMEGA: f32 = 7.0;
/// Damping ratio: slightly underdamped, so a bump settles with ONE visible nod, not a wobble.
const ATTITUDE_ZETA: f32 = 0.55;
/// Heave (vertical) spring: a touch stiffer so the hull hugs the ground without floating.
const HEAVE_OMEGA: f32 = 9.0;
const HEAVE_ZETA: f32 = 0.6;
/// Radians of dynamic pitch per m/s² of longitudinal acceleration (braking dives the nose).
const PITCH_PER_ACCEL: f32 = 0.006;
/// Radians of dynamic roll per m/s² of lateral (centripetal) acceleration.
const ROLL_PER_ACCEL: f32 = 0.005;
const MAX_DYNAMIC_PITCH: f32 = 0.035;
const MAX_DYNAMIC_ROLL: f32 = 0.035;
/// The sprung hull may float this far ABOVE the replicated height (a drop reads as a settle)...
const MAX_HEAVE_UP_M: f32 = 0.30;
/// ...but only this far BELOW it: a deeper lag visibly buries the tracks in the terrain.
const MAX_HEAVE_DOWN_M: f32 = 0.12;
/// Low-pass rate (1/s) for the acceleration estimates that drive the dynamic cues. The P/v drive
/// launches at ~8 m/s² from the first frame; feeding that raw step into the pitch/roll targets
/// snaps them to their clamps in one frame, which reads as violence rather than weight transfer.
const ACCEL_SMOOTH_PER_S: f32 = 6.0;
/// Spring frequency scale at a fully drained suspension pool: 1.0 at full HP easing to this
/// floor at 0 HP, so a wounded suspension is a visibly softer spring (the hull wallows).
const WOUNDED_OMEGA_FLOOR: f32 = 0.6;
/// Damping ratio scale at a drained pool — the wallow also overshoots a touch more.
const WOUNDED_ZETA_FLOOR: f32 = 0.8;

/// The terrain half of one frame's attitude target, sampled by the presentation world.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct AttitudeSample {
    /// Terrain pitch under the hull (positive = nose up, i.e. ground rises ahead).
    pub terrain_pitch_rad: f32,
    /// Terrain roll under the hull (positive = right side up, i.e. ground rises to the right).
    pub terrain_roll_rad: f32,
}

/// Spring-filtered hull attitude, persisted per presentation entity across frames.
#[derive(Debug, Clone, Copy, PartialEq, Default, Component)]
pub struct HullAttitude {
    /// Filtered pitch (positive = nose up).
    pub pitch_rad: f32,
    /// Filtered roll (positive = right side up).
    pub roll_rad: f32,
    /// Filtered vertical offset applied on top of the replicated hull height.
    pub heave_m: f32,
    /// Filtered longitudinal acceleration (m/s², forward positive) — drives the render-side
    /// track-tension cue as well as the dynamic pitch.
    pub accel_long_mps2: f32,
    /// Filtered lateral (centripetal) acceleration — drives the dynamic roll.
    accel_lat_mps2: f32,
    pitch_vel: f32,
    roll_vel: f32,
    smoothed_y: f32,
    heave_vel: f32,
    prev_translation: [f32; 3],
    prev_yaw: f32,
    prev_speed: f32,
    seeded: bool,
}

impl HullAttitude {
    /// Fold one presented frame into the sprung attitude. `dt` is the render-frame delta; motion
    /// (speed, longitudinal and lateral acceleration) is estimated from the pose history so the
    /// cue needs nothing the snapshot does not already carry. `suspension_fraction` is the live
    /// suspension pool `0..=1`: a wounded suspension is a softer spring, so the hull wallows.
    pub fn step(
        &mut self,
        translation: [f32; 3],
        yaw_rad: f32,
        sample: AttitudeSample,
        suspension_fraction: f32,
        dt: f32,
    ) {
        if !self.seeded {
            self.pitch_rad = sample.terrain_pitch_rad;
            self.roll_rad = sample.terrain_roll_rad;
            self.smoothed_y = translation[1];
            self.prev_translation = translation;
            self.prev_yaw = yaw_rad;
            self.seeded = true;
            return;
        }
        let dt = dt.clamp(1.0e-3, 0.05);

        // A wounded suspension pool softens every spring below: lower frequency (slower settle)
        // and slightly less damping (one extra visible sway) as the pool drains.
        let pool = suspension_fraction.clamp(0.0, 1.0);
        let omega_scale = WOUNDED_OMEGA_FLOOR + (1.0 - WOUNDED_OMEGA_FLOOR) * pool;
        let zeta_scale = WOUNDED_ZETA_FLOOR + (1.0 - WOUNDED_ZETA_FLOOR) * pool;
        let attitude_omega = ATTITUDE_OMEGA * omega_scale;
        let attitude_zeta = ATTITUDE_ZETA * zeta_scale;
        let heave_omega = HEAVE_OMEGA * omega_scale;
        let heave_zeta = HEAVE_ZETA * zeta_scale;

        // Motion estimates from the pose delta: signed forward speed, its derivative, and the
        // centripetal acceleration (speed x yaw rate).
        let dx = translation[0] - self.prev_translation[0];
        let dz = translation[2] - self.prev_translation[2];
        let speed = (dx * yaw_rad.sin() + dz * yaw_rad.cos()) / dt;
        let accel_long = ((speed - self.prev_speed) / dt).clamp(-12.0, 12.0);
        let yaw_rate = game_core::math::wrap_angle(yaw_rad - self.prev_yaw) / dt;
        let accel_lat = (speed * yaw_rate).clamp(-12.0, 12.0);
        self.prev_translation = translation;
        self.prev_yaw = yaw_rad;
        self.prev_speed = speed;
        self.accel_long_mps2 = spring_to(self.accel_long_mps2, accel_long, ACCEL_SMOOTH_PER_S * dt);
        self.accel_lat_mps2 = spring_to(self.accel_lat_mps2, accel_lat, ACCEL_SMOOTH_PER_S * dt);

        // Targets: terrain slope plus the dynamic weight transfer, read from the SMOOTHED
        // accelerations so a throttle tap eases the hull over rather than snapping it. Braking
        // (negative accel while moving forward) dives the nose; accelerating squats it; turning
        // leans the hull out of the corner.
        let pitch_target = sample.terrain_pitch_rad
            + (self.accel_long_mps2 * PITCH_PER_ACCEL).clamp(-MAX_DYNAMIC_PITCH, MAX_DYNAMIC_PITCH);
        let roll_target = sample.terrain_roll_rad
            + (self.accel_lat_mps2 * ROLL_PER_ACCEL).clamp(-MAX_DYNAMIC_ROLL, MAX_DYNAMIC_ROLL);

        spring_step(&mut self.pitch_rad, &mut self.pitch_vel, pitch_target, attitude_omega, attitude_zeta, dt);
        spring_step(&mut self.roll_rad, &mut self.roll_vel, roll_target, attitude_omega, attitude_zeta, dt);

        // Heave: the replicated hull height snaps to the terrain sample; the sprung hull follows
        // it through the same kind of spring, so a step in the heightmap becomes a settle, not a
        // teleport. The offset (not the absolute) is what the render applies.
        spring_step(&mut self.smoothed_y, &mut self.heave_vel, translation[1], heave_omega, heave_zeta, dt);
        self.heave_m =
            (self.smoothed_y - translation[1]).clamp(-MAX_HEAVE_DOWN_M, MAX_HEAVE_UP_M);
    }
}

/// One semi-implicit spring-damper step: `x'' = w^2 (target - x) - 2 z w x'`.
fn spring_step(x: &mut f32, vel: &mut f32, target: f32, omega: f32, zeta: f32, dt: f32) {
    *vel += (omega * omega * (target - *x) - 2.0 * zeta * omega * *vel) * dt;
    *x += *vel * dt;
}

fn spring_to(current: f32, target: f32, alpha: f32) -> f32 {
    current + (target - current) * alpha.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settle(att: &mut HullAttitude, pos: [f32; 3], yaw: f32, sample: AttitudeSample, frames: u32) {
        for _ in 0..frames {
            att.step(pos, yaw, sample, 1.0, 1.0 / 60.0);
        }
    }

    #[test]
    fn attitude_settles_onto_the_terrain_slope() {
        let mut att = HullAttitude::default();
        let sample = AttitudeSample { terrain_pitch_rad: 0.15, terrain_roll_rad: -0.08 };
        settle(&mut att, [0.0, 1.0, 0.0], 0.0, sample, 240);
        assert!((att.pitch_rad - 0.15).abs() < 5.0e-3, "pitch settles: {}", att.pitch_rad);
        assert!((att.roll_rad + 0.08).abs() < 5.0e-3, "roll settles: {}", att.roll_rad);
    }

    #[test]
    fn braking_dives_the_nose() {
        let mut att = HullAttitude::default();
        let dt = 1.0 / 60.0;
        // Drive forward at constant speed, then brake hard: pitch must go negative (nose down).
        let mut z = 0.0;
        att.step([0.0, 0.0, z], 0.0, AttitudeSample::default(), 1.0, dt);
        for _ in 0..60 {
            z += 10.0 * dt;
            att.step([0.0, 0.0, z], 0.0, AttitudeSample::default(), 1.0, dt);
        }
        let mut v = 10.0;
        let mut min_pitch = 0.0_f32;
        for _ in 0..40 {
            v = (v - 8.0 * dt).max(0.0);
            z += v * dt;
            att.step([0.0, 0.0, z], 0.0, AttitudeSample::default(), 1.0, dt);
            min_pitch = min_pitch.min(att.pitch_rad);
        }
        assert!(min_pitch < -0.02, "braking must dive the nose, got {min_pitch}");
    }

    #[test]
    fn a_height_step_becomes_a_damped_settle_not_a_teleport() {
        let mut att = HullAttitude::default();
        settle(&mut att, [0.0, 1.0, 0.0], 0.0, AttitudeSample::default(), 120);
        // The replicated hull jumps 0.2 m up; the sprung hull must lag below it first.
        att.step([0.0, 1.2, 0.0], 0.0, AttitudeSample::default(), 1.0, 1.0 / 60.0);
        assert!(att.heave_m < -0.1, "the sprung hull lags a sudden step, got {}", att.heave_m);
        settle(&mut att, [0.0, 1.2, 0.0], 0.0, AttitudeSample::default(), 240);
        assert!(att.heave_m.abs() < 0.01, "and settles back onto it, got {}", att.heave_m);
    }

    #[test]
    fn a_full_throttle_launch_eases_the_hull_over_instead_of_snapping_it() {
        let mut att = HullAttitude::default();
        let dt = 1.0 / 60.0;
        att.step([0.0, 0.0, 0.0], 0.0, AttitudeSample::default(), 1.0, dt); // seed
        // Hard P/v launch: a constant ~8 m/s² from the very first driven frame.
        let (mut v, mut z) = (0.0_f32, 0.0_f32);
        let mut after_three_frames = 0.0;
        for frame in 0..60 {
            v += 8.0 * dt;
            z += v * dt;
            att.step([0.0, 0.0, z], 0.0, AttitudeSample::default(), 1.0, dt);
            if frame == 2 {
                after_three_frames = att.pitch_rad;
            }
        }
        assert!(
            after_three_frames.abs() < 0.005,
            "the first frames must ease, not snap: {after_three_frames}"
        );
        assert!(
            att.pitch_rad > 0.015,
            "a sustained launch still visibly squats the hull: {}",
            att.pitch_rad
        );
    }

    #[test]
    fn a_wounded_suspension_wallows_longer_than_a_healthy_one() {
        let dt = 1.0 / 60.0;
        let mut healthy = HullAttitude::default();
        let mut wounded = HullAttitude::default();
        settle(&mut healthy, [0.0, 1.0, 0.0], 0.0, AttitudeSample::default(), 240);
        for _ in 0..240 {
            wounded.step([0.0, 1.0, 0.0], 0.0, AttitudeSample::default(), 0.0, dt);
        }
        // The same 0.2 m height step: a quarter second in (before either spring's first
        // overshoot) the softer (wounded) spring must hang visibly further below the hull.
        for _ in 0..15 {
            healthy.step([0.0, 1.2, 0.0], 0.0, AttitudeSample::default(), 1.0, dt);
            wounded.step([0.0, 1.2, 0.0], 0.0, AttitudeSample::default(), 0.0, dt);
        }
        assert!(
            wounded.heave_m < healthy.heave_m - 0.01,
            "wounded {} must lag below healthy {}",
            wounded.heave_m,
            healthy.heave_m
        );
    }
}
