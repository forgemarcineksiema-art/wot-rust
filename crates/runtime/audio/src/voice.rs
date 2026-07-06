//! The voice contract: a voice is a self-contained mono sound that renders itself sample by
//! sample and reports when it has died out. Spatialization (gain, pan, air absorption, distance
//! delay) is NOT the voice's job — the mixer owns that, so every design stays a pure timbre.

/// One mono procedural sound. `render` overwrites `out` completely and returns `true` while the
/// voice is still audible; returning `false` frees the slot (any samples written on the final
/// call are still played).
pub trait Voice: Send {
    fn render(&mut self, out: &mut [f32]) -> bool;
}

/// Convenience for tests and offline inspection: run a voice to completion (or `max_samples`)
/// and collect the waveform.
pub fn render_to_vec(voice: &mut dyn Voice, max_samples: usize) -> Vec<f32> {
    let mut collected = Vec::new();
    let mut chunk = [0.0f32; 256];
    while collected.len() < max_samples {
        let alive = voice.render(&mut chunk);
        collected.extend_from_slice(&chunk);
        if !alive {
            break;
        }
    }
    collected.truncate(max_samples);
    collected
}

/// Peak absolute amplitude of a rendered waveform — the loudness bound tests assert on.
pub fn peak(samples: &[f32]) -> f32 {
    samples.iter().fold(0.0f32, |acc, s| acc.max(s.abs()))
}

/// Root-mean-square level over a window — the energy measure tests compare across time.
pub fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
}

/// Zero-crossing rate per second at `sample_rate_hz` — a crude but dependency-free brightness
/// probe: duller waveforms cross zero less often.
pub fn zero_crossing_rate_hz(samples: &[f32], sample_rate_hz: f32) -> f32 {
    if samples.len() < 2 {
        return 0.0;
    }
    let crossings = samples.windows(2).filter(|w| (w[0] >= 0.0) != (w[1] >= 0.0)).count();
    crossings as f32 * sample_rate_hz / samples.len() as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Chirp {
        remaining: usize,
    }

    impl Voice for Chirp {
        fn render(&mut self, out: &mut [f32]) -> bool {
            for sample in out.iter_mut() {
                *sample = if self.remaining > 0 { 0.5 } else { 0.0 };
                self.remaining = self.remaining.saturating_sub(1);
            }
            self.remaining > 0
        }
    }

    #[test]
    fn render_to_vec_stops_when_the_voice_dies() {
        let mut voice = Chirp { remaining: 100 };
        let collected = render_to_vec(&mut voice, 48_000);
        assert!(collected.len() < 48_000, "a dead voice must stop the collection");
        assert!(peak(&collected[..100]) > 0.4);
    }

    #[test]
    fn analysis_helpers_measure_a_known_sine() {
        let sample_rate = 48_000.0;
        let hz = 1_000.0;
        let sine: Vec<f32> = (0..48_000)
            .map(|i| (std::f32::consts::TAU * hz * i as f32 / sample_rate).sin())
            .collect();
        assert!((peak(&sine) - 1.0).abs() < 1.0e-3);
        assert!((rms(&sine) - std::f32::consts::FRAC_1_SQRT_2).abs() < 1.0e-3);
        let zcr = zero_crossing_rate_hz(&sine, sample_rate);
        assert!((zcr - 2.0 * hz).abs() < 20.0, "a sine crosses zero twice per cycle, got {zcr}");
    }
}
