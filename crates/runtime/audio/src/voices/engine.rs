//! The player's powerplant: a V-12 tank diesel modeled as its firing pulse train. Every engine
//! cycle excites a low resonant thump; exhaust noise breathes over it with load, and above
//! walking speed the tracks join in — a clatter amplitude-modulated at the real link-passing
//! rate. One continuous voice, driven per frame by three knobs: RPM, load and ground speed.

use crate::dsp::{Biquad, Noise, OnePoleLowPass};
use glam::FloatExt;

/// Firing fundamental range. A four-stroke V-12 fires 6 times per revolution, so 650 RPM idle
/// -> 65 Hz and 2000 RPM flat out -> 200 Hz.
const FIRING_HZ_IDLE: f32 = 65.0;
const FIRING_HZ_MAX: f32 = 200.0;
/// Track link pitch of the reference vehicle (T-54): one link passes the sprocket every 0.137 m.
const LINK_PITCH_M: f32 = 0.137;

/// What the tracks are actually riding (Immersja C2) — the audio crate's OWN vocabulary;
/// the client translates the terrain's ground/road answer into it, the DSP stays
/// terrain-ignorant. Each surface names how the clatter speaks: its level, how much of
/// its top end survives, and how hard the link-rate knock reads through.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TrackSurface {
    /// Field soil / grass: the reference clatter (exactly the pre-C2 sound).
    #[default]
    Soil,
    /// Natural rock crest: harder, brighter.
    Rock,
    /// Railway ballast: a crunchy, slightly brighter bed.
    Ballast,
    /// A granite street: loud, bright, and the stones knock at the link rate.
    Cobble,
}

impl TrackSurface {
    /// `(level multiplier, brightness multiplier, knock depth)` for the clatter chain.
    fn voice(self) -> (f32, f32, f32) {
        match self {
            TrackSurface::Soil => (1.0, 1.0, 0.0),
            TrackSurface::Rock => (1.15, 1.3, 0.15),
            TrackSurface::Ballast => (1.25, 1.15, 0.3),
            TrackSurface::Cobble => (1.4, 1.5, 0.6),
        }
    }
}

pub struct EngineVoice {
    sample_rate_hz: f32,
    /// Smoothed control values — the game thread sets targets, the audio rate glides to them so
    /// RPM never steps audibly between frames.
    rpm_norm: f32,
    load: f32,
    speed_mps: f32,
    target_rpm_norm: f32,
    target_load: f32,
    target_speed_mps: f32,
    /// Master on/off (dead tank, garage): eased so the engine spools down, not cuts.
    target_running: f32,
    running: f32,
    firing_phase: f32,
    lope_phase: f32,
    track_phase: f32,
    /// Ground layer (D8): 0 = hard dry field, 1 = rain-soaked. Wet soil swallows the clatter's
    /// top and damps the squeal — the same run sounds heavier in a squall.
    target_wetness: f32,
    wetness: f32,
    /// Surface voice (Immersja C2), glided like every other control so crossing a curb
    /// crossfades the clatter instead of stepping it: level ×, brightness ×, knock 0..1.
    target_surface: (f32, f32, f32),
    surface_level: f32,
    surface_bright: f32,
    surface_knock: f32,
    squeal_phase: f32,
    noise: Noise,
    exhaust_lp: OnePoleLowPass,
    body_bp: Biquad,
    track_hp: Biquad,
    track_ground_lp: OnePoleLowPass,
}

impl EngineVoice {
    pub fn new(sample_rate_hz: f32, seed: u64) -> Self {
        Self {
            sample_rate_hz,
            rpm_norm: 0.0,
            load: 0.0,
            speed_mps: 0.0,
            target_rpm_norm: 0.0,
            target_load: 0.0,
            target_speed_mps: 0.0,
            target_running: 0.0,
            running: 0.0,
            firing_phase: 0.0,
            lope_phase: 0.0,
            track_phase: 0.0,
            target_wetness: 0.0,
            wetness: 0.0,
            target_surface: TrackSurface::Soil.voice(),
            surface_level: 1.0,
            surface_bright: 1.0,
            surface_knock: 0.0,
            squeal_phase: 0.0,
            noise: Noise::new(seed),
            exhaust_lp: OnePoleLowPass::new(700.0, sample_rate_hz),
            body_bp: Biquad::band_pass(95.0, 1.4, sample_rate_hz),
            track_hp: Biquad::high_pass(1_400.0, 0.707, sample_rate_hz),
            track_ground_lp: OnePoleLowPass::new(6_000.0, sample_rate_hz),
        }
    }

    /// The ground under the tracks (D8): 0 dry field, 1 rain-soaked. Wet soil swallows the
    /// clatter's brightness and the metal-on-metal squeal.
    pub fn set_ground_wetness(&mut self, wetness: f32) {
        self.target_wetness = wetness.clamp(0.0, 1.0);
    }

    /// What the tracks are riding (Immersja C2): the clatter's timbre follows the street.
    /// Glided internally — crossing a curb crossfades, never steps.
    pub fn set_track_surface(&mut self, surface: TrackSurface) {
        self.target_surface = surface.voice();
    }

    /// Per-frame control update from the game thread. `rpm_norm` 0..1 spans idle..governed,
    /// `load` 0..1 is throttle demand, `speed_mps` the hull ground speed.
    pub fn set_state(&mut self, rpm_norm: f32, load: f32, speed_mps: f32, running: bool) {
        self.target_rpm_norm = rpm_norm.clamp(0.0, 1.0);
        self.target_load = load.clamp(0.0, 1.0);
        self.target_speed_mps = speed_mps.abs();
        self.target_running = if running { 1.0 } else { 0.0 };
    }

    /// True once the bed has spooled fully down (switched off and audibly gone) — the remote
    /// engine pool frees slots on this instead of hard-cutting a still-sounding voice.
    pub fn is_silent(&self) -> bool {
        self.target_running == 0.0 && self.running < 1.0e-3
    }

    /// Render additively into `out` (the mixer owns the buffer; the bed never claims it).
    pub fn render_add(&mut self, out: &mut [f32], gain: f32) {
        // Per-sample one-pole glide: ~120 ms to close 63 % of a control step.
        let glide = 1.0 - (-1.0 / (0.12 * self.sample_rate_hz)).exp();
        for sample in out.iter_mut() {
            self.rpm_norm += (self.target_rpm_norm - self.rpm_norm) * glide;
            self.load += (self.target_load - self.load) * glide;
            self.speed_mps += (self.target_speed_mps - self.speed_mps) * glide;
            self.running += (self.target_running - self.running) * glide;
            if self.running < 1.0e-3 {
                continue;
            }

            let firing_hz = FIRING_HZ_IDLE.lerp(FIRING_HZ_MAX, self.rpm_norm);
            self.firing_phase += firing_hz / self.sample_rate_hz;
            if self.firing_phase >= 1.0 {
                self.firing_phase -= 1.0;
            }
            // Each firing event is a sharp exponential within its own cycle; the band-pass turns
            // the pulse train into the engine's low chest tone.
            let pulse = (-6.0 * self.firing_phase).exp() * 2.0 - 0.31;
            let body = self.body_bp.process(pulse) * 2.0.lerp(3.2, self.load);

            // Idle lope: a slow half-order wobble that makes a standing tank sound alive.
            self.lope_phase += std::f32::consts::TAU * firing_hz * 0.5 / 6.0 / self.sample_rate_hz;
            let lope = 1.0 + self.lope_phase.sin() * 0.18 * (1.0 - self.rpm_norm);

            // Exhaust: noise that opens up (louder AND brighter) under load.
            self.exhaust_lp.set_cutoff(500.0.lerp(2_200.0, self.load), self.sample_rate_hz);
            let exhaust = self.exhaust_lp.process(self.noise.signed()) * 0.12.lerp(0.42, self.load);

            // Tracks: clatter high-passed noise, amplitude-modulated at the link-passing rate,
            // fading in above a slow walk. The ground answers (D8): wet soil swallows the top
            // of the clatter and some of its level — the same run reads heavier in a squall.
            self.wetness += (self.target_wetness - self.wetness) * glide;
            self.surface_level += (self.target_surface.0 - self.surface_level) * glide;
            self.surface_bright += (self.target_surface.1 - self.surface_bright) * glide;
            self.surface_knock += (self.target_surface.2 - self.surface_knock) * glide;
            let link_hz = self.speed_mps / LINK_PITCH_M;
            self.track_phase += std::f32::consts::TAU * link_hz / self.sample_rate_hz;
            let track_level = (self.speed_mps / 8.0).clamp(0.0, 1.0)
                * 0.3
                * (1.0 - 0.35 * self.wetness)
                * self.surface_level;
            // Cobble knocks at the link rate: the knock deepens the AM floor, so stones
            // read as a rhythm, not just a louder hiss. Soil keeps the original 0.6/0.4.
            let modulation_floor = 0.6 * (1.0 - 0.7 * self.surface_knock);
            let modulation =
                modulation_floor + (1.0 - modulation_floor) * self.track_phase.sin().powi(2);
            self.track_ground_lp.set_cutoff(
                (6_000.0 * self.surface_bright).lerp(1_600.0, self.wetness),
                self.sample_rate_hz,
            );
            let clatter = self.track_hp.process(self.noise.signed());
            let tracks = self.track_ground_lp.process(clatter) * track_level * modulation;

            // The crawl squeal (D8): metal grinding at walking pace — a thin tone that lives
            // only in the low-speed window and dies both at rest and at speed; rain damps it.
            let squeal_window = ((self.speed_mps - 0.4) / 0.6).clamp(0.0, 1.0)
                * ((3.0 - self.speed_mps) / 1.2).clamp(0.0, 1.0);
            let squeal = if squeal_window > 0.0 {
                let vibrato = 1.0 + 0.015 * self.track_phase.sin();
                self.squeal_phase +=
                    std::f32::consts::TAU * 2_600.0 * vibrato / self.sample_rate_hz;
                self.squeal_phase.sin() * squeal_window * 0.10 * (1.0 - 0.6 * self.wetness)
            } else {
                0.0
            };

            let idle_floor = 0.4.lerp(1.0, self.rpm_norm.max(self.load));
            *sample += (body * lope + exhaust + tracks + squeal) * idle_floor * self.running * gain;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voice::{peak, rms, zero_crossing_rate_hz};

    const SR: f32 = 48_000.0;

    fn render(engine: &mut EngineVoice, seconds: f32) -> Vec<f32> {
        let mut out = vec![0.0; (seconds * SR) as usize];
        engine.render_add(&mut out, 1.0);
        out
    }

    /// D8's ground layer: rain-soaked soil swallows the clatter's top end (a duller spectrum
    /// at speed) and damps the crawl squeal (quieter at walking pace). Same drive, same
    /// speed — the ground is the only thing that changed.
    #[test]
    fn wet_ground_dulls_the_clatter_and_damps_the_crawl_squeal() {
        let run = |speed: f32, wetness: f32| {
            let mut engine = EngineVoice::new(SR, 5);
            engine.set_state(0.6, 0.3, speed, true);
            engine.set_ground_wetness(wetness);
            let mut out = vec![0.0; (2.0 * SR) as usize];
            engine.render_add(&mut out, 1.0);
            out[out.len() / 2..].to_vec()
        };

        // At speed: the wet run's spectrum sits lower (duller clatter).
        let dry_fast = run(9.0, 0.0);
        let wet_fast = run(9.0, 1.0);
        let dry_zcr = zero_crossing_rate_hz(&dry_fast, SR);
        let wet_zcr = zero_crossing_rate_hz(&wet_fast, SR);
        assert!(
            wet_zcr < dry_zcr * 0.85,
            "wet soil must swallow the clatter's top: dry {dry_zcr} Hz vs wet {wet_zcr} Hz"
        );

        // At a crawl: the squeal's 2.6 kHz tone lifts the spectrum on dry ground and rain
        // damps it — the engine body swamps plain RMS, so the spectrum is the witness.
        let dry_crawl = run(1.2, 0.0);
        let wet_crawl = run(1.2, 1.0);
        let dry_crawl_zcr = zero_crossing_rate_hz(&dry_crawl, SR);
        let wet_crawl_zcr = zero_crossing_rate_hz(&wet_crawl, SR);
        assert!(
            dry_crawl_zcr > wet_crawl_zcr * 1.05,
            "the crawl squeal must lose its voice in the rain: dry {dry_crawl_zcr} Hz vs wet {wet_crawl_zcr} Hz"
        );
        // And the squeal is a crawl phenomenon: a resting tank's spectrum sits far lower.
        let dry_rest = run(0.0, 0.0);
        assert!(
            dry_crawl_zcr > zero_crossing_rate_hz(&dry_rest, SR) * 1.1,
            "a crawling tank squeals over its idle"
        );
    }

    #[test]
    fn a_running_engine_is_audible_and_a_stopped_one_spools_down_to_silence() {
        let mut engine = EngineVoice::new(SR, 4);
        engine.set_state(0.2, 0.0, 0.0, true);
        let idle = render(&mut engine, 1.0);
        assert!(rms(&idle[SR as usize / 2..]) > 0.01, "an idling engine must be heard");
        assert!(idle.iter().all(|s| s.is_finite()));

        engine.set_state(0.2, 0.0, 0.0, false);
        let stopping = render(&mut engine, 2.0);
        let tail = &stopping[stopping.len() - 4_800..];
        assert!(rms(tail) < 1.0e-3, "a stopped engine must fade to silence");
    }

    #[test]
    fn higher_rpm_raises_the_firing_fundamental() {
        let mut idle_engine = EngineVoice::new(SR, 4);
        idle_engine.set_state(0.0, 0.0, 0.0, true);
        let idle = render(&mut idle_engine, 2.0);

        let mut governed = EngineVoice::new(SR, 4);
        governed.set_state(1.0, 1.0, 0.0, true);
        let flat_out = render(&mut governed, 2.0);

        // Compare after the glide settled.
        let settled = SR as usize..2 * SR as usize;
        let idle_zcr = zero_crossing_rate_hz(&idle[settled.clone()], SR);
        let flat_zcr = zero_crossing_rate_hz(&flat_out[settled], SR);
        assert!(
            flat_zcr > idle_zcr * 1.3,
            "full throttle must sit higher than idle: {flat_zcr} vs {idle_zcr}"
        );
    }

    #[test]
    fn moving_adds_track_clatter_over_the_standing_engine() {
        let mut standing = EngineVoice::new(SR, 4);
        standing.set_state(0.5, 0.5, 0.0, true);
        let parked = render(&mut standing, 2.0);

        let mut rolling = EngineVoice::new(SR, 4);
        rolling.set_state(0.5, 0.5, 8.0, true);
        let moving = render(&mut rolling, 2.0);

        // Track clatter lives above 1.4 kHz; crude probe: the moving take is brighter.
        let settled = SR as usize..2 * SR as usize;
        let parked_zcr = zero_crossing_rate_hz(&parked[settled.clone()], SR);
        let moving_zcr = zero_crossing_rate_hz(&moving[settled], SR);
        assert!(
            moving_zcr > parked_zcr,
            "tracks must brighten the moving tank: {moving_zcr} vs {parked_zcr}"
        );
    }

    /// Immersja C2's audible promise: the same drive over a granite street is louder and
    /// brighter than over field soil — the surface is the only thing that changed. And the
    /// default surface IS the old sound: a Soil run matches the pre-C2 clatter contract
    /// (the wet-ground A/B above keeps passing untouched on the same defaults).
    #[test]
    fn cobble_rings_louder_and_brighter_than_soil_under_the_same_drive() {
        let run = |surface: TrackSurface| {
            let mut engine = EngineVoice::new(SR, 5);
            engine.set_state(0.6, 0.3, 9.0, true);
            engine.set_track_surface(surface);
            let out = render(&mut engine, 2.0);
            out[SR as usize..].to_vec()
        };
        let soil = run(TrackSurface::Soil);
        let cobble = run(TrackSurface::Cobble);
        // The engine body swamps plain RMS (same reason the wet-ground lock reads the
        // spectrum): the clatter lives in the top end, so brightness is the witness —
        // and the level multiplier rides the same glided chain the brightness proves.
        assert!(
            zero_crossing_rate_hz(&cobble, SR) > zero_crossing_rate_hz(&soil, SR) * 1.08,
            "stones must ring brighter than soil: {} vs {}",
            zero_crossing_rate_hz(&cobble, SR),
            zero_crossing_rate_hz(&soil, SR)
        );
        assert!(rms(&cobble) > rms(&soil), "and never quieter: {} vs {}", rms(&cobble), rms(&soil));
    }

    /// Immersja C1's audible promise, DSP side: the same rpm under MORE load is louder
    /// (the working body swells) and brighter (the exhaust bark opens up). Probed at a
    /// standstill — a stall dig — so the track clatter cannot mask the exhaust's voice.
    /// The client feeds this knob the predictor's honest strain; this locks what the
    /// knob buys when it arrives.
    #[test]
    fn more_load_at_the_same_rpm_is_louder_and_brighter() {
        let run = |load: f32| {
            let mut engine = EngineVoice::new(SR, 4);
            engine.set_state(0.6, load, 0.0, true);
            let out = render(&mut engine, 2.0);
            out[SR as usize..].to_vec()
        };
        let relaxed = run(0.35);
        let straining = run(1.0);
        assert!(
            rms(&straining) > rms(&relaxed) * 1.15,
            "a straining engine must swell: {} vs {}",
            rms(&straining),
            rms(&relaxed)
        );
        assert!(
            zero_crossing_rate_hz(&straining, SR) > zero_crossing_rate_hz(&relaxed, SR) * 1.05,
            "and its exhaust must bark brighter: {} vs {}",
            zero_crossing_rate_hz(&straining, SR),
            zero_crossing_rate_hz(&relaxed, SR)
        );
    }

    #[test]
    fn the_bed_is_additive_and_bounded() {
        let mut engine = EngineVoice::new(SR, 4);
        engine.set_state(1.0, 1.0, 10.0, true);
        let mut out = vec![0.25; SR as usize];
        engine.render_add(&mut out, 1.0);
        assert!(out.iter().all(|s| s.is_finite()));
        assert!(peak(&out) < 4.0, "the engine bed must not explode the mix");
        assert!(out.iter().any(|s| (s - 0.25).abs() > 1.0e-4), "must add onto existing content");
    }
}
