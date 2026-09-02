//! The collapse voice (Inny Poziom Z1): a structure coming down is not a shell thud. It is a
//! fracture (a short, bright crack as the first course lets go), a rumble that lasts as long as
//! the mass takes to settle, a SECOND wave a third of a second later when the walls meet the
//! ground, and a rain of masonry that goes on after the rumble has died. Everything scales with
//! the box that fell — an 18 m tenement is not a shed — through `size` (footprint) and `mass`
//! (footprint × height), so the same voice serves a fence and a factory hall.

use crate::dsp::{Biquad, ExpDecay, Noise, OnePoleLowPass};
use crate::voice::Voice;

/// The footprint a `size` of 1 means: a 6 × 10 m barn.
const REFERENCE_FOOTPRINT_M2: f32 = 60.0;
/// The height a `mass` factor of 1 means at size 1.
const REFERENCE_HEIGHT_M: f32 = 8.0;
/// When the second wave — the walls meeting the ground — sets in.
pub const SETTLE_ONSET_S: f32 = 0.35;

pub struct Collapse {
    sample_rate_hz: f32,
    age_samples: usize,
    mass: f32,
    noise: Noise,
    crack_env: ExpDecay,
    crack_hp: Biquad,
    rumble_env: ExpDecay,
    rumble_lp: OnePoleLowPass,
    sub_env: ExpDecay,
    sub_phase: f32,
    settle_start: usize,
    settle_env: ExpDecay,
    /// The second fracture: the walls hitting the ground open bright for a few milliseconds
    /// before the second rumble — what makes the beat read as an EVENT, not a swell.
    settle_crack_env: ExpDecay,
    settle_lp: OnePoleLowPass,
    debris: Vec<(usize, usize)>, // (start_sample, end_sample)
    debris_hp: Biquad,
    last_debris: usize,
}

impl Collapse {
    pub fn new(footprint_m2: f32, height_m: f32, sample_rate_hz: f32, seed: u64) -> Self {
        let size = (footprint_m2 / REFERENCE_FOOTPRINT_M2).clamp(0.3, 3.0);
        let mass = size * (height_m / REFERENCE_HEIGHT_M).clamp(0.5, 2.5);
        let mut noise = Noise::new(seed);
        // Masonry rains from a tenth of a second until the rumble is well into its tail: more
        // of it from a bigger box, each grain 6–40 ms.
        let count = 8 + (size * 30.0) as usize;
        let rain_end_s = 1.2 + 1.2 * mass;
        let mut debris = Vec::with_capacity(count);
        for _ in 0..count {
            let start = ((0.10 + noise.unit() * (rain_end_s - 0.10)) * sample_rate_hz) as usize;
            let length = ((0.006 + noise.unit() * 0.034) * sample_rate_hz) as usize;
            debris.push((start, start + length));
        }
        let last_debris = debris.iter().map(|(_, end)| *end).max().unwrap_or(0);
        Self {
            sample_rate_hz,
            age_samples: 0,
            mass,
            noise,
            crack_env: ExpDecay::new(0.5, 0.012, sample_rate_hz),
            crack_hp: Biquad::high_pass(900.0, 0.707, sample_rate_hz),
            // The first rumble is the top course going; the SECOND wave (below) is the mass
            // meeting the ground and carries most of the energy — the beat, not a swell.
            rumble_env: ExpDecay::new(0.7, 0.5 + 0.35 * mass, sample_rate_hz),
            rumble_lp: OnePoleLowPass::new(140.0, sample_rate_hz),
            sub_env: ExpDecay::new(0.9, 0.5 + 0.4 * mass, sample_rate_hz),
            sub_phase: 0.0,
            settle_start: (SETTLE_ONSET_S * sample_rate_hz) as usize,
            settle_env: ExpDecay::new(2.4, 0.4 + 0.3 * mass, sample_rate_hz),
            settle_crack_env: ExpDecay::new(0.8, 0.02, sample_rate_hz),
            settle_lp: OnePoleLowPass::new(110.0, sample_rate_hz),
            debris,
            debris_hp: Biquad::high_pass(700.0, 0.707, sample_rate_hz),
            last_debris,
        }
    }
}

impl Voice for Collapse {
    fn render(&mut self, out: &mut [f32]) -> bool {
        for sample in out.iter_mut() {
            let t = self.age_samples as f32 / self.sample_rate_hz;
            let raw = self.noise.signed();
            // The fracture: bright and gone in tens of milliseconds.
            let crack = self.crack_hp.process(raw) * self.crack_env.step();
            // The rumble's spectrum closes as the mass settles — slower for a heavier box.
            let sweep = (-t / (0.6 * self.mass)).exp();
            self.rumble_lp.set_cutoff(60.0 + 220.0 * sweep, self.sample_rate_hz);
            let rumble = self.rumble_lp.process(raw) * self.rumble_env.step();
            // The sub: the ground taking the weight, 32 Hz falling to 18 — from the moment the
            // walls land, not before (until then only the top course is moving).
            let sub_hz = 18.0 + 14.0 * sweep;
            self.sub_phase += std::f32::consts::TAU * sub_hz / self.sample_rate_hz;
            let sub = if self.age_samples >= self.settle_start {
                self.sub_phase.sin() * self.sub_env.step()
            } else {
                0.0
            };
            // The second wave: the walls meet the ground a third of a second in.
            let settle = if self.age_samples >= self.settle_start {
                self.settle_lp.process(raw) * self.settle_env.step()
                    + self.crack_hp.process(raw) * self.settle_crack_env.step()
            } else {
                0.0
            };
            let in_grain = self
                .debris
                .iter()
                .any(|(start, end)| self.age_samples >= *start && self.age_samples < *end);
            let debris = if in_grain { self.debris_hp.process(raw) * 0.12 } else { 0.0 };

            *sample = crack + rumble + sub + settle + debris;
            self.age_samples += 1;
        }
        !(self.crack_env.is_quiet()
            && self.rumble_env.is_quiet()
            && self.sub_env.is_quiet()
            && (self.age_samples >= self.settle_start && self.settle_env.is_quiet())
            && self.age_samples > self.last_debris)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voice::{peak, render_to_vec, rms};
    use crate::voices::impact::{GroundImpact, GroundKind};

    const SR: f32 = 48_000.0;

    fn last_audible_s(wave: &[f32]) -> f32 {
        wave.iter().rposition(|s| s.abs() > 0.01).map(|i| i as f32 / SR).unwrap_or(0.0)
    }

    /// A barn coming down sits deeper and lasts longer than a shell thudding into its wall,
    /// and it has its second wave: the signal after the settle onset is louder than just before
    /// it — the walls meeting the ground, not a tail fading out.
    #[test]
    fn a_collapse_outlasts_and_undercuts_a_shell_thud_and_has_its_second_wave() {
        let mut voice = Collapse::new(60.0, 8.0, SR, 7);
        let fall = render_to_vec(&mut voice, 6 * SR as usize);
        let thud = render_to_vec(&mut GroundImpact::new(GroundKind::Soil, SR, 7), 6 * SR as usize);
        assert!(fall.iter().all(|s| s.is_finite()));
        assert!(peak(&fall[fall.len().saturating_sub(256)..]) < 0.01, "the collapse ends");

        let low_share = |wave: &[f32]| {
            let mut lp = crate::dsp::OnePoleLowPass::new(120.0, SR);
            let low: Vec<f32> = wave.iter().map(|s| lp.process(*s)).collect();
            rms(&low) / rms(wave).max(1.0e-9)
        };
        // The whole event, fracture to settle: the collapse sits under the thud.
        let window = (0.05 * SR) as usize..(0.8 * SR) as usize;
        assert!(
            low_share(&fall[window.clone()]) > low_share(&thud[window]) * 1.3,
            "the collapse rumbles under the thud"
        );
        assert!(
            last_audible_s(&fall) > last_audible_s(&thud) + 0.5,
            "the collapse outlasts the thud: {} s vs {} s",
            last_audible_s(&fall),
            last_audible_s(&thud)
        );

        let before = (SETTLE_ONSET_S * SR) as usize;
        let just_before = rms(&fall[before - (0.05 * SR) as usize..before]);
        let just_after = rms(&fall[before..before + (0.05 * SR) as usize]);
        assert!(
            just_after > just_before * 1.15,
            "the second wave lands at {SETTLE_ONSET_S} s: {just_before} -> {just_after}"
        );
    }

    /// A tenement takes longer to come down than a shed: the rumble and the rain scale with the
    /// box, so the theatre is sized by what fell.
    #[test]
    fn a_bigger_box_rumbles_longer() {
        let shed = render_to_vec(&mut Collapse::new(12.0, 3.0, SR, 3), 8 * SR as usize);
        let tenement = render_to_vec(&mut Collapse::new(216.0, 18.0, SR, 3), 8 * SR as usize);
        assert!(
            last_audible_s(&tenement) > last_audible_s(&shed) + 1.0,
            "tenement {} s vs shed {} s",
            last_audible_s(&tenement),
            last_audible_s(&shed)
        );
    }
}
