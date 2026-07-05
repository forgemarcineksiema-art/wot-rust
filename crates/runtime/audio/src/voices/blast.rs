//! The high-explosive burst: not a plate ringing but the charge itself. A deep sub boom
//! sweeping down as the fireball expands, a broadband pressure wave whose spectrum slams shut,
//! a sharp crack at detonation, and a long rain of thrown debris — altogether wider, deeper and
//! longer than any kinetic clang (those live in [`super::impact`]).

use crate::dsp::{Biquad, ExpDecay, Noise, OnePoleLowPass};
use crate::voice::Voice;

pub struct HeBlast {
    sample_rate_hz: f32,
    age_samples: usize,
    noise: Noise,
    crack_env: ExpDecay,
    crack_hp: Biquad,
    wave_env: ExpDecay,
    wave_lp: OnePoleLowPass,
    boom_env: ExpDecay,
    boom_phase: f32,
    debris: Vec<(usize, usize)>, // (start_sample, end_sample)
    debris_hp: Biquad,
    last_debris: usize,
}

impl HeBlast {
    pub fn new(sample_rate_hz: f32, seed: u64) -> Self {
        let mut noise = Noise::new(seed);
        // 10-20 fragments and clods land 60-700 ms after the burst, each 8-30 ms.
        let count = 10 + (noise.unit() * 10.0) as usize;
        let mut debris = Vec::with_capacity(count);
        for _ in 0..count {
            let start = ((0.06 + noise.unit() * 0.64) * sample_rate_hz) as usize;
            let length = ((0.008 + noise.unit() * 0.022) * sample_rate_hz) as usize;
            debris.push((start, start + length));
        }
        let last_debris = debris.iter().map(|(_, end)| *end).max().unwrap_or(0);
        Self {
            sample_rate_hz,
            age_samples: 0,
            noise,
            crack_env: ExpDecay::new(0.6, 0.005, sample_rate_hz),
            crack_hp: Biquad::high_pass(1_400.0, 0.707, sample_rate_hz),
            wave_env: ExpDecay::new(0.9, 0.12, sample_rate_hz),
            wave_lp: OnePoleLowPass::new(3_000.0, sample_rate_hz),
            boom_env: ExpDecay::new(1.25, 0.45, sample_rate_hz),
            boom_phase: 0.0,
            debris,
            debris_hp: Biquad::high_pass(1_100.0, 0.707, sample_rate_hz),
            last_debris,
        }
    }
}

impl Voice for HeBlast {
    fn render(&mut self, out: &mut [f32]) -> bool {
        for sample in out.iter_mut() {
            let t = self.age_samples as f32 / self.sample_rate_hz;
            // The pressure wave's spectrum slams shut on its own decay clock.
            let sweep = (-t / 0.12).exp();
            self.wave_lp.set_cutoff(120.0 + 2_880.0 * sweep, self.sample_rate_hz);

            let raw = self.noise.signed();
            let crack = self.crack_hp.process(raw) * self.crack_env.step();
            let wave = self.wave_lp.process(raw) * self.wave_env.step();

            // Sub boom: 45 Hz falling to 25 Hz as the fireball cools.
            let boom_hz = 25.0 + 20.0 * sweep;
            self.boom_phase += std::f32::consts::TAU * boom_hz / self.sample_rate_hz;
            let boom = self.boom_phase.sin() * self.boom_env.step();

            let in_grain = self
                .debris
                .iter()
                .any(|(start, end)| self.age_samples >= *start && self.age_samples < *end);
            let debris = if in_grain { self.debris_hp.process(raw) * 0.10 } else { 0.0 };

            *sample = crack + wave + boom + debris;
            self.age_samples += 1;
        }
        !(self.crack_env.is_quiet()
            && self.wave_env.is_quiet()
            && self.boom_env.is_quiet()
            && self.age_samples > self.last_debris)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voice::{peak, render_to_vec, rms};
    use crate::voices::impact::ArmorHit;

    const SR: f32 = 48_000.0;

    /// HE is the charge, not the plate: the burst must carry more low end than the bare plate
    /// ring (the bounce voice — the penetration variant already carries its own interior thunk)
    /// and keep rumbling (and raining debris) after the clang has died.
    #[test]
    fn a_high_explosive_burst_is_deeper_and_longer_than_a_kinetic_clang() {
        let blast = render_to_vec(&mut HeBlast::new(SR, 7), 3 * SR as usize);
        let clang = render_to_vec(&mut ArmorHit::new(false, false, SR, 7), 3 * SR as usize);
        assert!(blast.iter().all(|s| s.is_finite()));
        assert!(peak(&blast[blast.len().saturating_sub(256)..]) < 0.01, "the burst ends");

        let low_share = |wave: &[f32]| {
            let mut lp = crate::dsp::OnePoleLowPass::new(120.0, SR);
            let low: Vec<f32> = wave.iter().map(|s| lp.process(*s)).collect();
            rms(&low) / rms(wave).max(1.0e-9)
        };
        // Compare character AFTER the detonation transient (both voices open broadband).
        let window = (0.04 * SR) as usize..(0.5 * SR) as usize;
        assert!(
            low_share(&blast[window.clone()])
                > low_share(clang.get(window).unwrap_or(&clang[(0.04 * SR) as usize..])) * 1.5,
            "the burst must sit far deeper than the plate ring"
        );

        let late = (0.6 * SR) as usize..(0.9 * SR) as usize;
        let blast_late = rms(blast.get(late.clone()).unwrap_or(&[]));
        let clang_late = rms(clang.get(late).unwrap_or(&[]));
        assert!(
            blast_late > clang_late,
            "the burst's tail (rumble + debris) must outlast the clang: {blast_late} vs {clang_late}"
        );
    }
}
