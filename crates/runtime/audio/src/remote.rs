//! Remote powerplants: every other tank on the field hums too. The game reports the set of
//! audible vehicles once per frame; this module keeps one persistent [`EngineVoice`] per kept
//! tank, spatialized like a one-shot (1/r gain, constant-power pan, air absorption) but living
//! across frames — an engine is a *presence*, not an event. A tank that dies or drives out of
//! earshot spools its bed down instead of cutting.

use glam::Vec3;

use crate::dsp::OnePoleLowPass;
use crate::spatial::Listener;
use crate::voices::engine::EngineVoice;

/// One remote vehicle's powerplant, reported by the game each frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RemoteEngineState {
    /// Stable per-vehicle key (the tank id); the bed persists across frames under it.
    pub key: u64,
    pub position: Vec3,
    pub speed_mps: f32,
    /// A dead tank's engine is off (the bed spools down, never cuts).
    pub running: bool,
}

/// Beds beyond this contribute nothing worth the mixing work.
const CULL_DISTANCE_M: f32 = 260.0;
/// Loudest simultaneous remote beds; the nearest win.
const MAX_REMOTE_ENGINES: usize = 4;
/// Distance at which a remote bed has fallen to half its close-up level. Engines carry less
/// than gunfire, so they fall off faster than one-shot voices.
const REFERENCE_DISTANCE_M: f32 = 12.0;
/// Remote speed that reads as "governed flat out" (rpm_norm 1).
const FLAT_OUT_MPS: f32 = 12.0;

pub(crate) struct RemoteEngineSlot {
    key: u64,
    voice: EngineVoice,
    gain: f32,
    pan: f32,
    air: OnePoleLowPass,
    /// No longer reported (dead, despawned, out of earshot): spooling down toward removal.
    absent: bool,
}

/// The pool of remote beds. Owned by the mixer; `sync` runs on the game's per-frame control
/// update, `render_add` inside the audio callback.
#[derive(Default)]
pub(crate) struct RemoteEngines {
    slots: Vec<RemoteEngineSlot>,
}

impl RemoteEngines {
    /// Reconcile the pool with this frame's report: keep the nearest audible beds, update their
    /// spatial placement, spool down everything no longer reported.
    pub fn sync(&mut self, states: &[RemoteEngineState], listener: &Listener, sample_rate_hz: f32) {
        // Nearest-first, hard-culled by earshot, budgeted.
        let mut audible: Vec<&RemoteEngineState> = states
            .iter()
            .filter(|state| state.position.distance(listener.position) < CULL_DISTANCE_M)
            .collect();
        audible.sort_by(|a, b| {
            a.position
                .distance_squared(listener.position)
                .total_cmp(&b.position.distance_squared(listener.position))
        });
        audible.truncate(MAX_REMOTE_ENGINES);

        for slot in &mut self.slots {
            slot.absent = true;
        }
        for state in audible {
            let offset = state.position - listener.position;
            let distance = offset.length();
            let gain = REFERENCE_DISTANCE_M / (REFERENCE_DISTANCE_M + distance);
            // Screen right for a right-handed, Y-up look-at: (-forward.z, 0, forward.x).
            let right = Vec3::new(-listener.forward.z, 0.0, listener.forward.x);
            let flat = Vec3::new(offset.x, 0.0, offset.z);
            let pan = if flat.length_squared() > 1.0e-6 {
                flat.normalize().dot(right.normalize_or_zero()).clamp(-1.0, 1.0)
            } else {
                0.0
            };
            let cutoff_hz = 20_000.0 / (1.0 + distance / 60.0);
            let rpm_norm = (state.speed_mps / FLAT_OUT_MPS).clamp(0.0, 1.0);

            let slot = match self.slots.iter_mut().find(|slot| slot.key == state.key) {
                Some(slot) => slot,
                None => {
                    self.slots.push(RemoteEngineSlot {
                        key: state.key,
                        // The key seeds the noise, so two idling tanks never phase-lock.
                        voice: EngineVoice::new(sample_rate_hz, state.key ^ 0x0E17_61E5),
                        gain: 0.0,
                        pan: 0.0,
                        air: OnePoleLowPass::new(cutoff_hz, sample_rate_hz),
                        absent: false,
                    });
                    self.slots.last_mut().expect("just pushed")
                }
            };
            slot.absent = false;
            slot.gain = gain;
            slot.pan = pan;
            slot.air.set_cutoff(cutoff_hz, sample_rate_hz);
            slot.voice.set_state(rpm_norm, rpm_norm, state.speed_mps, state.running);
        }
        // Unreported beds spool down; once silent the slot is freed.
        for slot in &mut self.slots {
            if slot.absent {
                slot.voice.set_state(0.0, 0.0, 0.0, false);
            }
        }
        self.slots.retain(|slot| !(slot.absent && slot.voice.is_silent()));
    }

    /// Mix every live bed into the interleaved stereo buffer. `scratch` must hold at least
    /// `out.len() / 2` mono samples.
    pub fn render_add(&mut self, out: &mut [f32], scratch: &mut [f32], gain: f32) {
        let frames = out.len() / 2;
        for slot in &mut self.slots {
            scratch[..frames].fill(0.0);
            slot.voice.render_add(&mut scratch[..frames], gain * slot.gain);
            let angle = (slot.pan + 1.0) * std::f32::consts::FRAC_PI_4;
            let (left, right) = (angle.cos(), angle.sin());
            for (frame, sample) in out.chunks_exact_mut(2).zip(&scratch[..frames]) {
                let absorbed = slot.air.process(*sample);
                frame[0] += absorbed * left;
                frame[1] += absorbed * right;
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn live_beds(&self) -> usize {
        self.slots.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voice::rms;

    const SR: f32 = 48_000.0;

    fn listener() -> Listener {
        Listener { position: Vec3::ZERO, forward: Vec3::Z }
    }

    fn state(key: u64, position: Vec3, speed: f32) -> RemoteEngineState {
        RemoteEngineState { key, position, speed_mps: speed, running: true }
    }

    fn render_stereo(engines: &mut RemoteEngines, seconds: f32) -> Vec<f32> {
        let mut out = vec![0.0; (seconds * SR) as usize * 2];
        let mut scratch = vec![0.0; 512];
        for chunk in out.chunks_mut(2 * 512) {
            engines.render_add(chunk, &mut scratch, 1.0);
        }
        out
    }

    #[test]
    fn a_nearby_tank_hums_and_a_distant_one_is_culled() {
        let mut engines = RemoteEngines::default();
        engines.sync(
            &[state(1, Vec3::new(0.0, 0.0, 30.0), 5.0), state(2, Vec3::new(0.0, 0.0, 900.0), 5.0)],
            &listener(),
            SR,
        );
        assert_eq!(engines.live_beds(), 1, "only the tank in earshot claims a bed");
        let out = render_stereo(&mut engines, 1.0);
        assert!(rms(&out[out.len() / 2..]) > 1.0e-4, "the nearby bed is audible");
        assert!(out.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn the_bed_pool_keeps_only_the_nearest_budget() {
        let mut engines = RemoteEngines::default();
        let states: Vec<RemoteEngineState> =
            (0..9).map(|i| state(i, Vec3::new(0.0, 0.0, 20.0 + i as f32 * 20.0), 6.0)).collect();
        engines.sync(&states, &listener(), SR);
        assert_eq!(engines.live_beds(), MAX_REMOTE_ENGINES);
    }

    #[test]
    fn a_tank_on_the_right_hums_in_the_right_ear() {
        let mut engines = RemoteEngines::default();
        // Looking down +Z, screen right is -X.
        engines.sync(&[state(1, Vec3::new(-40.0, 0.0, 10.0), 8.0)], &listener(), SR);
        let out = render_stereo(&mut engines, 1.0);
        let left: Vec<f32> = out.iter().step_by(2).copied().collect();
        let right: Vec<f32> = out.iter().skip(1).step_by(2).copied().collect();
        assert!(
            rms(&right) > rms(&left) * 1.3,
            "right-flank engine must favor the right ear: L {} vs R {}",
            rms(&left),
            rms(&right)
        );
    }

    #[test]
    fn an_unreported_bed_spools_down_and_frees_its_slot() {
        let mut engines = RemoteEngines::default();
        engines.sync(&[state(1, Vec3::new(0.0, 0.0, 25.0), 6.0)], &listener(), SR);
        render_stereo(&mut engines, 0.5);
        assert_eq!(engines.live_beds(), 1);

        // The tank disappears (died / drove off): the bed fades, then the slot frees itself.
        engines.sync(&[], &listener(), SR);
        assert_eq!(engines.live_beds(), 1, "the bed lingers to spool down, no hard cut");
        let fade = render_stereo(&mut engines, 2.0);
        assert!(rms(&fade[fade.len() - 9_600..]) < 1.0e-4, "the bed fades to silence");
        engines.sync(&[], &listener(), SR);
        assert_eq!(engines.live_beds(), 0, "a silent absent bed frees its slot");
    }
}
