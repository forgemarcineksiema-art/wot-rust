//! The shell that missed (Inna Liga D8): a supersonic round passing the ear speaks as an
//! N-wave — one biphasic pressure snap a few milliseconds long, then a short tear of air. The
//! sharpest sound in the game by design: it carries no music, only the fact that death went by
//! this close. Caliber sets the wave's period and weight; the mixer scales gain by how close.

use crate::dsp::{Noise, OnePoleLowPass};
use crate::voice::Voice;

pub struct FlybyCrack {
    /// Samples elapsed since the snap began.
    t: usize,
    /// N-wave samples: bigger rounds push a longer, heavier wave.
    n_samples: usize,
    amplitude: f32,
    noise: Noise,
    tail_lp: OnePoleLowPass,
    tail_samples: usize,
}

impl FlybyCrack {
    pub fn new(caliber_mm: f32, sample_rate_hz: f32, seed: u64) -> Self {
        // ~2.5 ms for a 76 mm round up to ~5 ms for a 122 mm: the N-wave period scales with
        // the shell's diameter (bow-shock geometry), which the ear reads as weight.
        let period_s = (caliber_mm / 100.0).clamp(0.6, 1.6) * 0.0035;
        let weight = (caliber_mm / 100.0).clamp(0.5, 1.5);
        Self {
            t: 0,
            n_samples: (period_s * sample_rate_hz).max(8.0) as usize,
            amplitude: 0.85 * weight,
            noise: Noise::new(seed ^ 0xF17B_75EE_D000_0001),
            tail_lp: OnePoleLowPass::new(3_800.0, sample_rate_hz),
            tail_samples: (0.045 * sample_rate_hz) as usize,
        }
    }
}

impl Voice for FlybyCrack {
    fn render(&mut self, out: &mut [f32]) -> bool {
        let total = self.n_samples + self.tail_samples;
        for sample in out.iter_mut() {
            if self.t >= total {
                return false;
            }
            if self.t < self.n_samples {
                // The N itself: +1 falling linearly through -1 — the textbook sonic-boom shape.
                let phase = self.t as f32 / self.n_samples as f32;
                *sample += (1.0 - 2.0 * phase) * self.amplitude;
            } else {
                // The tear: a fast-dying hiss of ripped air behind the wave.
                let tail_phase = (self.t - self.n_samples) as f32 / self.tail_samples as f32;
                let envelope = (1.0 - tail_phase).powi(2) * 0.25;
                *sample += self.tail_lp.process(self.noise.signed()) * envelope * self.amplitude;
            }
            self.t += 1;
        }
        self.t < total
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voice::peak;

    const SR: f32 = 48_000.0;

    /// Render a voice to completion in small chunks, returning only the samples it touched.
    fn run(voice: &mut FlybyCrack) -> Vec<f32> {
        let mut all = Vec::new();
        loop {
            let mut chunk = vec![0.0; 256];
            let alive = voice.render(&mut chunk);
            all.extend_from_slice(&chunk);
            if !alive {
                while all.last() == Some(&0.0) {
                    all.pop();
                }
                return all;
            }
        }
    }

    #[test]
    fn the_crack_is_short_sharp_and_heavier_with_caliber() {
        let small = run(&mut FlybyCrack::new(76.0, SR, 1));
        let large = run(&mut FlybyCrack::new(122.0, SR, 1));
        assert!(
            small.len() < (0.06 * SR) as usize,
            "an N-wave is milliseconds, not a note: {} samples",
            small.len()
        );
        assert!(peak(&large) > peak(&small), "a 122 mm pass outweighs a 76 mm one");
        assert!(large.len() > small.len(), "the bigger bow shock takes longer to cross the ear");
        assert!(small.iter().chain(&large).all(|s| s.is_finite()));
        // The biphasic shape: the wave starts positive and ends negative before the tail.
        assert!(small[0] > 0.0, "the N leads with overpressure");
    }
}
