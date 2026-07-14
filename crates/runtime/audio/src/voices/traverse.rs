//! The turret traverse voice (Inna Liga D3): the electric drive's whine plus the gear bed under
//! it, pitched and weighted by the LIVE slew rate — the one sound that makes ten tons of turret
//! feel like ten tons. A presence like the engine: it spools with the stick and dies with it,
//! never clicks.

use crate::dsp::{Biquad, Noise, OnePoleLowPass};

/// Whine frequency at full slew (Hz); idle sits far below hearing focus.
const WHINE_MAX_HZ: f32 = 540.0;
const WHINE_MIN_HZ: f32 = 160.0;
/// Level smoothing (per second): the drive engages fast, releases a touch slower.
const ATTACK_PER_S: f32 = 9.0;
const RELEASE_PER_S: f32 = 5.0;

pub struct TraverseVoice {
    noise: Noise,
    gear_band: Biquad,
    tone_phase: f32,
    tone_hz: f32,
    smooth_rate: f32,
    level: f32,
    lowpass: OnePoleLowPass,
    sample_rate_hz: f32,
}

impl TraverseVoice {
    pub fn new(sample_rate_hz: f32, seed: u64) -> Self {
        Self {
            noise: Noise::new(seed ^ 0x77AA_5EED),
            gear_band: Biquad::band_pass(900.0, 1.4, sample_rate_hz),
            tone_phase: 0.0,
            tone_hz: WHINE_MIN_HZ,
            smooth_rate: 0.0,
            level: 0.0,
            lowpass: OnePoleLowPass::new(2_600.0, sample_rate_hz),
            sample_rate_hz,
        }
    }

    /// Feed the live slew as a 0..1 fraction of the vehicle's top traverse rate.
    pub fn set_slew(&mut self, slew_norm: f32) {
        self.smooth_rate = slew_norm.clamp(0.0, 1.0);
    }

    pub fn is_silent(&self) -> bool {
        self.smooth_rate <= 0.0 && self.level < 1.0e-4
    }

    pub fn render_add(&mut self, out: &mut [f32], gain: f32) {
        if self.is_silent() {
            return;
        }
        let attack = ATTACK_PER_S / self.sample_rate_hz;
        let release = RELEASE_PER_S / self.sample_rate_hz;
        for sample in out.iter_mut() {
            let target = self.smooth_rate;
            self.level = if self.level < target {
                (self.level + attack).min(target)
            } else {
                (self.level - release).max(target)
            };
            if self.level <= 0.0 {
                continue;
            }
            // The whine glides with the level (drive speed), never steps.
            let target_hz = WHINE_MIN_HZ + (WHINE_MAX_HZ - WHINE_MIN_HZ) * self.level;
            self.tone_hz += (target_hz - self.tone_hz) * 0.002;
            self.tone_phase = (self.tone_phase + self.tone_hz / self.sample_rate_hz) % 1.0;
            let whine = (self.tone_phase * std::f32::consts::TAU).sin()
                + 0.35 * (self.tone_phase * 2.0 * std::f32::consts::TAU).sin();
            let gear = self.gear_band.process(self.noise.signed()) * 1.6;
            let mixed = self.lowpass.process(whine * 0.5 + gear * 0.5);
            *sample += mixed * self.level * gain;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voice::rms;

    const SR: f32 = 48_000.0;

    #[test]
    fn the_drive_spools_with_the_stick_and_dies_with_it() {
        let mut voice = TraverseVoice::new(SR, 3);
        let mut out = vec![0.0; 24_000];
        voice.render_add(&mut out, 1.0);
        assert_eq!(rms(&out), 0.0, "an untouched stick is silent");

        voice.set_slew(0.8);
        let mut spinning = vec![0.0; 24_000];
        voice.render_add(&mut spinning, 1.0);
        assert!(rms(&spinning[12_000..]) > 1.0e-3, "a slewing turret is heard");
        assert!(spinning.iter().all(|s| s.is_finite()));

        voice.set_slew(0.0);
        let mut fade = vec![0.0; 48_000];
        voice.render_add(&mut fade, 1.0);
        assert!(
            rms(&fade[fade.len() - 4_800..]) < 1.0e-4,
            "releasing the stick spools the drive down to silence"
        );
    }
}
