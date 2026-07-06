//! The platform audio output: a cpal stream whose callback pulls interleaved stereo from the
//! shared [`audio::AudioEngine`]. This is the ONLY device-touching audio code in the workspace —
//! everything audible lives in the `audio` crate and is unit-tested there. No output device (CI,
//! headless, exotic formats) simply means a silent game, never a crash.

use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use tracing::{info, warn};

pub(crate) struct AudioOutput {
    engine: Arc<Mutex<audio::AudioEngine>>,
    /// Held for its lifetime: dropping the stream stops playback.
    _stream: cpal::Stream,
}

impl AudioOutput {
    /// Open the default output device at its native sample rate. Any failure is logged and
    /// answered with `None` — audio is a presentation layer, not a requirement to play.
    pub fn try_new() -> Option<Self> {
        let host = cpal::default_host();
        let device = host.default_output_device().or_else(|| {
            warn!("no default audio output device; the game runs silent");
            None
        })?;
        let config = match device.default_output_config() {
            Ok(config) => config,
            Err(error) => {
                warn!(%error, "no default audio output config; the game runs silent");
                return None;
            }
        };
        if config.sample_format() != cpal::SampleFormat::F32 {
            warn!(format = ?config.sample_format(), "unsupported sample format; running silent");
            return None;
        }
        let channels = config.channels() as usize;
        let sample_rate = config.sample_rate().0 as f32;
        let engine = Arc::new(Mutex::new(audio::AudioEngine::new(sample_rate)));
        let callback_engine = Arc::clone(&engine);
        // The engine renders straight stereo; other layouts get the pair spread over the first
        // two channels (mono collapses to the left sum).
        let mut stereo_scratch: Vec<f32> = Vec::new();
        let stream = device
            .build_output_stream(
                &config.into(),
                move |out: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    let Ok(mut engine) = callback_engine.lock() else {
                        out.fill(0.0);
                        return;
                    };
                    if channels == 2 {
                        engine.render(out);
                        return;
                    }
                    let frames = out.len() / channels.max(1);
                    if stereo_scratch.len() < frames * 2 {
                        stereo_scratch.resize(frames * 2, 0.0);
                    }
                    engine.render(&mut stereo_scratch[..frames * 2]);
                    for (frame, stereo) in
                        out.chunks_exact_mut(channels).zip(stereo_scratch.chunks_exact(2))
                    {
                        frame[0] = stereo[0];
                        if channels > 1 {
                            frame[1] = stereo[1];
                        } else {
                            frame[0] = (stereo[0] + stereo[1]) * 0.5;
                        }
                        for extra in frame.iter_mut().skip(2) {
                            *extra = 0.0;
                        }
                    }
                },
                |error| warn!(%error, "audio stream error"),
                None,
            )
            .ok()?;
        if let Err(error) = stream.play() {
            warn!(%error, "failed to start the audio stream; the game runs silent");
            return None;
        }
        info!(sample_rate, channels, "procedural audio output running");
        Some(Self { engine, _stream: stream })
    }

    /// Run `apply` against the shared engine (events, listener, engine bed state). Contention is
    /// a lost frame of control updates at worst — never a block on the audio thread's cadence.
    pub fn with_engine(&self, apply: impl FnOnce(&mut audio::AudioEngine)) {
        if let Ok(mut engine) = self.engine.lock() {
            apply(&mut engine);
        }
    }
}
