//! Shop work (Hala v4 R2): the repair beat's own hands. The garage repair used to answer
//! with a bare UI click — a switch toggling, while the nameplate said REPAIRING and the hull
//! rose off its springs. This voice is the work itself: a socket wrench pulled three times,
//! each pull a burst of pawl ticks that accelerates the way a real ratchet does as the nut
//! runs free, closed by a brighter seat-tick when it lands. Deterministic per seed, like
//! every voice in this crate.

use crate::dsp::{Biquad, ExpDecay, Noise};
use crate::voice::Voice;

/// One scheduled pawl tick: when (samples), how bright (Hz), how hard (amp).
struct Tick {
    at: usize,
    hz: f32,
    amp: f32,
}

/// The wrench at work over a fixed beat. The schedule is BUILT at spawn from the beat length
/// and the seed, so the audio spans the repair beat exactly and two repairs with one seed
/// are one waveform.
pub struct RatchetWork {
    sample_rate_hz: f32,
    age_samples: usize,
    ticks: Vec<Tick>,
    next: usize,
    // The live tick being rendered: a high inharmonic partial plus a breath of filtered noise,
    // re-armed at each scheduled tick (the pawl is the same steel every time).
    partial: (f32, f32, ExpDecay),
    noise: Noise,
    noise_env: ExpDecay,
    noise_hp: Biquad,
}

impl RatchetWork {
    pub fn new(seconds: f32, sample_rate_hz: f32, seed: u64) -> Self {
        // A tiny deterministic LCG for schedule jitter — the DSP noise stays for the air.
        let mut state = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        let mut jitter = |spread: f32| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (((state >> 33) as f32 / (1u64 << 31) as f32) - 0.5) * 2.0 * spread
        };

        // Three pulls across the beat (3.2 s -> starts at ~0.15/1.20/2.25 s), each ~12 pawl
        // ticks accelerating 55 -> 36 ms as the nut runs free, closed by a brighter, harder
        // seat-tick. The last pull ends inside the beat so the shop's finishing clunk
        // (queued by `tick_repair`) still has the last word.
        let pulls = 3usize;
        let pull_span = (seconds - 0.35).max(0.6) / pulls as f32;
        let mut ticks = Vec::new();
        for pull in 0..pulls {
            let mut t = pull as f32 * pull_span + 0.15 + jitter(0.03);
            let steps = 12;
            for step in 0..steps {
                let run = step as f32 / (steps - 1) as f32;
                let interval = 0.055 - 0.019 * run;
                t += interval + jitter(0.004);
                let seating = step == steps - 1;
                ticks.push(Tick {
                    at: (t.max(0.0) * sample_rate_hz) as usize,
                    hz: if seating { 2_450.0 } else { 1_650.0 } + jitter(120.0),
                    amp: if seating { 0.30 } else { 0.16 + 0.05 * run },
                });
            }
        }

        Self {
            sample_rate_hz,
            age_samples: 0,
            ticks,
            next: 0,
            partial: (0.0, 0.0, ExpDecay::new(0.0, 0.012, sample_rate_hz)),
            noise: Noise::new(seed),
            noise_env: ExpDecay::new(0.0, 0.003, sample_rate_hz),
            noise_hp: Biquad::high_pass(2_000.0, 0.707, sample_rate_hz),
        }
    }
}

impl Voice for RatchetWork {
    fn render(&mut self, out: &mut [f32]) -> bool {
        for sample in out.iter_mut() {
            if let Some(tick) = self.ticks.get(self.next)
                && self.age_samples >= tick.at
            {
                // Re-arm the pawl: same steel, the scheduled brightness and hand.
                self.partial = (
                    0.0,
                    std::f32::consts::TAU * tick.hz / self.sample_rate_hz,
                    ExpDecay::new(tick.amp, 0.012, self.sample_rate_hz),
                );
                self.noise_env = ExpDecay::new(tick.amp * 0.8, 0.003, self.sample_rate_hz);
                self.next += 1;
            }
            let (phase, step, env) = &mut self.partial;
            *phase += *step;
            let noise = self.noise_hp.process(self.noise.signed()) * self.noise_env.step();
            *sample = phase.sin() * env.step() + noise;
            self.age_samples += 1;
        }
        self.next < self.ticks.len() || !(self.partial.2.is_quiet() && self.noise_env.is_quiet())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voice::{peak, render_to_vec};

    const SR: f32 = 48_000.0;

    /// The work spans the beat it was given: pulls land early, middle and late, and the voice
    /// is OVER shortly after the beat — the finishing clunk keeps the last word.
    #[test]
    fn the_wrench_works_the_whole_beat_and_then_stops() {
        let mut work = RatchetWork::new(3.2, SR, 11);
        let wave = render_to_vec(&mut work, (5.0 * SR) as usize);
        assert!(wave.len() < (3.6 * SR) as usize, "the work ends with the beat");
        for (from, to) in [(0.15, 0.9), (1.2, 2.0), (2.3, 3.2)] {
            let window = peak(&wave[(from * SR) as usize..((to * SR) as usize).min(wave.len())]);
            assert!(window > 0.08, "a pull must land in {from}..{to}s: {window}");
        }
        assert!(peak(&wave) < 0.8, "shop work stays under the battle sounds");
        assert!(wave.iter().all(|s| s.is_finite()));
    }

    /// Deterministic per seed — one seed, one waveform; different seeds, different hands.
    #[test]
    fn the_work_is_deterministic_per_seed() {
        let a = render_to_vec(&mut RatchetWork::new(3.2, SR, 5), (4.0 * SR) as usize);
        let b = render_to_vec(&mut RatchetWork::new(3.2, SR, 5), (4.0 * SR) as usize);
        assert_eq!(a, b, "one seed must be one waveform");
        let c = render_to_vec(&mut RatchetWork::new(3.2, SR, 6), (4.0 * SR) as usize);
        assert_ne!(a, c, "a different seed is a different hand");
    }
}
