//! The cannon shot: the single most-heard sound in the game, designed from the physics up.
//! A real tank gun is four overlapping events — a supersonic crack, the muzzle-blast noise
//! wave, the low pressure "body" of the report, and a sub-audible thump you feel more than
//! hear. Each layer is synthesized separately and scaled by caliber, so a 122 mm D-25T booms
//! visibly deeper and longer than an 88 mm KwK 43 without any recorded assets.

use crate::dsp::{Biquad, ExpDecay, Noise, OnePoleLowPass};
use crate::voice::Voice;
use glam::FloatExt;

pub struct CannonShot {
    sample_rate_hz: f32,
    /// Samples rendered so far — drives the body-pitch sweep.
    age_samples: usize,
    noise: Noise,
    /// Supersonic crack: a few milliseconds of high-passed noise, dead almost instantly.
    crack_env: ExpDecay,
    crack_hp: Biquad,
    /// Muzzle blast: broadband noise whose low-pass sweeps shut as the wave expands.
    blast_env: ExpDecay,
    blast_lp: OnePoleLowPass,
    blast_cutoff_begin_hz: f32,
    blast_cutoff_end_hz: f32,
    blast_tau_s: f32,
    /// Report body: a low sine sweeping downward as the muzzle gases cool.
    body_env: ExpDecay,
    body_phase: f32,
    body_begin_hz: f32,
    body_end_hz: f32,
    /// Sub thump: a half-cycle pressure pulse below the body.
    thump_env: ExpDecay,
    thump_phase: f32,
    thump_hz: f32,
}

impl CannonShot {
    /// `caliber_mm` drives every layer's weight and duration; `seed` decorrelates the noise so
    /// two simultaneous shots do not phase-cancel.
    pub fn new(caliber_mm: f32, sample_rate_hz: f32, seed: u64) -> Self {
        // 0 at 75 mm, 1 at 130 mm: the design range of the current gun park.
        let size = ((caliber_mm - 75.0) / 55.0).clamp(0.0, 1.2);
        let body_tau_s = 0.10.lerp(0.24, size);
        Self {
            sample_rate_hz,
            age_samples: 0,
            noise: Noise::new(seed),
            crack_env: ExpDecay::new(0.7.lerp(0.5, size), 0.004, sample_rate_hz),
            crack_hp: Biquad::high_pass(1_800.0, 0.707, sample_rate_hz),
            blast_env: ExpDecay::new(0.8.lerp(1.0, size), 0.06.lerp(0.13, size), sample_rate_hz),
            blast_lp: OnePoleLowPass::new(6_000.0, sample_rate_hz),
            blast_cutoff_begin_hz: 6_000.0,
            blast_cutoff_end_hz: 500.0.lerp(260.0, size),
            blast_tau_s: 0.06.lerp(0.13, size),
            body_env: ExpDecay::new(0.55.lerp(0.95, size), body_tau_s, sample_rate_hz),
            body_phase: 0.0,
            body_begin_hz: 110.0.lerp(68.0, size),
            body_end_hz: 55.0.lerp(34.0, size),
            thump_env: ExpDecay::new(0.35.lerp(0.8, size), 0.09.lerp(0.2, size), sample_rate_hz),
            thump_phase: 0.0,
            thump_hz: 38.0.lerp(26.0, size),
        }
    }
}

impl Voice for CannonShot {
    fn render(&mut self, out: &mut [f32]) -> bool {
        for sample in out.iter_mut() {
            let t = self.age_samples as f32 / self.sample_rate_hz;
            // Blast cutoff sweeps shut on the same clock its envelope decays on.
            let sweep = (-t / self.blast_tau_s).exp();
            let cutoff = self.blast_cutoff_end_hz.lerp(self.blast_cutoff_begin_hz, sweep);
            self.blast_lp.set_cutoff(cutoff, self.sample_rate_hz);

            let crack = self.crack_hp.process(self.noise.signed()) * self.crack_env.step();
            let blast = self.blast_lp.process(self.noise.signed()) * self.blast_env.step();

            let body_hz = self.body_end_hz.lerp(self.body_begin_hz, sweep);
            self.body_phase += std::f32::consts::TAU * body_hz / self.sample_rate_hz;
            let body = self.body_phase.sin() * self.body_env.step();

            self.thump_phase += std::f32::consts::TAU * self.thump_hz / self.sample_rate_hz;
            let thump = self.thump_phase.sin() * self.thump_env.step();

            *sample = crack + blast + body + thump;
            self.age_samples += 1;
        }
        !(self.blast_env.is_quiet()
            && self.body_env.is_quiet()
            && self.thump_env.is_quiet()
            && self.crack_env.is_quiet())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voice::{peak, render_to_vec, rms};

    const SR: f32 = 48_000.0;

    #[test]
    fn a_shot_starts_loud_and_decays_to_silence() {
        let mut shot = CannonShot::new(100.0, SR, 1);
        let wave = render_to_vec(&mut shot, 5 * SR as usize);
        assert!(wave.len() < 5 * SR as usize, "the shot must end on its own");
        assert!(peak(&wave[..2_400]) > 0.5, "the first 50 ms carry the blast");
        let tail = &wave[wave.len().saturating_sub(512)..];
        assert!(peak(tail) < 0.01, "the tail dies out");
        assert!(wave.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn bigger_calibers_boom_longer_and_deeper() {
        let small = render_to_vec(&mut CannonShot::new(76.0, SR, 3), 3 * SR as usize);
        let large = render_to_vec(&mut CannonShot::new(122.0, SR, 3), 3 * SR as usize);

        // The 122 keeps ringing where the 76 has already died.
        let late = SR as usize / 2..SR as usize; // 0.5 s .. 1.0 s
        let small_late = rms(small.get(late.clone()).unwrap_or(&[]));
        let large_late = rms(large.get(late).unwrap_or(&[]));
        assert!(
            large_late > small_late * 1.5,
            "the big gun's tail must outlast the small one: {large_late} vs {small_late}"
        );

        // And a larger share of its energy sits below 150 Hz. (Zero-crossing rate is too crude
        // here — the broadband blast noise dominates the early crossings for both guns.)
        let low_fraction = |wave: &[f32]| {
            let mut lp = crate::dsp::OnePoleLowPass::new(150.0, SR);
            let low: Vec<f32> = wave.iter().map(|s| lp.process(*s)).collect();
            rms(&low) / rms(wave).max(1.0e-9)
        };
        let small_low = low_fraction(&small[..SR as usize / 2]);
        let large_low = low_fraction(&large[..SR as usize / 2]);
        assert!(
            large_low > small_low,
            "the big gun must sound deeper: LF share {large_low} vs {small_low}"
        );
    }

    #[test]
    fn different_seeds_decorrelate_simultaneous_shots() {
        let a = render_to_vec(&mut CannonShot::new(100.0, SR, 1), 4_800);
        let b = render_to_vec(&mut CannonShot::new(100.0, SR, 2), 4_800);
        assert_ne!(a, b, "two guns firing together must not phase-lock");
    }

    #[test]
    fn the_same_seed_replays_bit_exactly() {
        let a = render_to_vec(&mut CannonShot::new(88.0, SR, 9), 9_600);
        let b = render_to_vec(&mut CannonShot::new(88.0, SR, 9), 9_600);
        assert_eq!(a, b);
    }
}
