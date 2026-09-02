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
/// Angular velocity (rad/s) a main-gun shot injects into the sprung hull. With the attitude
/// spring at ~1.1 Hz this peaks around 0.8 degrees of deflection and settles in one visible nod —
/// tonnes of hull rocking on its torsion bars, not a flinch.
const FIRE_IMPULSE_RAD_S: f32 = 0.16;
/// Spring frequency scale at a fully drained suspension pool: 1.0 at full HP easing to this
/// floor at 0 HP, so a wounded suspension is a visibly softer spring (the hull wallows).
const WOUNDED_OMEGA_FLOOR: f32 = 0.6;
/// Damping ratio scale at a drained pool — the wallow also overshoots a touch more.
const WOUNDED_ZETA_FLOOR: f32 = 0.8;
/// Fixed integration substep (seconds) for the springs — the 60 Hz tick the springs were tuned
/// at. A slow or uneven presented frame is integrated as a whole number of these plus a
/// remainder, so the spring's clock always tracks real time: at 60+ FPS a frame is exactly one
/// substep (unchanged feel), and below it the springs advance by the true elapsed time instead
/// of a single clamped step that ran their clock slower than the world (the low-FPS rubbery lag).
const SPRING_SUBSTEP_S: f32 = 1.0 / 60.0;
/// The most real time one presented frame may advance the springs (the stall clamp): a long
/// hitch settles by this much, not a full second of fast-forward. Shared with the camera rig.
const MAX_FRAME_S: f32 = 0.1;

/// The terrain half of one frame's attitude target, sampled by the presentation world.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct AttitudeSample {
    /// Terrain pitch under the hull (positive = nose up, i.e. ground rises ahead).
    pub terrain_pitch_rad: f32,
    /// Terrain roll under the hull (positive = right side up, i.e. ground rises to the right).
    pub terrain_roll_rad: f32,
}

/// Tick-domain hull motion feeding the presentation cues. The values come from whoever actually
/// knows the motion — the local predictor's rigid body or the snapshot pair — NOT from
/// differentiating presented positions against the render clock: the presented pose advances on
/// the sim clock while the frame delta is measured on the render clock, so a finite difference
/// across the two is noise even at a perfectly steady cruise (that noise was visible hull jitter).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TankMotion {
    /// Signed speed along the hull heading (m/s, forward positive).
    pub forward_speed_mps: f32,
    /// Rate of change of the forward speed (m/s², forward positive).
    pub accel_long_mps2: f32,
    /// Hull angular velocity (rad/s) — with the forward speed it yields the centripetal cue.
    pub yaw_rate_rad_s: f32,
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
    seeded: bool,
}

impl HullAttitude {
    /// Fold one presented frame into the sprung attitude. `dt` is the render-frame delta used to
    /// integrate the springs only; `motion` is the tick-domain hull motion (see [`TankMotion`] for
    /// why it must not be re-derived from presented positions). `suspension_fraction` is the live
    /// suspension pool `0..=1`: a wounded suspension is a softer spring, so the hull wallows.
    pub fn step(
        &mut self,
        translation: [f32; 3],
        motion: TankMotion,
        sample: AttitudeSample,
        suspension_fraction: f32,
        dt: f32,
    ) {
        if !self.seeded {
            self.pitch_rad = sample.terrain_pitch_rad;
            self.roll_rad = sample.terrain_roll_rad;
            self.smoothed_y = translation[1];
            self.seeded = true;
            return;
        }
        // Integrate the springs in fixed 60 Hz substeps up to the stall clamp: the frame's motion
        // and terrain inputs are constant across the substeps, but the spring clock advances by
        // the true elapsed time. At 60+ FPS this is exactly one substep — bit-identical to the
        // old single 1/60 step — so this is a no-op at frame rate and only bites the slow frames.
        let mut remaining = dt.clamp(0.0, MAX_FRAME_S);
        while remaining > 1.0e-6 {
            let step = remaining.min(SPRING_SUBSTEP_S);
            self.integrate(translation, motion, sample, suspension_fraction, step);
            remaining -= step;
        }
    }

    /// One spring-integration substep at `dt` (see [`Self::step`] for the substep loop).
    fn integrate(
        &mut self,
        translation: [f32; 3],
        motion: TankMotion,
        sample: AttitudeSample,
        suspension_fraction: f32,
        dt: f32,
    ) {
        // A wounded suspension pool softens every spring below: lower frequency (slower settle)
        // and slightly less damping (one extra visible sway) as the pool drains.
        let pool = suspension_fraction.clamp(0.0, 1.0);
        let omega_scale = WOUNDED_OMEGA_FLOOR + (1.0 - WOUNDED_OMEGA_FLOOR) * pool;
        let zeta_scale = WOUNDED_ZETA_FLOOR + (1.0 - WOUNDED_ZETA_FLOOR) * pool;
        let attitude_omega = ATTITUDE_OMEGA * omega_scale;
        let attitude_zeta = ATTITUDE_ZETA * zeta_scale;
        let heave_omega = HEAVE_OMEGA * omega_scale;
        let heave_zeta = HEAVE_ZETA * zeta_scale;

        // Motion cues straight from the tick domain: the longitudinal weight transfer and the
        // centripetal acceleration (speed x yaw rate).
        let accel_long = motion.accel_long_mps2.clamp(-12.0, 12.0);
        let accel_lat = (motion.forward_speed_mps * motion.yaw_rate_rad_s).clamp(-12.0, 12.0);
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

        spring_step(
            &mut self.pitch_rad,
            &mut self.pitch_vel,
            pitch_target,
            attitude_omega,
            attitude_zeta,
            dt,
        );
        spring_step(
            &mut self.roll_rad,
            &mut self.roll_vel,
            roll_target,
            attitude_omega,
            attitude_zeta,
            dt,
        );

        // Heave: the replicated hull height snaps to the terrain sample; the sprung hull follows
        // it through the same kind of spring, so a step in the heightmap becomes a settle, not a
        // teleport. The offset (not the absolute) is what the render applies.
        spring_step(
            &mut self.smoothed_y,
            &mut self.heave_vel,
            translation[1],
            heave_omega,
            heave_zeta,
            dt,
        );
        self.heave_m = (self.smoothed_y - translation[1]).clamp(-MAX_HEAVE_DOWN_M, MAX_HEAVE_UP_M);
    }

    /// One main-gun shot rocks the sprung hull through the same spring that absorbs terrain: the
    /// recoil moment decomposes by the turret's hull-relative heading, so firing over the bow
    /// lifts the nose while firing over the side rolls the hull away from the muzzle.
    /// `recoil_scale` is the round's (`ShellSpec::recoil_scale`, Inny Poziom S3): the impulse
    /// this spring was tuned on is the D-10's; heavier guns rock the hull further.
    pub fn fire_impulse(&mut self, turret_yaw_rad: f32, recoil_scale: f32) {
        let impulse = FIRE_IMPULSE_RAD_S * recoil_scale.clamp(0.4, 2.0);
        self.pitch_vel += turret_yaw_rad.cos() * impulse;
        self.roll_vel += turret_yaw_rad.sin() * impulse;
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

    fn settle(att: &mut HullAttitude, pos: [f32; 3], sample: AttitudeSample, frames: u32) {
        for _ in 0..frames {
            att.step(pos, TankMotion::default(), sample, 1.0, 1.0 / 60.0);
        }
    }

    fn cruise(speed_mps: f32, accel_mps2: f32) -> TankMotion {
        TankMotion {
            forward_speed_mps: speed_mps,
            accel_long_mps2: accel_mps2,
            yaw_rate_rad_s: 0.0,
        }
    }

    #[test]
    fn attitude_settles_onto_the_terrain_slope() {
        let mut att = HullAttitude::default();
        let sample = AttitudeSample { terrain_pitch_rad: 0.15, terrain_roll_rad: -0.08 };
        settle(&mut att, [0.0, 1.0, 0.0], sample, 240);
        assert!((att.pitch_rad - 0.15).abs() < 5.0e-3, "pitch settles: {}", att.pitch_rad);
        assert!((att.roll_rad + 0.08).abs() < 5.0e-3, "roll settles: {}", att.roll_rad);
    }

    #[test]
    fn braking_dives_the_nose() {
        let mut att = HullAttitude::default();
        let dt = 1.0 / 60.0;
        // Cruise at constant speed, then brake hard: pitch must go negative (nose down).
        settle(&mut att, [0.0, 0.0, 0.0], AttitudeSample::default(), 1);
        for _ in 0..60 {
            att.step([0.0, 0.0, 0.0], cruise(10.0, 0.0), AttitudeSample::default(), 1.0, dt);
        }
        let mut v = 10.0_f32;
        let mut min_pitch = 0.0_f32;
        for _ in 0..40 {
            v = (v - 8.0 * dt).max(0.0);
            let accel = if v > 0.0 { -8.0 } else { 0.0 };
            att.step([0.0, 0.0, 0.0], cruise(v, accel), AttitudeSample::default(), 1.0, dt);
            min_pitch = min_pitch.min(att.pitch_rad);
        }
        assert!(min_pitch < -0.02, "braking must dive the nose, got {min_pitch}");
    }

    #[test]
    fn a_turn_leans_the_hull_through_the_centripetal_cue() {
        let mut att = HullAttitude::default();
        let dt = 1.0 / 60.0;
        settle(&mut att, [0.0, 0.0, 0.0], AttitudeSample::default(), 1);
        let turning =
            TankMotion { forward_speed_mps: 8.0, accel_long_mps2: 0.0, yaw_rate_rad_s: 0.8 };
        for _ in 0..120 {
            att.step([0.0, 0.0, 0.0], turning, AttitudeSample::default(), 1.0, dt);
        }
        assert!(att.roll_rad > 0.015, "a sustained turn must lean the hull, got {}", att.roll_rad);
    }

    #[test]
    fn a_height_step_becomes_a_damped_settle_not_a_teleport() {
        let mut att = HullAttitude::default();
        settle(&mut att, [0.0, 1.0, 0.0], AttitudeSample::default(), 120);
        // The replicated hull jumps 0.2 m up; the sprung hull must lag below it first.
        att.step(
            [0.0, 1.2, 0.0],
            TankMotion::default(),
            AttitudeSample::default(),
            1.0,
            1.0 / 60.0,
        );
        assert!(att.heave_m < -0.1, "the sprung hull lags a sudden step, got {}", att.heave_m);
        settle(&mut att, [0.0, 1.2, 0.0], AttitudeSample::default(), 240);
        assert!(att.heave_m.abs() < 0.01, "and settles back onto it, got {}", att.heave_m);
    }

    #[test]
    fn a_full_throttle_launch_eases_the_hull_over_instead_of_snapping_it() {
        let mut att = HullAttitude::default();
        let dt = 1.0 / 60.0;
        settle(&mut att, [0.0, 0.0, 0.0], AttitudeSample::default(), 1); // seed
        // Hard P/v launch: a constant ~8 m/s² from the very first driven frame.
        let mut v = 0.0_f32;
        let mut after_three_frames = 0.0;
        for frame in 0..60 {
            v += 8.0 * dt;
            att.step([0.0, 0.0, 0.0], cruise(v, 8.0), AttitudeSample::default(), 1.0, dt);
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
    fn a_steady_cruise_on_a_jittery_frame_clock_keeps_the_hull_level() {
        // THE frame-domain jitter regression: a tank cruising at a perfectly steady speed,
        // presented through wildly uneven frame deltas (vsync misses, catch-up batches), must not
        // rock the hull. The old cue differentiated presented positions against the render clock,
        // so this exact scenario produced visible pitch noise at a constant real speed.
        let mut att = HullAttitude::default();
        let dts = [1.0 / 240.0, 1.0 / 30.0, 1.0 / 144.0, 1.0 / 45.0, 1.0 / 60.0, 1.0 / 33.0];
        att.step([0.0, 0.0, 0.0], cruise(10.0, 0.0), AttitudeSample::default(), 1.0, 1.0 / 60.0);
        let mut max_abs_pitch = 0.0_f32;
        for frame in 0..600 {
            let dt = dts[frame % dts.len()];
            att.step([0.0, 0.0, 0.0], cruise(10.0, 0.0), AttitudeSample::default(), 1.0, dt);
            if frame > 60 {
                max_abs_pitch = max_abs_pitch.max(att.pitch_rad.abs());
            }
        }
        assert!(
            max_abs_pitch < 1.0e-4,
            "a steady cruise must not rock the hull, saw {max_abs_pitch} rad of pitch"
        );
    }

    /// The low-FPS fix: one slow presented frame advances the springs by the SAME real time as
    /// the equivalent run of 60 Hz frames. A 0.1 s frame is exactly six 1/60 s substeps, so it
    /// must land bit-identical to six 1/60 s steps — the spring clock tracks the world, it does
    /// not run slow and lag (the old single clamped step did, hence the rubbery catch-up).
    #[test]
    fn a_slow_frame_settles_the_same_as_the_equivalent_60hz_frames() {
        let sample = AttitudeSample { terrain_pitch_rad: 0.12, terrain_roll_rad: -0.05 };
        let mut one_slow = HullAttitude::default();
        let mut many_fast = HullAttitude::default();
        // Seed both onto flat ground, then displace the target so the springs have work to do.
        one_slow.step(
            [0.0, 1.0, 0.0],
            TankMotion::default(),
            AttitudeSample::default(),
            1.0,
            1.0 / 60.0,
        );
        many_fast.step(
            [0.0, 1.0, 0.0],
            TankMotion::default(),
            AttitudeSample::default(),
            1.0,
            1.0 / 60.0,
        );

        let motion = cruise(9.0, -6.0);
        one_slow.step([0.0, 1.2, 0.0], motion, sample, 1.0, 6.0 / 60.0);
        for _ in 0..6 {
            many_fast.step([0.0, 1.2, 0.0], motion, sample, 1.0, 1.0 / 60.0);
        }

        assert!((one_slow.pitch_rad - many_fast.pitch_rad).abs() < 1.0e-6, "pitch diverged");
        assert!((one_slow.roll_rad - many_fast.roll_rad).abs() < 1.0e-6, "roll diverged");
        assert!((one_slow.heave_m - many_fast.heave_m).abs() < 1.0e-6, "heave diverged");
    }

    /// The stall clamp: a monster hitch settles by at most MAX_FRAME_S, never a full second of
    /// fast-forward — so a one-second stall does not snap the hull straight to its target.
    #[test]
    fn a_monster_hitch_is_capped_at_the_stall_clamp() {
        let sample = AttitudeSample { terrain_pitch_rad: 0.2, terrain_roll_rad: 0.0 };
        let mut hitched = HullAttitude::default();
        let mut capped = HullAttitude::default();
        hitched.step(
            [0.0, 0.0, 0.0],
            TankMotion::default(),
            AttitudeSample::default(),
            1.0,
            1.0 / 60.0,
        );
        capped.step(
            [0.0, 0.0, 0.0],
            TankMotion::default(),
            AttitudeSample::default(),
            1.0,
            1.0 / 60.0,
        );

        hitched.step([0.0, 0.0, 0.0], TankMotion::default(), sample, 1.0, 1.0);
        capped.step([0.0, 0.0, 0.0], TankMotion::default(), sample, 1.0, MAX_FRAME_S);

        assert!((hitched.pitch_rad - capped.pitch_rad).abs() < 1.0e-6, "a 1 s hitch must clamp");
        assert!(
            hitched.pitch_rad < sample.terrain_pitch_rad,
            "and not snap to target in one frame"
        );
    }

    #[test]
    fn a_wounded_suspension_wallows_longer_than_a_healthy_one() {
        let dt = 1.0 / 60.0;
        let mut healthy = HullAttitude::default();
        let mut wounded = HullAttitude::default();
        settle(&mut healthy, [0.0, 1.0, 0.0], AttitudeSample::default(), 240);
        for _ in 0..240 {
            wounded.step(
                [0.0, 1.0, 0.0],
                TankMotion::default(),
                AttitudeSample::default(),
                0.0,
                dt,
            );
        }
        // The same 0.2 m height step: a quarter second in (before either spring's first
        // overshoot) the softer (wounded) spring must hang visibly further below the hull.
        for _ in 0..15 {
            healthy.step(
                [0.0, 1.2, 0.0],
                TankMotion::default(),
                AttitudeSample::default(),
                1.0,
                dt,
            );
            wounded.step(
                [0.0, 1.2, 0.0],
                TankMotion::default(),
                AttitudeSample::default(),
                0.0,
                dt,
            );
        }
        assert!(
            wounded.heave_m < healthy.heave_m - 0.01,
            "wounded {} must lag below healthy {}",
            wounded.heave_m,
            healthy.heave_m
        );
    }
}

/// Inny Poziom S3: the hull's rock scales with the round's recoil like every other channel.
#[cfg(test)]
mod recoil_scale_locks {
    use super::*;

    #[test]
    fn a_heavier_round_rocks_the_hull_harder_and_the_reference_keeps_its_impulse() {
        let mut reference = HullAttitude::default();
        reference.fire_impulse(0.0, 1.0);
        assert!((reference.pitch_vel - FIRE_IMPULSE_RAD_S).abs() < 1.0e-6, "the D-10 keeps 0.16");
        let mut heavy = HullAttitude::default();
        heavy.fire_impulse(0.0, 1.36);
        let mut light = HullAttitude::default();
        light.fire_impulse(0.0, 0.67);
        assert!(heavy.pitch_vel > reference.pitch_vel && reference.pitch_vel > light.pitch_vel);
        assert!(
            heavy.pitch_vel > light.pitch_vel * 1.8,
            "{} vs {}",
            heavy.pitch_vel,
            light.pitch_vel
        );
    }
}
