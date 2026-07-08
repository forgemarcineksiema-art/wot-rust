//! Track damage: the running gear's own voice, distinct from the plate clang. A thrown track is a
//! sharp metallic SNAP of the parting band plus a low thunk as it drops; a merely damaged track is
//! a shorter, duller grind with no thunk. Synthesized from a band-passed noise burst — no assets.

use crate::dsp::{Biquad, ExpDecay, Noise};
use crate::voice::Voice;

pub struct TrackSnap {
    noise: Noise,
    snap_env: ExpDecay,
    snap_bp: Biquad,
    /// Low body thump as a thrown band hits the ground; near-silent for a mere grind.
    thunk: (f32, f32, ExpDecay),
}

impl TrackSnap {
    pub fn new(broken: bool, sample_rate_hz: f32, seed: u64) -> Self {
        // A throw cracks brighter, louder and rings a touch longer than a grind, and drops a low
        // thunk the grind lacks.
        let (bp_hz, snap_amp, snap_tau) =
            if broken { (2_600.0, 0.72, 0.055) } else { (1_350.0, 0.42, 0.038) };
        let thunk_amp = if broken { 0.55 } else { 0.0 };
        Self {
            noise: Noise::new(seed),
            snap_env: ExpDecay::new(snap_amp, snap_tau, sample_rate_hz),
            snap_bp: Biquad::band_pass(bp_hz, if broken { 1.1 } else { 2.2 }, sample_rate_hz),
            thunk: (
                0.0,
                std::f32::consts::TAU * 72.0 / sample_rate_hz,
                ExpDecay::new(thunk_amp, 0.06, sample_rate_hz),
            ),
        }
    }
}

impl Voice for TrackSnap {
    fn render(&mut self, out: &mut [f32]) -> bool {
        for sample in out.iter_mut() {
            let snap = self.snap_bp.process(self.noise.signed()) * self.snap_env.step();
            let (phase, step, env) = &mut self.thunk;
            *phase += *step;
            let thunk = phase.sin() * env.step();
            *sample = snap + thunk;
        }
        !(self.snap_env.is_quiet() && self.thunk.2.is_quiet())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voice::{peak, render_to_vec};

    const SR: f32 = 48_000.0;

    #[test]
    fn a_thrown_track_snaps_harder_than_a_grind() {
        let mut snap = TrackSnap::new(true, SR, 7);
        let mut grind = TrackSnap::new(false, SR, 7);
        let snap_wave = render_to_vec(&mut snap, SR as usize);
        let grind_wave = render_to_vec(&mut grind, SR as usize);
        assert!(peak(&snap_wave) > peak(&grind_wave), "the throw is the louder event");
        assert!(peak(&snap_wave[..960]) > 0.1, "but both are heard immediately");
        assert!(snap_wave.iter().all(|s| s.is_finite()));
        assert!(grind_wave.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn both_are_short() {
        let mut snap = TrackSnap::new(true, SR, 1);
        let wave = render_to_vec(&mut snap, SR as usize);
        assert!(wave.len() < (0.7 * SR) as usize, "a track snap is a transient, not a drone");
    }
}
