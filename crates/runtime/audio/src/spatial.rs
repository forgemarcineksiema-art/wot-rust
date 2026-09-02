//! Spatialization: how a mono voice at a world position reaches two ears. Honest physics on
//! purpose — 1/r loudness, air absorption dulling far sounds, and true speed-of-sound arrival
//! delay, so a shot on the far flank flashes first and bangs a second later.

use glam::Vec3;

use crate::dsp::OnePoleLowPass;
use crate::voice::Voice;

/// Where the player's ears are: camera position plus facing, updated once per rendered frame.
#[derive(Debug, Clone, Copy)]
pub struct Listener {
    pub position: Vec3,
    /// Look direction; only the XZ heading pans (elevation never flips left/right).
    pub forward: Vec3,
}

impl Default for Listener {
    fn default() -> Self {
        Self { position: Vec3::ZERO, forward: Vec3::Z }
    }
}

/// Beyond this a sound contributes nothing worth a voice slot.
const AUDIBLE_GAIN_FLOOR: f32 = 0.003;
/// Distance at which a source has fallen to half its close-up level.
const REFERENCE_DISTANCE_M: f32 = 18.0;
pub(crate) const SPEED_OF_SOUND_MPS: f32 = 343.0;

/// One playing voice with its spatial rendering state: gain, pan, air filter and the samples of
/// speed-of-sound delay still to elapse before its first sample plays.
pub(crate) struct VoiceSlot {
    voice: Box<dyn Voice>,
    pub(crate) gain: f32,
    /// -1 full left .. +1 full right.
    pan: f32,
    air: OnePoleLowPass,
    delay_samples: usize,
}

impl VoiceSlot {
    /// Place a voice in the world relative to `listener`. Returns `None` when distance has
    /// pushed it below audibility — an inaudible source must not claim a slot.
    ///
    /// `occlusion` (0 clear .. 1 fully masked by terrain) is the game's judgment — the audio
    /// crate knows no heightmap. A masked source diffracts around the ridge: quieter, and with
    /// its highs stripped much harder than open-air absorption alone.
    pub fn positioned(
        voice: Box<dyn Voice>,
        listener: &Listener,
        position: Vec3,
        gain: f32,
        travel_delay: bool,
        occlusion: f32,
        sample_rate_hz: f32,
    ) -> Option<Self> {
        Self::positioned_with_reference(
            voice,
            listener,
            position,
            gain,
            travel_delay,
            occlusion,
            REFERENCE_DISTANCE_M,
            sample_rate_hz,
        )
    }

    /// [`Self::positioned`] with the source's own reference distance (Inny Poziom S11): the
    /// distance at which its gain is its nominal value. A plate clang carries further than a
    /// footfall, so it is not one number for every voice.
    #[allow(clippy::too_many_arguments)]
    pub fn positioned_with_reference(
        voice: Box<dyn Voice>,
        listener: &Listener,
        position: Vec3,
        gain: f32,
        travel_delay: bool,
        occlusion: f32,
        reference_m: f32,
        sample_rate_hz: f32,
    ) -> Option<Self> {
        let occlusion = occlusion.clamp(0.0, 1.0);
        let offset = position - listener.position;
        let distance = offset.length();
        let reference_m = reference_m.max(1.0);
        let gain = gain * reference_m / (reference_m + distance) * (1.0 - 0.55 * occlusion);
        if gain < AUDIBLE_GAIN_FLOOR {
            return None;
        }
        // Screen right for a right-handed, Y-up look-at: (-forward.z, 0, forward.x).
        let right = Vec3::new(-listener.forward.z, 0.0, listener.forward.x);
        let flat = Vec3::new(offset.x, 0.0, offset.z);
        let pan = if flat.length_squared() > 1.0e-6 {
            flat.normalize().dot(right.normalize_or_zero()).clamp(-1.0, 1.0)
        } else {
            0.0
        };
        // Air absorption: ~20 kHz up close, ~2 kHz at half a kilometer. Terrain in the way
        // closes the filter much further — only the low thud diffracts over a ridge.
        let cutoff_hz = 20_000.0 / (1.0 + distance / 60.0) / (1.0 + 5.0 * occlusion);
        let delay_samples = if travel_delay {
            (distance / SPEED_OF_SOUND_MPS * sample_rate_hz) as usize
        } else {
            0
        };
        Some(Self {
            voice,
            gain,
            pan,
            air: OnePoleLowPass::new(cutoff_hz, sample_rate_hz),
            delay_samples,
        })
    }

    /// A listener-local voice (UI cues, the breech): centered, undelayed, full bandwidth.
    pub fn flat(voice: Box<dyn Voice>, gain: f32, sample_rate_hz: f32) -> Self {
        Self {
            voice,
            gain,
            pan: 0.0,
            air: OnePoleLowPass::new(20_000.0, sample_rate_hz),
            delay_samples: 0,
        }
    }

    /// Advance this voice by one stereo chunk, mixing (additively) into `out` through the air
    /// filter and the constant-power pan. Returns whether the voice is still alive.
    pub fn mix_into(&mut self, out: &mut [f32], scratch: &mut [f32]) -> bool {
        let frames = out.len() / 2;
        let skip = self.delay_samples.min(frames);
        self.delay_samples -= skip;
        let audible = frames - skip;
        if audible == 0 {
            return true; // still waiting for the wavefront to arrive
        }
        scratch[..audible].fill(0.0);
        let alive = self.voice.render(&mut scratch[..audible]);
        // Constant-power pan: equal energy at center, full isolation at the edges.
        let angle = (self.pan + 1.0) * std::f32::consts::FRAC_PI_4;
        let (left_gain, right_gain) = (angle.cos() * self.gain, angle.sin() * self.gain);
        for (frame, sample) in out[skip * 2..].chunks_exact_mut(2).zip(&scratch[..audible]) {
            let absorbed = self.air.process(*sample);
            frame[0] += absorbed * left_gain;
            frame[1] += absorbed * right_gain;
        }
        alive
    }
}
