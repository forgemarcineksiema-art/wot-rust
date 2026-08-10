//! The hangar's air (Hala 3.0 G1): a workshop at rest. Three layers, all deterministic:
//! a room tone (ventilation hum + a low, slowly breathing rumble — the space itself), a radio
//! murmuring on the workbench (a pentatonic tune through a small-speaker band-pass, panned to
//! the bench's bearing), and rare one-shots — a drip, a metal tick, a distant trolley — that
//! say somebody works here even when nothing moves. The battlefield never hears any of it.

use crate::dsp::{Biquad, ExpDecay, Noise, OnePoleLowPass};

/// Seconds between one-shots: a workshop speaks rarely — under ~7 s it becomes a haunted house.
const ONESHOT_GAP_S: (f32, f32) = (7.0, 14.0);

/// The rare punctuation of a quiet hall. Each renders mono and takes a fixed pan for its life.
enum OneShot {
    /// Condensation off a roof truss: a short ping gliding down, dying fast.
    Drip { phase: f32, freq_hz: f32, env: ExpDecay },
    /// Cooling sheet steel relaxing: a filtered snap with a metallic band.
    Tick { band: Biquad, env: ExpDecay, noise: Noise },
    /// A parts trolley rolling somewhere behind the stores: a low swell, there and gone.
    Trolley { t: f32, duration_s: f32, lp: OnePoleLowPass, noise: Noise },
}

pub struct HangarAmbience {
    sample_rate_hz: f32,
    // Room tone: decorrelated rumble + a detuned pair of vent-hum partials.
    left_noise: Noise,
    right_noise: Noise,
    left_lp: OnePoleLowPass,
    right_lp: OnePoleLowPass,
    hum_phase_a: f32,
    hum_phase_b: f32,
    breath_phase: f32,
    // The radio: a seeded melody through a small speaker, panned to the bench.
    radio_rng: Noise,
    radio_note_hz: f32,
    radio_note_t: f32,
    radio_phase: f32,
    radio_vibrato_phase: f32,
    radio_env: f32,
    radio_hp: Biquad,
    radio_lp: OnePoleLowPass,
    radio_pan: f32,
    radio_gain: f32,
    // One-shot scheduling.
    oneshot_rng: Noise,
    next_oneshot_s: f32,
    active: Option<(OneShot, f32)>,
    // Eased master level so garage <-> battle crossfades.
    level: f32,
    target_level: f32,
}

impl HangarAmbience {
    pub fn new(sample_rate_hz: f32, seed: u64) -> Self {
        let mut oneshot_rng = Noise::new(seed ^ 0x0A11_0F1A_7ED0_0D5Eu64.rotate_left(1));
        let first_gap = ONESHOT_GAP_S.0 + (ONESHOT_GAP_S.1 - ONESHOT_GAP_S.0) * oneshot_rng.unit();
        Self {
            sample_rate_hz,
            left_noise: Noise::new(seed),
            right_noise: Noise::new(seed ^ 0x6A2A_6ED5_1DEA_F00D),
            left_lp: OnePoleLowPass::new(150.0, sample_rate_hz),
            right_lp: OnePoleLowPass::new(150.0, sample_rate_hz),
            hum_phase_a: 0.0,
            hum_phase_b: 0.0,
            breath_phase: 0.0,
            radio_rng: Noise::new(seed ^ 0x2AD1_0BEA_7B05_CA5Eu64.rotate_left(3)),
            radio_note_hz: 0.0,
            radio_note_t: 0.0,
            radio_phase: 0.0,
            radio_vibrato_phase: 0.0,
            radio_env: 0.0,
            radio_hp: Biquad::high_pass(380.0, 0.8, sample_rate_hz),
            radio_lp: OnePoleLowPass::new(2_300.0, sample_rate_hz),
            radio_pan: 0.0,
            radio_gain: 0.0,
            oneshot_rng,
            next_oneshot_s: first_gap,
            active: None,
            level: 0.0,
            target_level: 0.0,
        }
    }

    /// Scene amount: 1 inside the hangar, 0 on the battlefield.
    pub fn set_level(&mut self, level: f32) {
        self.target_level = level.clamp(0.0, 1.0);
    }

    /// Where the bench radio sits between the ears this frame: `pan` −1 left .. 1 right from
    /// the listener's own bearing, `gain` the distance attenuation (both computed by the game,
    /// which knows where the bench and the camera are — this crate knows no hall geometry).
    pub fn set_radio(&mut self, pan: f32, gain: f32) {
        self.radio_pan = pan.clamp(-1.0, 1.0);
        self.radio_gain = gain.clamp(0.0, 1.0);
    }

    /// The next radio note: mostly steps of a pentatonic scale over a low root, with rests —
    /// a tune you cannot quite follow, which is what a radio across a hall sounds like.
    fn next_radio_note(&mut self) -> f32 {
        const SCALE: [f32; 6] = [0.0, 2.0, 4.0, 7.0, 9.0, 12.0];
        if self.radio_rng.unit() < 0.3 {
            return 0.0; // a rest
        }
        let degree = SCALE[(self.radio_rng.unit() * SCALE.len() as f32) as usize % SCALE.len()];
        311.0 * (2.0f32).powf(degree / 12.0)
    }

    fn spawn_oneshot(&mut self) -> (OneShot, f32) {
        let pan = self.oneshot_rng.signed() * 0.6;
        let pick = self.oneshot_rng.unit();
        let seed = (self.oneshot_rng.unit() * 1.0e6) as u64 + 17;
        let shot = if pick < 0.4 {
            OneShot::Drip {
                phase: 0.0,
                freq_hz: 2_100.0 + 500.0 * self.oneshot_rng.unit(),
                env: ExpDecay::new(0.9, 0.085, self.sample_rate_hz),
            }
        } else if pick < 0.75 {
            OneShot::Tick {
                band: Biquad::band_pass(
                    2_600.0 + 900.0 * self.oneshot_rng.unit(),
                    11.0,
                    self.sample_rate_hz,
                ),
                env: ExpDecay::new(1.0, 0.045, self.sample_rate_hz),
                noise: Noise::new(seed),
            }
        } else {
            OneShot::Trolley {
                t: 0.0,
                duration_s: 1.6 + 0.8 * self.oneshot_rng.unit(),
                lp: OnePoleLowPass::new(120.0, self.sample_rate_hz),
                noise: Noise::new(seed ^ 0x7011_EE75),
            }
        };
        (shot, pan)
    }

    /// Render additively into an interleaved stereo buffer.
    pub fn render_add_stereo(&mut self, out: &mut [f32], gain: f32) {
        let sr = self.sample_rate_hz;
        let glide = 1.0 - (-1.0 / (0.8 * sr)).exp();
        let dt = 1.0 / sr;
        for frame in out.chunks_exact_mut(2) {
            self.level += (self.target_level - self.level) * glide;
            if self.level < 1.0e-4 {
                continue;
            }
            // Room tone: rumble breathing on a slow LFO, and the vent's detuned hum pair —
            // two close partials beat against each other, which is what a duct actually does.
            self.breath_phase += std::f32::consts::TAU * 0.043 * dt;
            self.hum_phase_a += std::f32::consts::TAU * 87.0 * dt;
            self.hum_phase_b += std::f32::consts::TAU * 92.5 * dt;
            // The room tone stays a FLOOR: quiet enough that a drip off a truss genuinely
            // punctuates it (the first mix drowned every one-shot under its own hum).
            let breath = 0.8 + 0.2 * self.breath_phase.sin();
            let hum = (self.hum_phase_a.sin() + 0.8 * self.hum_phase_b.sin()) * 0.07;
            let room_l = self.left_lp.process(self.left_noise.signed()) * breath * 0.5 + hum;
            let room_r = self.right_lp.process(self.right_noise.signed()) * breath * 0.5 + hum;

            // The radio: melody sequencer at ~140 BPM eighths, small-speaker band, bench pan.
            let mut radio = 0.0;
            if self.radio_gain > 1.0e-3 {
                self.radio_note_t -= dt;
                if self.radio_note_t <= 0.0 {
                    self.radio_note_hz = self.next_radio_note();
                    self.radio_note_t = 0.42;
                    self.radio_env = if self.radio_note_hz > 0.0 { 1.0 } else { self.radio_env };
                }
                self.radio_env *= (-dt / 0.30).exp();
                if self.radio_note_hz > 0.0 {
                    self.radio_vibrato_phase += std::f32::consts::TAU * 5.3 * dt;
                    let vibrato = 1.0 + 0.006 * self.radio_vibrato_phase.sin();
                    self.radio_phase += std::f32::consts::TAU * self.radio_note_hz * vibrato * dt;
                    let tone = self.radio_phase.sin() + 0.35 * (2.0 * self.radio_phase).sin();
                    radio = tone * self.radio_env;
                }
                // Tape hiss under the tune, then the whole thing through the tiny speaker.
                radio += self.radio_rng.signed() * 0.06;
                radio = self.radio_lp.process(self.radio_hp.process(radio)) * self.radio_gain;
            }

            // One-shot scheduling and synthesis.
            self.next_oneshot_s -= dt;
            if self.active.is_none() && self.next_oneshot_s <= 0.0 {
                self.active = Some(self.spawn_oneshot());
                self.next_oneshot_s =
                    ONESHOT_GAP_S.0 + (ONESHOT_GAP_S.1 - ONESHOT_GAP_S.0) * self.oneshot_rng.unit();
            }
            let mut shot_sample = 0.0;
            let mut shot_pan = 0.0;
            let mut done = false;
            if let Some((shot, pan)) = &mut self.active {
                shot_pan = *pan;
                match shot {
                    OneShot::Drip { phase, freq_hz, env } => {
                        // The pitch falls over the ping's life: a drop, not a beep.
                        *freq_hz *= 1.0 - 0.9 * dt;
                        *phase += std::f32::consts::TAU * *freq_hz * dt;
                        shot_sample = phase.sin() * env.step() * 0.7;
                        done = env.is_quiet();
                    }
                    OneShot::Tick { band, env, noise } => {
                        shot_sample = band.process(noise.signed()) * env.step() * 2.4;
                        done = env.is_quiet();
                    }
                    OneShot::Trolley { t, duration_s, lp, noise } => {
                        *t += dt;
                        // Raised-cosine swell: rolls in, rolls away, never slams.
                        let window = 0.5 - 0.5 * (std::f32::consts::TAU * (*t / *duration_s)).cos();
                        shot_sample = lp.process(noise.signed()) * window * 0.9;
                        done = *t >= *duration_s;
                    }
                }
            }
            if done {
                self.active = None;
            }

            // Equal-power pans for the radio and the active one-shot.
            let (radio_l, radio_r) = equal_power(self.radio_pan);
            let (shot_l, shot_r) = equal_power(shot_pan);
            let amount = self.level * gain;
            frame[0] += (room_l * 0.8 + radio * radio_l + shot_sample * shot_l) * amount;
            frame[1] += (room_r * 0.8 + radio * radio_r + shot_sample * shot_r) * amount;
        }
    }
}

/// Equal-power stereo weights for a pan in −1..1.
fn equal_power(pan: f32) -> (f32, f32) {
    let t = (pan.clamp(-1.0, 1.0) + 1.0) * 0.25 * std::f32::consts::PI;
    (t.cos(), t.sin())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voice::rms;

    const SR: f32 = 48_000.0;

    fn stereo(bed: &mut HangarAmbience, seconds: f32) -> Vec<f32> {
        let mut out = vec![0.0; (seconds * SR) as usize * 2];
        bed.render_add_stereo(&mut out, 1.0);
        out
    }

    #[test]
    fn the_hall_breathes_in_the_garage_and_is_silent_on_the_field() {
        let mut bed = HangarAmbience::new(SR, 3);
        bed.set_level(1.0);
        let up = stereo(&mut bed, 3.0);
        assert!(rms(&up[up.len() / 2..]) > 0.005, "the hangar bed must be audible");
        bed.set_level(0.0);
        let down = stereo(&mut bed, 5.0);
        assert!(rms(&down[down.len() - 9_600..]) < 1.0e-3, "the field hears no hangar");
    }

    #[test]
    fn the_radio_lives_in_a_small_speaker_band() {
        // The radio's contribution = (bed with radio) − (bed without): same seeds, same
        // scheduling, the difference is exactly the murmur. Its energy must sit in the
        // speech-box band a small speaker allows — hundreds of crossings per second, far
        // above the room rumble, far below hiss.
        let mut with = HangarAmbience::new(SR, 11);
        with.set_level(1.0);
        with.set_radio(0.0, 1.0);
        let a = stereo(&mut with, 6.0);
        let mut without = HangarAmbience::new(SR, 11);
        without.set_level(1.0);
        let b = stereo(&mut without, 6.0);
        let diff: Vec<f32> =
            a.iter().zip(&b).map(|(x, y)| x - y).skip(2 * SR as usize).step_by(2).collect();
        assert!(rms(&diff) > 1.0e-3, "the radio must actually murmur");
        let crossings = diff.windows(2).filter(|w| (w[0] >= 0.0) != (w[1] >= 0.0)).count();
        let per_second = crossings as f32 / (diff.len() as f32 / SR);
        assert!(
            (600.0..8_000.0).contains(&per_second),
            "a small speaker's murmur, not rumble and not hiss: {per_second} crossings/s"
        );
    }

    #[test]
    fn the_hall_speaks_rarely_but_it_speaks() {
        let mut bed = HangarAmbience::new(SR, 5);
        bed.set_level(1.0);
        let buffer = stereo(&mut bed, 40.0);
        // Peak-over-bed detection per quarter-second window, after the fade-in.
        let settled = &buffer[2 * SR as usize..];
        let windows: Vec<(f32, f32)> = settled
            .chunks((SR / 4.0) as usize * 2)
            .map(|w| (rms(w), w.iter().fold(0.0f32, |m, s| m.max(s.abs()))))
            .collect();
        let median_rms = {
            let mut r: Vec<f32> = windows.iter().map(|(r, _)| *r).collect();
            r.sort_by(f32::total_cmp);
            r[r.len() / 2]
        };
        let loud = windows.iter().filter(|(_, peak)| *peak > 6.0 * median_rms).count();
        assert!(loud >= 2, "40 s of workshop must contain punctuation, got {loud} loud windows");
        assert!(
            loud <= windows.len() / 5,
            "punctuation stays rare — {loud} loud windows of {}",
            windows.len()
        );
    }

    #[test]
    fn the_bed_is_deterministic() {
        let render = || {
            let mut bed = HangarAmbience::new(SR, 9);
            bed.set_level(1.0);
            bed.set_radio(-0.4, 0.8);
            stereo(&mut bed, 20.0)
        };
        assert!(render() == render(), "same seed, same hall, bit for bit");
    }
}
