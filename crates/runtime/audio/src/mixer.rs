//! The mixer: world-space events in, an interleaved stereo buffer out. Each event picks its
//! voice design, [`crate::spatial`] places it between the ears, and the master bus soft-clips
//! the sum. The player's own gun and the UI cues bypass the travel physics (they are AT the
//! listener).

use glam::Vec3;

pub use crate::events::AudioEvent;

use crate::dsp::soft_clip;
use crate::remote::{RemoteEngineState, RemoteEngines};
use crate::spatial::{Listener, VoiceSlot};
use crate::voice::Voice;
use crate::voices::ambience::WindAmbience;
use crate::voices::blast::HeBlast;
use crate::voices::cannon::CannonShot;
use crate::voices::engine::EngineVoice;
use crate::voices::impact::{ArmorHit, GroundImpact};
use crate::voices::ui::{DoubleThud, MechanicalClick};

/// Simultaneous one-shot voices; a 7v7 barrage peaks well under this.
const MAX_VOICES: usize = 40;

/// The one object the platform audio callback owns: push events and control state from the game
/// thread, pull interleaved stereo from the audio thread.
pub struct AudioEngine {
    sample_rate_hz: f32,
    listener: Listener,
    slots: Vec<VoiceSlot>,
    engine_bed: EngineVoice,
    /// Every other tank's powerplant in earshot (see `remote`).
    remote_engines: RemoteEngines,
    wind: WindAmbience,
    master_gain: f32,
    /// Mono scratch reused across voices; grows to the largest callback chunk and stays.
    scratch: Vec<f32>,
    /// Decorrelation seed, bumped per spawned voice.
    next_seed: u64,
}

impl AudioEngine {
    pub fn new(sample_rate_hz: f32) -> Self {
        Self {
            sample_rate_hz,
            listener: Listener::default(),
            slots: Vec::with_capacity(MAX_VOICES),
            engine_bed: EngineVoice::new(sample_rate_hz, 0x0E17_61E5),
            remote_engines: RemoteEngines::default(),
            wind: WindAmbience::new(sample_rate_hz, 0x57A7_1CA1),
            master_gain: 0.85,
            scratch: Vec::new(),
            next_seed: 1,
        }
    }

    pub fn set_listener(&mut self, listener: Listener) {
        self.listener = listener;
    }

    /// Per-frame player powerplant state — see [`EngineVoice::set_state`].
    pub fn set_player_engine(&mut self, rpm_norm: f32, load: f32, speed_mps: f32, running: bool) {
        self.engine_bed.set_state(rpm_norm, load, speed_mps, running);
    }

    /// Scene wind amount: ~1 on the battlefield, ~0.25 inside the hangar.
    pub fn set_wind_level(&mut self, level: f32) {
        self.wind.set_level(level);
    }

    /// This frame's remote powerplants (every other tank the player might hear). The pool keeps
    /// the nearest few in earshot, spatializes them against the current listener, and spools
    /// down anything no longer reported — call after [`Self::set_listener`].
    pub fn set_remote_engines(&mut self, states: &[RemoteEngineState]) {
        self.remote_engines.sync(states, &self.listener, self.sample_rate_hz);
    }

    pub fn push_event(&mut self, event: AudioEvent) {
        self.push_event_occluded(event, 0.0);
    }

    /// Push an event with the game's terrain-occlusion judgment (0 clear .. 1 fully masked by a
    /// ridge). Only world-positioned sounds care; listener-local cues ignore it. The player's
    /// own shot is never occluded — the muzzle is meters from the ear.
    pub fn push_event_occluded(&mut self, event: AudioEvent, occlusion: f32) {
        let seed = self.next_seed;
        self.next_seed = self.next_seed.wrapping_add(1);
        match event {
            AudioEvent::CannonFired { position, caliber_mm, own_shot } => {
                let voice = Box::new(CannonShot::new(caliber_mm, self.sample_rate_hz, seed));
                if own_shot {
                    // The player's own gun: full presence, centered a touch by its true bearing
                    // (the muzzle sits meters ahead), and never delayed behind the muzzle flash.
                    self.spawn_at(voice, position, 1.0, false, 0.0);
                } else {
                    self.spawn_at(voice, position, 1.05, true, occlusion);
                }
            }
            AudioEvent::ArmorStruck { position, penetrated, ricocheted, high_explosive } => {
                // HE speaks as the charge, not the plate: a burst instead of a modal clang.
                let voice: Box<dyn Voice> = if high_explosive {
                    Box::new(HeBlast::new(self.sample_rate_hz, seed))
                } else {
                    Box::new(ArmorHit::new(penetrated, ricocheted, self.sample_rate_hz, seed))
                };
                let gain = if high_explosive { 1.0 } else { 0.8 };
                self.spawn_at(voice, position, gain, true, occlusion);
            }
            AudioEvent::ShellAbsorbed { position, surface } => {
                let voice = Box::new(GroundImpact::new(surface, self.sample_rate_hz, seed));
                self.spawn_at(voice, position, 0.7, true, occlusion);
            }
            AudioEvent::GunReady => {
                let voice = Box::new(MechanicalClick::new(true, self.sample_rate_hz, seed));
                self.spawn_flat(voice, 0.5);
            }
            AudioEvent::KillConfirmed => {
                self.spawn_flat(Box::new(DoubleThud::new(self.sample_rate_hz)), 0.7);
            }
            AudioEvent::UiClick { accent } => {
                let voice = Box::new(MechanicalClick::new(accent, self.sample_rate_hz, seed));
                self.spawn_flat(voice, 0.4);
            }
        }
    }

    /// Spawn a world-positioned voice — see [`VoiceSlot::positioned`] for the physics.
    fn spawn_at(
        &mut self,
        voice: Box<dyn Voice>,
        position: Vec3,
        gain: f32,
        travel_delay: bool,
        occlusion: f32,
    ) {
        if let Some(slot) = VoiceSlot::positioned(
            voice,
            &self.listener,
            position,
            gain,
            travel_delay,
            occlusion,
            self.sample_rate_hz,
        ) {
            self.push_slot(slot);
        }
    }

    /// Spawn a listener-local voice (UI cues, the breech): centered, undelayed, full bandwidth.
    fn spawn_flat(&mut self, voice: Box<dyn Voice>, gain: f32) {
        self.push_slot(VoiceSlot::flat(voice, gain, self.sample_rate_hz));
    }

    fn push_slot(&mut self, slot: VoiceSlot) {
        if self.slots.len() < MAX_VOICES {
            self.slots.push(slot);
            return;
        }
        // Over budget a barrage degrades by dropping its quietest thread, never by growing.
        if let Some(quietest) = self.slots.iter_mut().min_by(|a, b| a.gain.total_cmp(&b.gain))
            && quietest.gain < slot.gain
        {
            *quietest = slot;
        }
    }

    #[cfg(test)]
    pub(crate) fn live_voices(&self) -> usize {
        self.slots.len()
    }

    /// Render the next chunk of interleaved stereo. This is the audio callback's whole job; it
    /// allocates only if the platform hands a larger buffer than ever before.
    pub fn render(&mut self, out: &mut [f32]) {
        out.fill(0.0);
        let frames = out.len() / 2;
        if frames == 0 {
            return;
        }
        self.wind.render_add_stereo(out, 0.045);

        // The player's engine bed: mono at the ears, added to both channels equally.
        if self.scratch.len() < frames {
            self.scratch.resize(frames, 0.0);
        }
        self.scratch[..frames].fill(0.0);
        self.engine_bed.render_add(&mut self.scratch[..frames], 0.16);
        for (frame, engine) in out.chunks_exact_mut(2).zip(&self.scratch[..frames]) {
            frame[0] += engine;
            frame[1] += engine;
        }

        // Every other powerplant in earshot, panned to its bearing.
        self.remote_engines.render_add(out, &mut self.scratch, 0.14);

        // One-shot voices: render mono, absorb, pan, mix (see `spatial::VoiceSlot::mix_into`).
        let mut index = 0;
        while index < self.slots.len() {
            if self.slots[index].mix_into(out, &mut self.scratch) {
                index += 1;
            } else {
                self.slots.swap_remove(index);
            }
        }

        for sample in out.iter_mut() {
            *sample = soft_clip(*sample * self.master_gain);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voice::rms;
    use crate::voices::impact::GroundKind;

    const SR: f32 = 48_000.0;

    fn stereo_chunks(engine: &mut AudioEngine, seconds: f32) -> Vec<f32> {
        // Render in odd-sized chunks on purpose: correctness must not depend on buffer size.
        let mut out = vec![0.0; (seconds * SR) as usize * 2];
        for chunk in out.chunks_mut(2 * 480) {
            engine.render(chunk);
        }
        out
    }

    fn split(buffer: &[f32]) -> (Vec<f32>, Vec<f32>) {
        let left = buffer.iter().step_by(2).copied().collect();
        let right = buffer.iter().skip(1).step_by(2).copied().collect();
        (left, right)
    }

    #[test]
    fn silence_in_silence_out() {
        let mut engine = AudioEngine::new(SR);
        let out = stereo_chunks(&mut engine, 0.5);
        assert!(rms(&out) < 1.0e-4, "no events, no engine, no wind level => silence");
    }

    #[test]
    fn a_shot_to_the_right_lands_louder_in_the_right_ear() {
        let mut engine = AudioEngine::new(SR);
        // Looking down +Z, screen right is -X.
        engine.set_listener(Listener { position: Vec3::ZERO, forward: Vec3::Z });
        engine.push_event(AudioEvent::CannonFired {
            position: Vec3::new(-40.0, 0.0, 10.0),
            caliber_mm: 100.0,
            own_shot: false,
        });
        let out = stereo_chunks(&mut engine, 1.0);
        let (left, right) = split(&out);
        assert!(
            rms(&right) > rms(&left) * 1.5,
            "right-flank shot must favor the right ear: L {} vs R {}",
            rms(&left),
            rms(&right)
        );
    }

    #[test]
    fn distance_makes_a_shot_quieter_duller_and_later() {
        let close_wave = {
            let mut engine = AudioEngine::new(SR);
            engine.push_event(AudioEvent::CannonFired {
                position: Vec3::new(0.0, 0.0, 30.0),
                caliber_mm: 100.0,
                own_shot: false,
            });
            stereo_chunks(&mut engine, 2.5)
        };
        let far_wave = {
            let mut engine = AudioEngine::new(SR);
            engine.push_event(AudioEvent::CannonFired {
                position: Vec3::new(0.0, 0.0, 500.0),
                caliber_mm: 100.0,
                own_shot: false,
            });
            stereo_chunks(&mut engine, 2.5)
        };
        assert!(rms(&close_wave) > rms(&far_wave) * 3.0, "1/r loudness");

        let first_audible = |wave: &[f32]| wave.iter().position(|s| s.abs() > 1.0e-4).unwrap();
        let close_start = first_audible(&close_wave) / 2;
        let far_start = first_audible(&far_wave) / 2;
        let expected_delay = (500.0 - 30.0) / crate::spatial::SPEED_OF_SOUND_MPS * SR;
        let measured = far_start as f32 - close_start as f32;
        assert!(
            (measured - expected_delay).abs() < 0.1 * SR,
            "sound must travel at ~343 m/s: expected ~{expected_delay} samples, got {measured}"
        );

        use crate::voice::zero_crossing_rate_hz;
        let (close_left, _) = split(&close_wave);
        let (far_left, _) = split(&far_wave);
        let close_zcr = zero_crossing_rate_hz(&close_left[close_start..close_start + 9_600], SR);
        let far_zcr = zero_crossing_rate_hz(&far_left[far_start..far_start + 9_600], SR);
        assert!(far_zcr < close_zcr, "air absorption must dull the far shot");
    }

    /// A ridge between the shooter and the ear mutes and muffles: the occluded take is quieter
    /// and darker (fewer zero crossings) than the same shot in the open at the same distance —
    /// only the low thud diffracts over the hill.
    #[test]
    fn an_occluded_shot_is_quieter_and_duller_than_the_same_shot_in_the_open() {
        let shot = AudioEvent::CannonFired {
            position: Vec3::new(0.0, 0.0, 120.0),
            caliber_mm: 100.0,
            own_shot: false,
        };
        let render = |occlusion: f32| {
            let mut engine = AudioEngine::new(SR);
            engine.push_event_occluded(shot, occlusion);
            stereo_chunks(&mut engine, 1.5)
        };
        let open = render(0.0);
        let masked = render(1.0);

        assert!(rms(&masked) < rms(&open) * 0.6, "the ridge takes most of the level");
        use crate::voice::zero_crossing_rate_hz;
        let start = open.iter().position(|s| s.abs() > 1.0e-4).expect("audible") & !1;
        let window = 9_600 * 2;
        let open_zcr = zero_crossing_rate_hz(&open[start..start + window], SR);
        let masked_zcr = zero_crossing_rate_hz(&masked[start..start + window], SR);
        assert!(
            masked_zcr < open_zcr * 0.8,
            "the highs must not clear the ridge: {masked_zcr} vs {open_zcr}"
        );
    }

    #[test]
    fn the_players_own_shot_is_never_delayed() {
        let mut engine = AudioEngine::new(SR);
        engine.push_event(AudioEvent::CannonFired {
            position: Vec3::new(0.0, 0.0, 25.0), // TPP camera sits well behind the muzzle
            caliber_mm: 100.0,
            own_shot: true,
        });
        let out = stereo_chunks(&mut engine, 0.2);
        let start = out.iter().position(|s| s.abs() > 1.0e-4).expect("own shot audible") / 2;
        assert!(start < 64, "the player's gun must answer the trigger instantly, got {start}");
    }

    #[test]
    fn a_full_barrage_stays_bounded_and_within_the_voice_budget() {
        let mut engine = AudioEngine::new(SR);
        engine.set_player_engine(1.0, 1.0, 8.0, true);
        engine.set_wind_level(1.0);
        for i in 0..100 {
            engine.push_event(AudioEvent::CannonFired {
                position: Vec3::new((i % 20) as f32 * 30.0 - 300.0, 0.0, 60.0),
                caliber_mm: 122.0,
                own_shot: false,
            });
            engine.push_event(AudioEvent::ArmorStruck {
                position: Vec3::new(0.0, 0.0, 40.0),
                penetrated: i % 2 == 0,
                ricocheted: i % 3 == 0,
                high_explosive: i % 5 == 0,
            });
        }
        assert!(engine.live_voices() <= MAX_VOICES);
        let out = stereo_chunks(&mut engine, 1.0);
        assert!(out.iter().all(|s| s.is_finite() && s.abs() <= 1.0));
    }

    #[test]
    fn inaudibly_far_sources_never_claim_a_voice() {
        let mut engine = AudioEngine::new(SR);
        engine.push_event(AudioEvent::ShellAbsorbed {
            position: Vec3::new(0.0, 0.0, 50_000.0),
            surface: GroundKind::Soil,
        });
        assert_eq!(engine.live_voices(), 0);
    }

    #[test]
    fn the_same_event_sequence_renders_bit_identically() {
        let run = || {
            let mut engine = AudioEngine::new(SR);
            engine.set_player_engine(0.6, 0.4, 5.0, true);
            engine.set_wind_level(0.8);
            engine.push_event(AudioEvent::CannonFired {
                position: Vec3::new(50.0, 0.0, 80.0),
                caliber_mm: 88.0,
                own_shot: false,
            });
            engine.push_event(AudioEvent::KillConfirmed);
            stereo_chunks(&mut engine, 1.0)
        };
        assert_eq!(run(), run(), "audio must be deterministic for replay and tests");
    }

    #[test]
    fn ui_cues_render_centered() {
        let mut engine = AudioEngine::new(SR);
        engine.push_event(AudioEvent::GunReady);
        let out = stereo_chunks(&mut engine, 0.3);
        let (left, right) = split(&out);
        assert!((rms(&left) - rms(&right)).abs() < 1.0e-4, "UI cues sit dead center");
        assert!(rms(&left) > 1.0e-4, "and must be audible");
    }
}
