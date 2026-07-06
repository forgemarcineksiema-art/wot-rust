//! The battlefield's air: a wide, slowly breathing wind bed. Two decorrelated noise channels
//! (real stereo width, not a mono source panned) through low-passes whose gain and color drift
//! on two incommensurate LFOs — so the field never loops and never sits still.

use crate::dsp::{Noise, OnePoleLowPass};

pub struct WindAmbience {
    sample_rate_hz: f32,
    left_noise: Noise,
    right_noise: Noise,
    left_lp: OnePoleLowPass,
    right_lp: OnePoleLowPass,
    /// Two slow LFOs at incommensurate rates: their sum never repeats audibly.
    gust_phase: f32,
    drift_phase: f32,
    /// Eased master level so scene changes (garage <-> battle) crossfade.
    level: f32,
    target_level: f32,
}

impl WindAmbience {
    pub fn new(sample_rate_hz: f32, seed: u64) -> Self {
        Self {
            sample_rate_hz,
            left_noise: Noise::new(seed),
            right_noise: Noise::new(seed ^ 0xDEAD_BEEF_CAFE_F00D),
            left_lp: OnePoleLowPass::new(320.0, sample_rate_hz),
            right_lp: OnePoleLowPass::new(320.0, sample_rate_hz),
            gust_phase: 0.0,
            drift_phase: 0.0,
            level: 0.0,
            target_level: 0.0,
        }
    }

    /// Scene-level wind amount: ~1.0 on the open battlefield, lower inside the hangar.
    pub fn set_level(&mut self, level: f32) {
        self.target_level = level.clamp(0.0, 1.0);
    }

    /// Render additively into an interleaved stereo buffer.
    pub fn render_add_stereo(&mut self, out: &mut [f32], gain: f32) {
        let glide = 1.0 - (-1.0 / (0.8 * self.sample_rate_hz)).exp();
        for frame in out.chunks_exact_mut(2) {
            self.level += (self.target_level - self.level) * glide;
            if self.level < 1.0e-4 {
                continue;
            }
            self.gust_phase += std::f32::consts::TAU * 0.13 / self.sample_rate_hz;
            self.drift_phase += std::f32::consts::TAU * 0.071 / self.sample_rate_hz;
            // Breathing amount in 0.55..1.0 — wind falls but never dies.
            let breath = 0.775 + 0.15 * self.gust_phase.sin() + 0.075 * self.drift_phase.sin();
            let cutoff = 240.0 + 160.0 * self.gust_phase.sin().max(0.0);
            self.left_lp.set_cutoff(cutoff, self.sample_rate_hz);
            self.right_lp.set_cutoff(cutoff, self.sample_rate_hz);

            let amount = self.level * breath * gain;
            frame[0] += self.left_lp.process(self.left_noise.signed()) * amount;
            frame[1] += self.right_lp.process(self.right_noise.signed()) * amount;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voice::rms;

    const SR: f32 = 48_000.0;

    fn stereo(wind: &mut WindAmbience, seconds: f32) -> Vec<f32> {
        let mut out = vec![0.0; (seconds * SR) as usize * 2];
        wind.render_add_stereo(&mut out, 1.0);
        out
    }

    #[test]
    fn wind_fades_in_to_its_level_and_out_to_silence() {
        let mut wind = WindAmbience::new(SR, 1);
        wind.set_level(1.0);
        let up = stereo(&mut wind, 3.0);
        assert!(rms(&up[up.len() / 2..]) > 0.01, "wind must be audible at full level");
        wind.set_level(0.0);
        let down = stereo(&mut wind, 5.0);
        assert!(rms(&down[down.len() - 9_600..]) < 1.0e-3, "wind must die when leveled to zero");
    }

    #[test]
    fn the_two_channels_are_decorrelated_for_real_width() {
        let mut wind = WindAmbience::new(SR, 1);
        wind.set_level(1.0);
        let buffer = stereo(&mut wind, 2.0);
        let (mut dot, mut left_sq, mut right_sq) = (0.0f64, 0.0f64, 0.0f64);
        for frame in buffer.chunks_exact(2) {
            dot += f64::from(frame[0]) * f64::from(frame[1]);
            left_sq += f64::from(frame[0]).powi(2);
            right_sq += f64::from(frame[1]).powi(2);
        }
        let correlation = dot / (left_sq.sqrt() * right_sq.sqrt()).max(1.0e-12);
        assert!(
            correlation.abs() < 0.2,
            "stereo wind must not be a panned mono source, correlation {correlation}"
        );
    }

    #[test]
    fn wind_breathes_instead_of_holding_one_level() {
        let mut wind = WindAmbience::new(SR, 1);
        wind.set_level(1.0);
        let buffer = stereo(&mut wind, 10.0);
        // RMS per half-second window, after the fade-in settled.
        let windows: Vec<f32> =
            buffer[2 * SR as usize..].chunks((SR / 2.0) as usize * 2).map(rms).collect();
        let min = windows.iter().copied().fold(f32::MAX, f32::min);
        let max = windows.iter().copied().fold(0.0f32, f32::max);
        assert!(max > min * 1.15, "gusts must modulate the level: {min}..{max}");
    }
}
