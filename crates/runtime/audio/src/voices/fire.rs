//! A burning engine's crackle bed: a fire is a *presence*, not an event, so this voice lives
//! across frames like the engine hum and spools in/out instead of cutting. Two layers, both
//! procedural: a low draft rumble (heavily low-passed noise) and sparse pops — random impulses
//! ringing through a band-pass — that read as burning fuel and paint snapping off hot steel.

use crate::dsp::{Biquad, Noise, OnePoleLowPass};

/// How fast the bed fades in when the engine catches (per-second level rate).
const IGNITE_PER_S: f32 = 2.2;
/// How fast it dies down when the fire is out — slower than the cut, a fire never clicks off.
const QUENCH_PER_S: f32 = 0.9;
/// Average pops per second at full burn.
const POPS_PER_S: f32 = 26.0;

pub struct FireVoice {
    noise: Noise,
    rumble: OnePoleLowPass,
    crackle_band: Biquad,
    pop_env: f32,
    pop_decay_per_sample: f32,
    pop_chance_per_sample: f32,
    level: f32,
    target: f32,
    ignite_per_sample: f32,
    quench_per_sample: f32,
}

impl FireVoice {
    pub fn new(sample_rate_hz: f32, seed: u64) -> Self {
        Self {
            noise: Noise::new(seed ^ 0xF14E_5EED),
            rumble: OnePoleLowPass::new(140.0, sample_rate_hz),
            crackle_band: Biquad::band_pass(1_900.0, 1.1, sample_rate_hz),
            pop_env: 0.0,
            // A pop rings ~35 ms.
            pop_decay_per_sample: (-1.0 / (0.035 * sample_rate_hz)).exp(),
            pop_chance_per_sample: POPS_PER_S / sample_rate_hz,
            level: 0.0,
            target: 0.0,
            ignite_per_sample: IGNITE_PER_S / sample_rate_hz,
            quench_per_sample: QUENCH_PER_S / sample_rate_hz,
        }
    }

    pub fn set_burning(&mut self, burning: bool) {
        self.target = if burning { 1.0 } else { 0.0 };
    }

    pub fn is_silent(&self) -> bool {
        self.target == 0.0 && self.level < 1.0e-4
    }

    /// Mix the bed into a mono buffer at `gain`.
    pub fn render_add(&mut self, out: &mut [f32], gain: f32) {
        if self.is_silent() {
            return;
        }
        for sample in out.iter_mut() {
            self.level = if self.level < self.target {
                (self.level + self.ignite_per_sample).min(self.target)
            } else {
                (self.level - self.quench_per_sample).max(self.target)
            };
            if self.level <= 0.0 {
                continue;
            }
            let raw = self.noise.signed();
            let rumble = self.rumble.process(raw) * 1.6;
            if self.noise.unit() < self.pop_chance_per_sample {
                self.pop_env += 0.4 + self.noise.unit() * 0.6;
            }
            self.pop_env *= self.pop_decay_per_sample;
            let crackle = self.crackle_band.process(raw * self.pop_env) * 2.4;
            *sample += (rumble + crackle) * self.level * gain;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voice::rms;

    const SR: f32 = 48_000.0;

    fn render(voice: &mut FireVoice, seconds: f32) -> Vec<f32> {
        let mut out = vec![0.0; (seconds * SR) as usize];
        voice.render_add(&mut out, 1.0);
        out
    }

    #[test]
    fn a_lit_fire_crackles_audibly_and_finitely() {
        let mut voice = FireVoice::new(SR, 7);
        voice.set_burning(true);
        let out = render(&mut voice, 1.0);
        assert!(out.iter().all(|s| s.is_finite()));
        assert!(rms(&out[out.len() / 2..]) > 1.0e-3, "a burning engine must be heard");
    }

    #[test]
    fn a_quenched_fire_spools_down_to_silence_instead_of_cutting() {
        let mut voice = FireVoice::new(SR, 7);
        voice.set_burning(true);
        render(&mut voice, 1.0);
        voice.set_burning(false);
        let fade = render(&mut voice, 2.0);
        let early = rms(&fade[..4_800]);
        let late = rms(&fade[fade.len() - 4_800..]);
        assert!(early > 1.0e-4, "the fade starts audible (no hard cut)");
        assert!(late < 1.0e-5, "the bed reaches silence: {late}");
        assert!(voice.is_silent());
    }

    #[test]
    fn an_unlit_fire_renders_nothing() {
        let mut voice = FireVoice::new(SR, 7);
        let out = render(&mut voice, 0.25);
        assert!(rms(&out) == 0.0);
        assert!(voice.is_silent());
    }
}
