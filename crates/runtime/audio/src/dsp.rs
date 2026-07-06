//! The DSP toolbox every procedural voice is built from: deterministic noise, one-pole and
//! biquad filters, exponential envelopes and a soft saturator. Everything here is sample-rate
//! agnostic, allocation-free per sample, and fully deterministic — the same seed renders the
//! same waveform on every run, which is what makes the whole audio stack unit-testable.

/// Deterministic white noise in `[-1, 1)` — splitmix64, the same generator the sim's dispersion
/// and the FX system use, so audio needs no RNG dependency and tests replay bit-exactly.
#[derive(Debug, Clone)]
pub struct Noise {
    state: u64,
}

impl Noise {
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Next uniform sample in `[0, 1)`.
    pub fn unit(&mut self) -> f32 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        value ^= value >> 31;
        ((value >> 40) as f32) / ((1u64 << 24) as f32)
    }

    /// Next uniform sample in `[-1, 1)`.
    pub fn signed(&mut self) -> f32 {
        self.unit() * 2.0 - 1.0
    }
}

/// One-pole low-pass: the cheapest "make it duller" filter. Distance air absorption, exhaust
/// tone and envelope smoothing all run through this.
#[derive(Debug, Clone)]
pub struct OnePoleLowPass {
    coefficient: f32,
    state: f32,
}

impl OnePoleLowPass {
    pub fn new(cutoff_hz: f32, sample_rate_hz: f32) -> Self {
        let mut filter = Self { coefficient: 0.0, state: 0.0 };
        filter.set_cutoff(cutoff_hz, sample_rate_hz);
        filter
    }

    /// Retune without resetting state — cutoff sweeps stay click-free.
    pub fn set_cutoff(&mut self, cutoff_hz: f32, sample_rate_hz: f32) {
        // Standard one-pole mapping: a = exp(-2*pi*fc/fs); clamp keeps the filter stable when a
        // sweep asks for a cutoff above Nyquist.
        let ratio = (cutoff_hz / sample_rate_hz).clamp(1.0e-5, 0.499);
        self.coefficient = (-std::f32::consts::TAU * ratio).exp();
    }

    pub fn process(&mut self, input: f32) -> f32 {
        self.state = input + (self.state - input) * self.coefficient;
        self.state
    }
}

/// The classic RBJ biquad, used here as band-pass (modal partials, resonant thumps) and
/// high-pass (shot crack, track clatter).
#[derive(Debug, Clone, Default)]
pub struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl Biquad {
    /// Constant-skirt band-pass centred on `center_hz`; `q` sets how long the ring lasts.
    pub fn band_pass(center_hz: f32, q: f32, sample_rate_hz: f32) -> Self {
        let omega = std::f32::consts::TAU * (center_hz / sample_rate_hz).clamp(1.0e-5, 0.499);
        let alpha = omega.sin() / (2.0 * q.max(0.05));
        let a0 = 1.0 + alpha;
        Self {
            b0: alpha / a0,
            b1: 0.0,
            b2: -alpha / a0,
            a1: -2.0 * omega.cos() / a0,
            a2: (1.0 - alpha) / a0,
            ..Self::default()
        }
    }

    /// Butterworth-ish high-pass at `cutoff_hz`.
    pub fn high_pass(cutoff_hz: f32, q: f32, sample_rate_hz: f32) -> Self {
        let omega = std::f32::consts::TAU * (cutoff_hz / sample_rate_hz).clamp(1.0e-5, 0.499);
        let (sin_o, cos_o) = omega.sin_cos();
        let alpha = sin_o / (2.0 * q.max(0.05));
        let a0 = 1.0 + alpha;
        Self {
            b0: (1.0 + cos_o) / (2.0 * a0),
            b1: -(1.0 + cos_o) / a0,
            b2: (1.0 + cos_o) / (2.0 * a0),
            a1: -2.0 * cos_o / a0,
            a2: (1.0 - alpha) / a0,
            ..Self::default()
        }
    }

    /// Carry the delay-line state from `previous` into this (re-tuned) biquad, so a coefficient
    /// sweep glides instead of clicking on every retune.
    pub fn copy_state_from(&mut self, previous: &Self) {
        self.x1 = previous.x1;
        self.x2 = previous.x2;
        self.y1 = previous.y1;
        self.y2 = previous.y2;
    }

    pub fn process(&mut self, input: f32) -> f32 {
        let output = self.b0 * input + self.b1 * self.x1 + self.b2 * self.x2
            - self.a1 * self.y1
            - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = input;
        self.y2 = self.y1;
        self.y1 = output;
        output
    }
}

/// Exponential decay envelope: `amplitude * exp(-t / tau)`. The workhorse of every percussive
/// voice — gun bodies, modal partials, thuds and clicks all die exponentially.
#[derive(Debug, Clone)]
pub struct ExpDecay {
    level: f32,
    decay_per_sample: f32,
}

impl ExpDecay {
    pub fn new(amplitude: f32, tau_s: f32, sample_rate_hz: f32) -> Self {
        let tau_samples = (tau_s * sample_rate_hz).max(1.0);
        Self { level: amplitude, decay_per_sample: (-1.0 / tau_samples).exp() }
    }

    /// Current level, then decay one sample.
    pub fn step(&mut self) -> f32 {
        let level = self.level;
        self.level *= self.decay_per_sample;
        level
    }

    /// Envelopes below audibility count as finished; voices use this to free their slot.
    pub fn is_quiet(&self) -> bool {
        self.level < 1.0e-4
    }
}

/// Soft saturation that stays linear-ish quiet and rounds off loud peaks instead of clipping
/// them square — the master bus safety so a 14-gun barrage crunches instead of cracking.
pub fn soft_clip(sample: f32) -> f32 {
    // Cubic soft clipper: identity slope at zero, hard bound at +/-1 past |x| = 1.5.
    let x = sample.clamp(-1.5, 1.5);
    x - x * x * x / 6.75
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_is_deterministic_and_bounded() {
        let mut a = Noise::new(7);
        let mut b = Noise::new(7);
        for _ in 0..256 {
            let sample = a.signed();
            assert_eq!(sample, b.signed());
            assert!((-1.0..1.0).contains(&sample));
        }
    }

    #[test]
    fn different_seeds_produce_different_streams() {
        let mut a = Noise::new(1);
        let mut b = Noise::new(2);
        let identical = (0..64).all(|_| a.unit() == b.unit());
        assert!(!identical, "two seeds must not replay the same waveform");
    }

    #[test]
    fn one_pole_low_pass_attenuates_alternating_input_but_passes_dc() {
        let sample_rate = 48_000.0;
        let mut lp = OnePoleLowPass::new(500.0, sample_rate);
        // DC settles to the input value.
        let mut dc = 0.0;
        for _ in 0..4_000 {
            dc = lp.process(1.0);
        }
        assert!((dc - 1.0).abs() < 1.0e-3, "DC must pass, got {dc}");

        // Nyquist-rate alternation (24 kHz) comes out tiny.
        let mut lp = OnePoleLowPass::new(500.0, sample_rate);
        let mut peak: f32 = 0.0;
        for i in 0..4_000 {
            let x = if i % 2 == 0 { 1.0 } else { -1.0 };
            if i > 2_000 {
                peak = peak.max(lp.process(x).abs());
            } else {
                lp.process(x);
            }
        }
        assert!(peak < 0.05, "high frequencies must be crushed, peak {peak}");
    }

    #[test]
    fn band_pass_rings_at_its_center_after_an_impulse() {
        let sample_rate = 48_000.0;
        let center = 440.0;
        let mut bp = Biquad::band_pass(center, 12.0, sample_rate);
        let mut out = Vec::with_capacity(4_800);
        out.push(bp.process(1.0));
        for _ in 1..4_800 {
            out.push(bp.process(0.0));
        }
        // Count zero crossings over the ring: frequency = crossings / 2 / duration.
        let crossings = out.windows(2).filter(|w| (w[0] >= 0.0) != (w[1] >= 0.0)).count();
        let duration_s = out.len() as f32 / sample_rate;
        let estimated_hz = crossings as f32 / 2.0 / duration_s;
        assert!(
            (estimated_hz - center).abs() < center * 0.1,
            "impulse response should ring near {center} Hz, estimated {estimated_hz}"
        );
    }

    #[test]
    fn high_pass_blocks_dc() {
        let mut hp = Biquad::high_pass(800.0, 0.707, 48_000.0);
        let mut settled = 1.0;
        for _ in 0..8_000 {
            settled = hp.process(1.0);
        }
        assert!(settled.abs() < 1.0e-3, "DC must be blocked, got {settled}");
    }

    #[test]
    fn exp_decay_halves_roughly_every_ln2_tau_and_reports_quiet() {
        let sample_rate = 48_000.0;
        let tau_s = 0.1;
        let mut env = ExpDecay::new(1.0, tau_s, sample_rate);
        let mut level_at_tau = 0.0;
        for _ in 0..(tau_s * sample_rate) as usize {
            level_at_tau = env.step();
        }
        assert!(
            (level_at_tau - (-1.0f32).exp()).abs() < 0.01,
            "after one tau the level should be ~1/e, got {level_at_tau}"
        );
        for _ in 0..(sample_rate as usize) {
            env.step();
        }
        assert!(env.is_quiet(), "a decayed envelope must report quiet");
    }

    #[test]
    fn soft_clip_is_gentle_below_one_and_bounded_above() {
        assert!((soft_clip(0.1) - 0.1).abs() < 0.001, "quiet samples pass nearly untouched");
        assert!(soft_clip(3.0) <= 1.0);
        assert!(soft_clip(-3.0) >= -1.0);
        // Monotone through the knee: no waveform folding.
        let mut previous = soft_clip(-1.5);
        let mut x = -1.5;
        while x < 1.5 {
            x += 0.01;
            let y = soft_clip(x);
            assert!(y >= previous, "soft clip must be monotone at {x}");
            previous = y;
        }
    }
}
