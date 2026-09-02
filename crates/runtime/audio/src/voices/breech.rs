//! The mechanical layer of the player's own shot (Inny Poziom S4): what the crew hears AFTER
//! the report — the tube sliding back in its cradle, the recoil buffer taking the end of the
//! stroke, the recuperator running the gun out to battery, the semi-automatic breech dropping
//! open and the spent case ringing out onto the floor. The report says a gun fired; this says
//! a MACHINE fired it. Every timing scales with caliber the way the report already does: a
//! 128 mm strokes longer, runs out slower and drops a heavier case that rings lower.

use crate::dsp::{Biquad, ExpDecay, Noise, OnePoleLowPass};
use crate::voice::Voice;
use glam::FloatExt;

/// One struck partial: a sine that starts decaying the sample the cycle reaches `start`.
struct Strike {
    phase: f32,
    step: f32,
    env: ExpDecay,
    start: usize,
}

impl Strike {
    fn new(hz: f32, amplitude: f32, tau_s: f32, start: usize, sample_rate_hz: f32) -> Self {
        Self {
            phase: 0.0,
            step: std::f32::consts::TAU * hz / sample_rate_hz,
            env: ExpDecay::new(amplitude, tau_s, sample_rate_hz),
            start,
        }
    }

    fn sample(&mut self, age: usize) -> f32 {
        if age < self.start {
            return 0.0;
        }
        self.phase += self.step;
        self.phase.sin() * self.env.step()
    }
}

/// A breath of high-passed noise released at `start` — the metal-on-metal edge of a strike.
struct Burst {
    env: ExpDecay,
    start: usize,
}

impl Burst {
    fn new(amplitude: f32, tau_s: f32, start: usize, sample_rate_hz: f32) -> Self {
        Self { env: ExpDecay::new(amplitude, tau_s, sample_rate_hz), start }
    }

    fn gain(&mut self, age: usize) -> f32 {
        if age < self.start { 0.0 } else { self.env.step() }
    }
}

/// The cycle's moments in seconds after the trigger — the contract the tests and any
/// picture that wants to move with the sound (a shell case, a breech block) read.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BreechTimeline {
    /// The tube stops sliding back: the buffer's thud.
    pub stroke_end_s: f32,
    /// The tube is back in battery: the clack.
    pub battery_s: f32,
    /// The breech drops and the case leaves it: the ring.
    pub eject_s: f32,
    /// The case meets the floor.
    pub floor_s: f32,
}

pub struct BreechCycle {
    sample_rate_hz: f32,
    age: usize,
    timeline: BreechTimeline,
    end: usize,
    noise: Noise,
    /// The recoil stroke: the tube sliding on its rails — a narrow, resonant band of noise (a
    /// scrape with a pitch) swelling and dying inside the stroke.
    slide_bp: Biquad,
    /// The rails do not hiss: the noise is dulled before it rings in the band.
    slide_lp: OnePoleLowPass,
    slide_gain: f32,
    /// The run-out: the recuperator's hydraulic hiss under the returning tube.
    hiss_lp: OnePoleLowPass,
    hiss_gain: f32,
    burst_hp: Biquad,
    bursts: [Burst; 2],
    strikes: [Strike; 11],
}

impl BreechCycle {
    /// `caliber_mm` sets every timing and the case's pitch; `seed` decorrelates the noise.
    pub fn new(caliber_mm: f32, sample_rate_hz: f32, seed: u64) -> Self {
        // 0 at 75 mm, 1 at 130 mm: the design range of the current gun park.
        let size = ((caliber_mm - 75.0) / 55.0).clamp(0.0, 1.2);
        let stroke_s = 0.09.lerp(0.16, size);
        let battery_s = stroke_s + 0.28.lerp(0.46, size);
        let eject_s = battery_s + 0.04;
        let floor_s = eject_s + 0.14.lerp(0.22, size);
        let bounce_s = floor_s + 0.07;
        let at = |t_s: f32| (t_s * sample_rate_hz) as usize;
        let ring_hz = 2_350.0.lerp(1_450.0, size);
        let ring_tau_s = 0.12.lerp(0.20, size);
        let strike = |hz: f32, amplitude: f32, tau_s: f32, t_s: f32| {
            Strike::new(hz, amplitude, tau_s, at(t_s), sample_rate_hz)
        };
        Self {
            sample_rate_hz,
            age: 0,
            timeline: BreechTimeline { stroke_end_s: stroke_s, battery_s, eject_s, floor_s },
            end: at(bounce_s + 0.30),
            noise: Noise::new(seed),
            slide_bp: Biquad::band_pass(900.0.lerp(620.0, size), 3.5, sample_rate_hz),
            slide_lp: OnePoleLowPass::new(1_200.0, sample_rate_hz),
            slide_gain: 0.55.lerp(0.75, size),
            hiss_lp: OnePoleLowPass::new(1_400.0, sample_rate_hz),
            hiss_gain: 0.05,
            burst_hp: Biquad::high_pass(1_500.0, 0.707, sample_rate_hz),
            bursts: [
                Burst::new(0.20, 0.004, at(battery_s), sample_rate_hz),
                Burst::new(0.14, 0.004, at(floor_s), sample_rate_hz),
            ],
            strikes: [
                // The buffer takes the stroke: a dull, low thud through the mount.
                strike(320.0, 0.22, 0.035, stroke_s),
                strike(505.0, 0.12, 0.020, stroke_s),
                // Battery: the tube seats — the brightest metal of the cycle.
                strike(1_900.0, 0.28, 0.022, battery_s),
                strike(3_000.0, 0.18, 0.014, battery_s),
                // The case leaves the breech ringing: an inharmonic cluster, lower per caliber.
                strike(ring_hz, 0.20, ring_tau_s, eject_s),
                strike(ring_hz * 1.62, 0.12, ring_tau_s * 0.7, eject_s),
                strike(ring_hz * 2.37, 0.07, ring_tau_s * 0.5, eject_s),
                // The floor, and one bounce.
                strike(760.0, 0.18, 0.030, floor_s),
                strike(1_180.0, 0.10, 0.020, floor_s),
                strike(760.0, 0.08, 0.020, bounce_s),
                strike(ring_hz, 0.05, ring_tau_s * 0.4, bounce_s),
            ],
        }
    }

    pub fn timeline(&self) -> BreechTimeline {
        self.timeline
    }
}

impl Voice for BreechCycle {
    fn render(&mut self, out: &mut [f32]) -> bool {
        let stroke_end = (self.timeline.stroke_end_s * self.sample_rate_hz) as usize;
        let battery = (self.timeline.battery_s * self.sample_rate_hz) as usize;
        for sample in out.iter_mut() {
            let age = self.age;
            let noise = self.noise.signed();
            // The stroke swells and dies inside its own window; the run-out hisses inside its.
            let slide = if age < stroke_end {
                let u = age as f32 / stroke_end as f32;
                self.slide_bp.process(self.slide_lp.process(noise))
                    * self.slide_gain
                    * (std::f32::consts::PI * u).sin()
            } else {
                0.0
            };
            let hiss = if (stroke_end..battery).contains(&age) {
                let u = (age - stroke_end) as f32 / (battery - stroke_end) as f32;
                self.hiss_lp.process(noise) * self.hiss_gain * (std::f32::consts::PI * u).sin()
            } else {
                0.0
            };
            let burst_gain: f32 = self.bursts.iter_mut().map(|burst| burst.gain(age)).sum();
            let burst = self.burst_hp.process(noise) * burst_gain;
            let struck: f32 = self.strikes.iter_mut().map(|strike| strike.sample(age)).sum();
            *sample = slide + hiss + burst + struck;
            self.age += 1;
        }
        self.age < self.end
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voice::{peak, render_to_vec, rms, zero_crossing_rate_hz};

    const SR: f32 = 48_000.0;

    fn window(wave: &[f32], from_s: f32, to_s: f32) -> &[f32] {
        &wave[(from_s * SR) as usize..((to_s * SR) as usize).min(wave.len())]
    }

    #[test]
    fn the_cycle_ends_on_its_own_and_stays_finite() {
        let mut cycle = BreechCycle::new(100.0, SR, 1);
        let wave = render_to_vec(&mut cycle, 5 * SR as usize);
        assert!(wave.len() < 2 * SR as usize, "a cycle is over inside two seconds");
        assert!(wave.iter().all(|s| s.is_finite()));
        assert!(peak(&wave) < 1.0, "the mechanics sit under the report, never clip");
    }

    /// The order of the machine: stroke, buffer, run-out, battery, then the case. The case is
    /// the brightest thing in the cycle and it comes AFTER the tube is back in battery; the
    /// battery clack stands out of the run-out hiss it ends.
    #[test]
    fn the_case_rings_after_the_tube_is_back_in_battery() {
        let mut cycle = BreechCycle::new(100.0, SR, 3);
        let timeline = cycle.timeline();
        let wave = render_to_vec(&mut cycle, 5 * SR as usize);
        let stroke = window(&wave, 0.0, timeline.stroke_end_s);
        let run_out = window(&wave, timeline.stroke_end_s + 0.08, timeline.battery_s - 0.01);
        let battery = window(&wave, timeline.battery_s, timeline.battery_s + 0.02);
        let ring = window(&wave, timeline.eject_s, timeline.eject_s + 0.05);
        assert!(rms(stroke) > 0.01, "the stroke is audible");
        assert!(
            peak(battery) > peak(run_out) * 3.0,
            "battery {} stands out of the run-out {}",
            peak(battery),
            peak(run_out)
        );
        assert!(
            zero_crossing_rate_hz(ring, SR) > zero_crossing_rate_hz(stroke, SR) * 1.8,
            "the case rings brighter than the tube slides: ring {} Hz, slide {} Hz",
            zero_crossing_rate_hz(ring, SR),
            zero_crossing_rate_hz(stroke, SR)
        );
        assert!(
            timeline.eject_s > timeline.battery_s && timeline.battery_s > timeline.stroke_end_s
        );
    }

    #[test]
    fn a_bigger_gun_cycles_longer_and_drops_a_lower_case() {
        let mut small = BreechCycle::new(75.0, SR, 5);
        let mut big = BreechCycle::new(128.0, SR, 5);
        let small_line = small.timeline();
        let big_line = big.timeline();
        let small_wave = render_to_vec(&mut small, 5 * SR as usize);
        let big_wave = render_to_vec(&mut big, 5 * SR as usize);
        assert!(big_wave.len() as f32 > small_wave.len() as f32 * 1.2, "a 128 mm cycles longer");
        assert!(big_line.floor_s > small_line.floor_s + 0.25);
        let small_ring = window(&small_wave, small_line.eject_s, small_line.eject_s + 0.05);
        let big_ring = window(&big_wave, big_line.eject_s, big_line.eject_s + 0.05);
        assert!(
            zero_crossing_rate_hz(big_ring, SR) < zero_crossing_rate_hz(small_ring, SR) * 0.8,
            "a heavier case rings lower"
        );
    }

    #[test]
    fn the_same_seed_replays_bit_exactly() {
        let a = render_to_vec(&mut BreechCycle::new(88.0, SR, 9), 9_600);
        let b = render_to_vec(&mut BreechCycle::new(88.0, SR, 9), 9_600);
        assert_eq!(a, b);
        let c = render_to_vec(&mut BreechCycle::new(88.0, SR, 10), 9_600);
        assert_ne!(a, c, "two guns cycling together must not phase-lock");
    }
}
