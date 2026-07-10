//! Interface cues, kept in the game's material language: dry, mechanical, steel-on-steel.
//! No synth beeps — the gun-ready cue is a breech block seating, the kill confirmation a
//! double low thud, the garage click a switch toggle.

use crate::dsp::{Biquad, ExpDecay, Noise, OnePoleLowPass};
use crate::voice::Voice;

/// A short metallic tick from a pair of high inharmonic partials plus a breath of noise.
/// `bright` picks the register: the gun-ready breech clack sits higher than the garage switch.
pub struct MechanicalClick {
    partial_a: (f32, f32, ExpDecay), // (phase, step, envelope)
    partial_b: (f32, f32, ExpDecay),
    noise: Noise,
    noise_env: ExpDecay,
    noise_hp: Biquad,
}

impl MechanicalClick {
    pub fn new(bright: bool, sample_rate_hz: f32, seed: u64) -> Self {
        let (a_hz, b_hz) = if bright { (2_100.0, 3_350.0) } else { (900.0, 1_460.0) };
        Self {
            partial_a: (
                0.0,
                std::f32::consts::TAU * a_hz / sample_rate_hz,
                ExpDecay::new(0.30, 0.030, sample_rate_hz),
            ),
            partial_b: (
                0.0,
                std::f32::consts::TAU * b_hz / sample_rate_hz,
                ExpDecay::new(0.22, 0.018, sample_rate_hz),
            ),
            noise: Noise::new(seed),
            noise_env: ExpDecay::new(0.25, 0.004, sample_rate_hz),
            noise_hp: Biquad::high_pass(1_500.0, 0.707, sample_rate_hz),
        }
    }
}

impl Voice for MechanicalClick {
    fn render(&mut self, out: &mut [f32]) -> bool {
        for sample in out.iter_mut() {
            let (phase_a, step_a, env_a) = &mut self.partial_a;
            *phase_a += *step_a;
            let (phase_b, step_b, env_b) = &mut self.partial_b;
            *phase_b += *step_b;
            let noise = self.noise_hp.process(self.noise.signed()) * self.noise_env.step();
            *sample = phase_a.sin() * env_a.step() + phase_b.sin() * env_b.step() + noise;
        }
        !(self.partial_a.2.is_quiet() && self.partial_b.2.is_quiet() && self.noise_env.is_quiet())
    }
}

/// A fit the garage REFUSED: a dead, damped double-knock in a low register — the switch that
/// did not engage. Deliberately duller than [`MechanicalClick`] (no bright partials, heavier
/// damping) so acceptance and refusal are told apart by ear alone.
pub struct RejectedThunk {
    age_samples: usize,
    knock: (f32, f32, ExpDecay),
    second: (f32, f32, ExpDecay),
    second_start: usize,
    noise: Noise,
    noise_env: ExpDecay,
    noise_lp: OnePoleLowPass,
}

impl RejectedThunk {
    pub fn new(sample_rate_hz: f32, seed: u64) -> Self {
        Self {
            age_samples: 0,
            knock: (
                0.0,
                std::f32::consts::TAU * 210.0 / sample_rate_hz,
                ExpDecay::new(0.40, 0.028, sample_rate_hz),
            ),
            // The refusal repeats a shade LOWER — the opposite contour of the kill thud's rise.
            second: (
                0.0,
                std::f32::consts::TAU * 165.0 / sample_rate_hz,
                ExpDecay::new(0.34, 0.034, sample_rate_hz),
            ),
            second_start: (0.07 * sample_rate_hz) as usize,
            noise: Noise::new(seed),
            noise_env: ExpDecay::new(0.12, 0.006, sample_rate_hz),
            noise_lp: OnePoleLowPass::new(900.0, sample_rate_hz),
        }
    }
}

impl Voice for RejectedThunk {
    fn render(&mut self, out: &mut [f32]) -> bool {
        for sample in out.iter_mut() {
            let (phase, step, env) = &mut self.knock;
            *phase += *step;
            let mut acc = phase.sin() * env.step();
            if self.age_samples >= self.second_start {
                let (phase, step, env) = &mut self.second;
                *phase += *step;
                acc += phase.sin() * env.step();
            }
            acc += self.noise_lp.process(self.noise.signed()) * self.noise_env.step();
            *sample = acc;
            self.age_samples += 1;
        }
        !(self.knock.2.is_quiet()
            && self.noise_env.is_quiet()
            && (self.second.2.is_quiet() || self.age_samples < self.second_start))
    }
}

/// The kill confirmation: two muffled low thuds, the second a beat later and a shade higher —
/// felt in the chest, not sung in the ear.
pub struct DoubleThud {
    age_samples: usize,
    first: (f32, f32, ExpDecay),
    second: (f32, f32, ExpDecay),
    second_start: usize,
}

impl DoubleThud {
    pub fn new(sample_rate_hz: f32) -> Self {
        Self {
            age_samples: 0,
            first: (
                0.0,
                std::f32::consts::TAU * 88.0 / sample_rate_hz,
                ExpDecay::new(0.55, 0.10, sample_rate_hz),
            ),
            second: (
                0.0,
                std::f32::consts::TAU * 118.0 / sample_rate_hz,
                ExpDecay::new(0.45, 0.12, sample_rate_hz),
            ),
            second_start: (0.13 * sample_rate_hz) as usize,
        }
    }
}

impl Voice for DoubleThud {
    fn render(&mut self, out: &mut [f32]) -> bool {
        for sample in out.iter_mut() {
            let (phase, step, env) = &mut self.first;
            *phase += *step;
            let mut acc = phase.sin() * env.step();
            if self.age_samples >= self.second_start {
                let (phase, step, env) = &mut self.second;
                *phase += *step;
                acc += phase.sin() * env.step();
            }
            *sample = acc;
            self.age_samples += 1;
        }
        !(self.first.2.is_quiet()
            && (self.second.2.is_quiet() || self.age_samples < self.second_start))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voice::{peak, render_to_vec};

    const SR: f32 = 48_000.0;

    #[test]
    fn clicks_are_short_quiet_and_finite() {
        for bright in [true, false] {
            let mut click = MechanicalClick::new(bright, SR, 2);
            let wave = render_to_vec(&mut click, SR as usize);
            assert!(wave.len() < (0.6 * SR) as usize, "a click must be over fast");
            assert!(peak(&wave) < 0.8, "UI cues stay under the battle sounds");
            assert!(peak(&wave[..480]) > 0.1, "but must still be heard");
            assert!(wave.iter().all(|s| s.is_finite()));
        }
    }

    #[test]
    fn the_rejection_is_a_dull_double_knock_distinct_from_the_click() {
        let mut thunk = RejectedThunk::new(SR, 7);
        let wave = render_to_vec(&mut thunk, SR as usize);
        assert!(wave.len() < (0.6 * SR) as usize, "the refusal is over fast");
        assert!(peak(&wave) < 0.8, "UI cues stay under the battle sounds");
        assert!(peak(&wave[..480]) > 0.1, "the first knock lands immediately");
        // The second knock rises over the first's decay — the double contour IS the message.
        let trough = peak(&wave[(0.05 * SR) as usize..(0.068 * SR) as usize]);
        let second = peak(&wave[(0.07 * SR) as usize..(0.12 * SR) as usize]);
        assert!(second > trough, "the refusal knocks twice: {second} vs trough {trough}");
        assert!(wave.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn the_kill_thud_lands_twice() {
        let mut thud = DoubleThud::new(SR);
        let wave = render_to_vec(&mut thud, SR as usize);
        let first_window = peak(&wave[..(0.05 * SR) as usize]);
        // The trough between the beats, just before the second lands.
        let trough = peak(&wave[(0.10 * SR) as usize..(0.128 * SR) as usize]);
        let second_window = peak(&wave[(0.13 * SR) as usize..(0.19 * SR) as usize]);
        assert!(first_window > 0.3, "first beat");
        assert!(second_window > trough * 1.5, "the second beat must rise over the decay");
    }
}
