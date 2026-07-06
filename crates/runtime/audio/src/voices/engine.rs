//! The player's powerplant: a V-12 tank diesel modeled as its firing pulse train. Every engine
//! cycle excites a low resonant thump; exhaust noise breathes over it with load, and above
//! walking speed the tracks join in — a clatter amplitude-modulated at the real link-passing
//! rate. One continuous voice, driven per frame by three knobs: RPM, load and ground speed.

use crate::dsp::{Biquad, Noise, OnePoleLowPass};
use glam::FloatExt;

/// Firing fundamental range. A four-stroke V-12 fires 6 times per revolution, so 650 RPM idle
/// -> 65 Hz and 2000 RPM flat out -> 200 Hz.
const FIRING_HZ_IDLE: f32 = 65.0;
const FIRING_HZ_MAX: f32 = 200.0;
/// Track link pitch of the reference vehicle (T-54): one link passes the sprocket every 0.137 m.
const LINK_PITCH_M: f32 = 0.137;

pub struct EngineVoice {
    sample_rate_hz: f32,
    /// Smoothed control values — the game thread sets targets, the audio rate glides to them so
    /// RPM never steps audibly between frames.
    rpm_norm: f32,
    load: f32,
    speed_mps: f32,
    target_rpm_norm: f32,
    target_load: f32,
    target_speed_mps: f32,
    /// Master on/off (dead tank, garage): eased so the engine spools down, not cuts.
    target_running: f32,
    running: f32,
    firing_phase: f32,
    lope_phase: f32,
    track_phase: f32,
    noise: Noise,
    exhaust_lp: OnePoleLowPass,
    body_bp: Biquad,
    track_hp: Biquad,
}

impl EngineVoice {
    pub fn new(sample_rate_hz: f32, seed: u64) -> Self {
        Self {
            sample_rate_hz,
            rpm_norm: 0.0,
            load: 0.0,
            speed_mps: 0.0,
            target_rpm_norm: 0.0,
            target_load: 0.0,
            target_speed_mps: 0.0,
            target_running: 0.0,
            running: 0.0,
            firing_phase: 0.0,
            lope_phase: 0.0,
            track_phase: 0.0,
            noise: Noise::new(seed),
            exhaust_lp: OnePoleLowPass::new(700.0, sample_rate_hz),
            body_bp: Biquad::band_pass(95.0, 1.4, sample_rate_hz),
            track_hp: Biquad::high_pass(1_400.0, 0.707, sample_rate_hz),
        }
    }

    /// Per-frame control update from the game thread. `rpm_norm` 0..1 spans idle..governed,
    /// `load` 0..1 is throttle demand, `speed_mps` the hull ground speed.
    pub fn set_state(&mut self, rpm_norm: f32, load: f32, speed_mps: f32, running: bool) {
        self.target_rpm_norm = rpm_norm.clamp(0.0, 1.0);
        self.target_load = load.clamp(0.0, 1.0);
        self.target_speed_mps = speed_mps.abs();
        self.target_running = if running { 1.0 } else { 0.0 };
    }

    /// True once the bed has spooled fully down (switched off and audibly gone) — the remote
    /// engine pool frees slots on this instead of hard-cutting a still-sounding voice.
    pub fn is_silent(&self) -> bool {
        self.target_running == 0.0 && self.running < 1.0e-3
    }

    /// Render additively into `out` (the mixer owns the buffer; the bed never claims it).
    pub fn render_add(&mut self, out: &mut [f32], gain: f32) {
        // Per-sample one-pole glide: ~120 ms to close 63 % of a control step.
        let glide = 1.0 - (-1.0 / (0.12 * self.sample_rate_hz)).exp();
        for sample in out.iter_mut() {
            self.rpm_norm += (self.target_rpm_norm - self.rpm_norm) * glide;
            self.load += (self.target_load - self.load) * glide;
            self.speed_mps += (self.target_speed_mps - self.speed_mps) * glide;
            self.running += (self.target_running - self.running) * glide;
            if self.running < 1.0e-3 {
                continue;
            }

            let firing_hz = FIRING_HZ_IDLE.lerp(FIRING_HZ_MAX, self.rpm_norm);
            self.firing_phase += firing_hz / self.sample_rate_hz;
            if self.firing_phase >= 1.0 {
                self.firing_phase -= 1.0;
            }
            // Each firing event is a sharp exponential within its own cycle; the band-pass turns
            // the pulse train into the engine's low chest tone.
            let pulse = (-6.0 * self.firing_phase).exp() * 2.0 - 0.31;
            let body = self.body_bp.process(pulse) * 2.0.lerp(3.2, self.load);

            // Idle lope: a slow half-order wobble that makes a standing tank sound alive.
            self.lope_phase += std::f32::consts::TAU * firing_hz * 0.5 / 6.0 / self.sample_rate_hz;
            let lope = 1.0 + self.lope_phase.sin() * 0.18 * (1.0 - self.rpm_norm);

            // Exhaust: noise that opens up (louder AND brighter) under load.
            self.exhaust_lp.set_cutoff(500.0.lerp(2_200.0, self.load), self.sample_rate_hz);
            let exhaust = self.exhaust_lp.process(self.noise.signed()) * 0.12.lerp(0.42, self.load);

            // Tracks: clatter high-passed noise, amplitude-modulated at the link-passing rate,
            // fading in above a slow walk.
            let link_hz = self.speed_mps / LINK_PITCH_M;
            self.track_phase += std::f32::consts::TAU * link_hz / self.sample_rate_hz;
            let track_level = (self.speed_mps / 8.0).clamp(0.0, 1.0) * 0.3;
            let modulation = 0.6 + 0.4 * self.track_phase.sin().powi(2);
            let tracks = self.track_hp.process(self.noise.signed()) * track_level * modulation;

            let idle_floor = 0.4.lerp(1.0, self.rpm_norm.max(self.load));
            *sample += (body * lope + exhaust + tracks) * idle_floor * self.running * gain;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voice::{peak, rms, zero_crossing_rate_hz};

    const SR: f32 = 48_000.0;

    fn render(engine: &mut EngineVoice, seconds: f32) -> Vec<f32> {
        let mut out = vec![0.0; (seconds * SR) as usize];
        engine.render_add(&mut out, 1.0);
        out
    }

    #[test]
    fn a_running_engine_is_audible_and_a_stopped_one_spools_down_to_silence() {
        let mut engine = EngineVoice::new(SR, 4);
        engine.set_state(0.2, 0.0, 0.0, true);
        let idle = render(&mut engine, 1.0);
        assert!(rms(&idle[SR as usize / 2..]) > 0.01, "an idling engine must be heard");
        assert!(idle.iter().all(|s| s.is_finite()));

        engine.set_state(0.2, 0.0, 0.0, false);
        let stopping = render(&mut engine, 2.0);
        let tail = &stopping[stopping.len() - 4_800..];
        assert!(rms(tail) < 1.0e-3, "a stopped engine must fade to silence");
    }

    #[test]
    fn higher_rpm_raises_the_firing_fundamental() {
        let mut idle_engine = EngineVoice::new(SR, 4);
        idle_engine.set_state(0.0, 0.0, 0.0, true);
        let idle = render(&mut idle_engine, 2.0);

        let mut governed = EngineVoice::new(SR, 4);
        governed.set_state(1.0, 1.0, 0.0, true);
        let flat_out = render(&mut governed, 2.0);

        // Compare after the glide settled.
        let settled = SR as usize..2 * SR as usize;
        let idle_zcr = zero_crossing_rate_hz(&idle[settled.clone()], SR);
        let flat_zcr = zero_crossing_rate_hz(&flat_out[settled], SR);
        assert!(
            flat_zcr > idle_zcr * 1.3,
            "full throttle must sit higher than idle: {flat_zcr} vs {idle_zcr}"
        );
    }

    #[test]
    fn moving_adds_track_clatter_over_the_standing_engine() {
        let mut standing = EngineVoice::new(SR, 4);
        standing.set_state(0.5, 0.5, 0.0, true);
        let parked = render(&mut standing, 2.0);

        let mut rolling = EngineVoice::new(SR, 4);
        rolling.set_state(0.5, 0.5, 8.0, true);
        let moving = render(&mut rolling, 2.0);

        // Track clatter lives above 1.4 kHz; crude probe: the moving take is brighter.
        let settled = SR as usize..2 * SR as usize;
        let parked_zcr = zero_crossing_rate_hz(&parked[settled.clone()], SR);
        let moving_zcr = zero_crossing_rate_hz(&moving[settled], SR);
        assert!(
            moving_zcr > parked_zcr,
            "tracks must brighten the moving tank: {moving_zcr} vs {parked_zcr}"
        );
    }

    #[test]
    fn the_bed_is_additive_and_bounded() {
        let mut engine = EngineVoice::new(SR, 4);
        engine.set_state(1.0, 1.0, 10.0, true);
        let mut out = vec![0.25; SR as usize];
        engine.render_add(&mut out, 1.0);
        assert!(out.iter().all(|s| s.is_finite()));
        assert!(peak(&out) < 4.0, "the engine bed must not explode the mix");
        assert!(out.iter().any(|s| (s - 0.25).abs() > 1.0e-4), "must add onto existing content");
    }
}
