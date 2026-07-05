//! Impact voices: what a shell sounds like when it stops. Armor answers with a modal clang —
//! a bank of inharmonic partials, the way a struck steel plate actually rings — plus a
//! penetration thunk or a ricochet whine on top. Soil swallows the shell with a low thud and a
//! patter of thrown debris; structures knock woodier with a body resonance.

use crate::dsp::{Biquad, ExpDecay, Noise, OnePoleLowPass};
use crate::voice::Voice;
use glam::FloatExt;

/// Inharmonic partial ratios of a struck plate. A harmonic series would sound like a bell in a
/// church; these clustered, detuned ratios read as dead steel.
const PLATE_PARTIALS: [f32; 5] = [1.0, 2.32, 3.01, 4.27, 5.63];

struct Partial {
    phase: f32,
    step: f32,
    env: ExpDecay,
}

/// A shell striking armor: ring + transient, with the outcome layered on top — penetrations add
/// an interior thunk and a muffled tail, ricochets add a departing whine.
pub struct ArmorHit {
    partials: Vec<Partial>,
    noise: Noise,
    transient_env: ExpDecay,
    transient_hp: Biquad,
    /// Penetration only: the shell dumping its energy inside the hull.
    thunk: Option<(f32, f32, ExpDecay)>, // (phase, step, envelope)
    interior_env: ExpDecay,
    interior_lp: OnePoleLowPass,
    interior_gain: f32,
    /// Ricochet only: a resonant whine sweeping down as the shell departs.
    whine_env: ExpDecay,
    whine_bp: Biquad,
    whine_gain: f32,
    whine_begin_hz: f32,
    whine_end_hz: f32,
    whine_tau_s: f32,
    sample_rate_hz: f32,
    age_samples: usize,
}

impl ArmorHit {
    pub fn new(penetrated: bool, ricocheted: bool, sample_rate_hz: f32, seed: u64) -> Self {
        let mut noise = Noise::new(seed);
        // Every plate rings on its own base note; 320-520 Hz keeps it heavy, not tinny.
        let base_hz = 320.0 + noise.unit() * 200.0;
        let partials = PLATE_PARTIALS
            .iter()
            .enumerate()
            .map(|(i, ratio)| {
                let hz = base_hz * ratio;
                Partial {
                    phase: noise.unit() * std::f32::consts::TAU,
                    step: std::f32::consts::TAU * hz / sample_rate_hz,
                    // Higher partials die faster, exactly like real steel.
                    env: ExpDecay::new(
                        0.5 / (1.0 + i as f32 * 0.8),
                        0.22 / (1.0 + i as f32 * 0.9),
                        sample_rate_hz,
                    ),
                }
            })
            .collect();
        let thunk = penetrated.then(|| {
            (
                0.0,
                std::f32::consts::TAU * 72.0 / sample_rate_hz,
                ExpDecay::new(0.8, 0.16, sample_rate_hz),
            )
        });
        Self {
            partials,
            noise,
            transient_env: ExpDecay::new(0.9, 0.003, sample_rate_hz),
            transient_hp: Biquad::high_pass(900.0, 0.707, sample_rate_hz),
            thunk,
            interior_env: ExpDecay::new(if penetrated { 0.5 } else { 0.0 }, 0.3, sample_rate_hz),
            interior_lp: OnePoleLowPass::new(500.0, sample_rate_hz),
            interior_gain: if penetrated { 1.0 } else { 0.0 },
            whine_env: ExpDecay::new(if ricocheted { 0.5 } else { 0.0 }, 0.18, sample_rate_hz),
            whine_bp: Biquad::band_pass(2_400.0, 14.0, sample_rate_hz),
            whine_gain: if ricocheted { 1.0 } else { 0.0 },
            whine_begin_hz: 2_400.0,
            whine_end_hz: 1_100.0,
            whine_tau_s: 0.14,
            sample_rate_hz,
            age_samples: 0,
        }
    }
}

impl Voice for ArmorHit {
    fn render(&mut self, out: &mut [f32]) -> bool {
        for sample in out.iter_mut() {
            let mut acc = 0.0;
            for partial in &mut self.partials {
                partial.phase += partial.step;
                acc += partial.phase.sin() * partial.env.step();
            }
            acc += self.transient_hp.process(self.noise.signed()) * self.transient_env.step();

            if let Some((phase, step, env)) = &mut self.thunk {
                *phase += *step;
                acc += phase.sin() * env.step();
            }
            if self.interior_gain > 0.0 {
                acc += self.interior_lp.process(self.noise.signed()) * self.interior_env.step();
            }
            if self.whine_gain > 0.0 {
                let t = self.age_samples as f32 / self.sample_rate_hz;
                let hz = self.whine_end_hz.lerp(self.whine_begin_hz, (-t / self.whine_tau_s).exp());
                self.whine_bp = retuned_band_pass(&self.whine_bp, hz, self.sample_rate_hz);
                acc += self.whine_bp.process(self.noise.signed()) * self.whine_env.step() * 3.0;
            }
            *sample = acc;
            self.age_samples += 1;
        }
        !(self.partials.iter().all(|p| p.env.is_quiet())
            && self.transient_env.is_quiet()
            && self.thunk.as_ref().is_none_or(|(_, _, env)| env.is_quiet())
            && self.interior_env.is_quiet()
            && self.whine_env.is_quiet())
    }
}

/// Rebuild a band-pass at a new center while carrying the delay-line state over, so a frequency
/// sweep glides instead of clicking.
fn retuned_band_pass(previous: &Biquad, center_hz: f32, sample_rate_hz: f32) -> Biquad {
    let mut retuned = Biquad::band_pass(center_hz, 14.0, sample_rate_hz);
    retuned.copy_state_from(previous);
    retuned
}

/// What surface a dying shell struck — mirrors the sim's impact surfaces without depending on
/// the game crates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroundKind {
    /// Open terrain: a dull thud and thrown soil.
    Soil,
    /// Static cover / a dead hull: a harder knock with a body resonance.
    Structure,
}

/// A shell the world absorbed: thud + debris patter (soil) or knock + body tone (structure).
pub struct GroundImpact {
    noise: Noise,
    thud_env: ExpDecay,
    thud_lp: OnePoleLowPass,
    /// Structure only: a woody body resonance.
    body_bp: Option<Biquad>,
    body_env: ExpDecay,
    /// Soil only: short high-passed noise grains landing behind the thud.
    debris: Vec<(usize, usize)>, // (start_sample, end_sample)
    debris_hp: Biquad,
    age_samples: usize,
    last_sample: usize,
}

impl GroundImpact {
    pub fn new(kind: GroundKind, sample_rate_hz: f32, seed: u64) -> Self {
        let mut noise = Noise::new(seed);
        let mut debris = Vec::new();
        if kind == GroundKind::Soil {
            // 6-12 clods land 40-350 ms after the strike, each 8-25 ms long.
            let count = 6 + (noise.unit() * 7.0) as usize;
            for _ in 0..count {
                let start = ((0.04 + noise.unit() * 0.31) * sample_rate_hz) as usize;
                let length = ((0.008 + noise.unit() * 0.017) * sample_rate_hz) as usize;
                debris.push((start, start + length));
            }
        }
        let last_debris = debris.iter().map(|(_, end)| *end).max().unwrap_or(0);
        Self {
            noise,
            thud_env: ExpDecay::new(0.9, 0.11, sample_rate_hz),
            thud_lp: OnePoleLowPass::new(
                if kind == GroundKind::Soil { 320.0 } else { 700.0 },
                sample_rate_hz,
            ),
            body_bp: (kind == GroundKind::Structure)
                .then(|| Biquad::band_pass(210.0, 9.0, sample_rate_hz)),
            body_env: ExpDecay::new(
                if kind == GroundKind::Structure { 0.6 } else { 0.0 },
                0.14,
                sample_rate_hz,
            ),
            debris,
            debris_hp: Biquad::high_pass(1_200.0, 0.707, sample_rate_hz),
            age_samples: 0,
            last_sample: last_debris,
        }
    }
}

impl Voice for GroundImpact {
    fn render(&mut self, out: &mut [f32]) -> bool {
        for sample in out.iter_mut() {
            let raw = self.noise.signed();
            let mut acc = self.thud_lp.process(raw) * self.thud_env.step() * 2.2;
            if let Some(bp) = &mut self.body_bp {
                acc += bp.process(raw) * self.body_env.step() * 2.0;
            }
            let in_grain = self
                .debris
                .iter()
                .any(|(start, end)| self.age_samples >= *start && self.age_samples < *end);
            if in_grain {
                acc += self.debris_hp.process(raw) * 0.12;
            }
            *sample = acc;
            self.age_samples += 1;
        }
        !(self.thud_env.is_quiet()
            && self.body_env.is_quiet()
            && self.age_samples > self.last_sample)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voice::{peak, render_to_vec, rms};

    const SR: f32 = 48_000.0;

    #[test]
    fn every_armor_hit_variant_renders_finite_and_dies_out() {
        for (pen, ric) in [(false, false), (true, false), (false, true)] {
            let mut hit = ArmorHit::new(pen, ric, SR, 5);
            let wave = render_to_vec(&mut hit, 4 * SR as usize);
            assert!(wave.len() < 4 * SR as usize, "hit (pen={pen}, ric={ric}) must end");
            assert!(wave.iter().all(|s| s.is_finite()));
            assert!(peak(&wave[..2_400]) > 0.15, "the strike must be audible");
            assert!(peak(&wave[wave.len().saturating_sub(256)..]) < 0.01);
        }
    }

    #[test]
    fn a_penetration_carries_more_low_end_than_a_bounce() {
        let pen = render_to_vec(&mut ArmorHit::new(true, false, SR, 11), SR as usize);
        let bounce = render_to_vec(&mut ArmorHit::new(false, false, SR, 11), SR as usize);
        // The interior thunk keeps ringing after the surface clang died.
        let late = SR as usize / 4..SR as usize / 2;
        assert!(
            rms(pen.get(late.clone()).unwrap_or(&[])) > rms(bounce.get(late).unwrap_or(&[])),
            "the penetration tail must outlast the bounce"
        );
    }

    #[test]
    fn soil_swallows_where_structure_knocks() {
        let soil = render_to_vec(&mut GroundImpact::new(GroundKind::Soil, SR, 3), SR as usize);
        let structure =
            render_to_vec(&mut GroundImpact::new(GroundKind::Structure, SR, 3), SR as usize);
        use crate::voice::zero_crossing_rate_hz;
        let soil_zcr = zero_crossing_rate_hz(&soil[..4_800], SR);
        let structure_zcr = zero_crossing_rate_hz(&structure[..4_800], SR);
        assert!(
            soil_zcr < structure_zcr,
            "soil must sound duller than a structure knock: {soil_zcr} vs {structure_zcr}"
        );
    }

    #[test]
    fn soil_throws_debris_after_the_thud() {
        let mut impact = GroundImpact::new(GroundKind::Soil, SR, 8);
        let wave = render_to_vec(&mut impact, SR as usize);
        // Some grain lands in the 100-400 ms window, well after the 110 ms-tau thud faded.
        let window = &wave[(0.15 * SR) as usize..(0.4 * SR) as usize];
        assert!(peak(window) > 0.02, "debris must patter behind the thud");
    }
}
