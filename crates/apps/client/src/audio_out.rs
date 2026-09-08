//! The platform audio output: a cpal stream whose callback pulls interleaved stereo from the
//! [`audio::AudioEngine`] it OWNS. This is the ONLY device-touching audio code in the workspace —
//! everything audible lives in the `audio` crate and is unit-tested there. No output device (CI,
//! headless, exotic formats) simply means a silent game, never a crash.
//!
//! Q4 (2026-09-08): the engine used to sit behind a `Mutex` shared with the main thread, and the
//! docstring claimed contention "never blocks the audio thread's cadence" — which nothing
//! enforced: the main thread held the lock for the whole control update (events, listener,
//! remote engines) every frame, and the callback, on its real-time thread, waited for it. Now
//! the main thread posts a [`ControlFrame`] into a bounded mailbox and never touches the engine;
//! the callback drains the mailbox (a non-blocking `try_recv`, no lock, no allocation on the
//! audio thread beyond dropping what it received) and applies the newest frame before rendering.
//! `tests/suite/audio_rt.rs` refuses a lock on this file's callback path.

use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use tracing::{info, warn};

/// One frame of control from the main thread: the continuous state the engine follows and the
/// one-shot events it fires. Built once per presented frame by `ClientApp::flush_audio`.
#[derive(Debug, Default)]
pub(crate) struct ControlFrame {
    pub listener: Option<audio::Listener>,
    pub wind_level: f32,
    /// (level, radio pan, radio gain).
    pub hangar_bed: (f32, f32, f32),
    pub rain_level: f32,
    /// (rpm norm, load, ground speed m/s, running).
    pub player_engine: (f32, f32, f32, bool),
    pub track_surface: Option<audio::TrackSurface>,
    pub player_fire: bool,
    pub turret_slew: f32,
    pub scope_muffle: bool,
    pub remote_engines: Vec<audio::RemoteEngineState>,
    /// (event, terrain occlusion).
    pub events: Vec<(audio::AudioEvent, f32)>,
    /// A settings change; `None` leaves the engine's gain where it is.
    pub master_gain: Option<f32>,
}

impl ControlFrame {
    /// Apply this frame to the engine — on the audio thread, after `try_recv`.
    fn apply(self, engine: &mut audio::AudioEngine) {
        if let Some(gain) = self.master_gain {
            engine.set_master_gain(gain);
        }
        if let Some(listener) = self.listener {
            engine.set_listener(listener);
        }
        engine.set_wind_level(self.wind_level);
        engine.set_hangar_bed(self.hangar_bed.0, self.hangar_bed.1, self.hangar_bed.2);
        engine.set_rain_level(self.rain_level);
        let (rpm_norm, load, speed_mps, running) = self.player_engine;
        engine.set_player_engine(rpm_norm, load, speed_mps, running);
        if let Some(surface) = self.track_surface {
            engine.set_track_surface(surface);
        }
        engine.set_player_fire(self.player_fire);
        engine.set_turret_slew(self.turret_slew);
        engine.set_scope_muffle(self.scope_muffle);
        engine.set_remote_engines(&self.remote_engines);
        for (event, occlusion) in self.events {
            engine.push_event_occluded(event, occlusion);
        }
    }
}

/// Frames the mailbox holds before the main thread starts dropping the OLDEST: a callback that
/// stalls (a device hiccup) must not make the game allocate without bound, and a control frame
/// older than a few presented frames is stale anyway — the newest one carries the state.
const MAILBOX_FRAMES: usize = 8;

/// The main thread's side of the mailbox. Sending never blocks: a full mailbox makes room by
/// dropping the oldest queued frame's continuous state and KEEPING its events (a shot that
/// happened is not un-happened by a busy device) — see [`AudioOutput::send`].
pub(crate) struct AudioOutput {
    frames: SyncSender<ControlFrame>,
    /// Events the mailbox had no room for; carried into the next frame that fits.
    carried_events: std::cell::RefCell<Vec<(audio::AudioEvent, f32)>>,
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
        let (frames, inbox) = mpsc::sync_channel::<ControlFrame>(MAILBOX_FRAMES);
        let mut engine = audio::AudioEngine::new(sample_rate);
        // The engine renders straight stereo; other layouts get the pair spread over the first
        // two channels (mono collapses to the left sum).
        let mut stereo_scratch: Vec<f32> = Vec::new();
        let stream = device
            .build_output_stream(
                &config.into(),
                move |out: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    drain_control(&inbox, &mut engine);
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
        Some(Self { frames, carried_events: std::cell::RefCell::new(Vec::new()), _stream: stream })
    }

    /// Post one frame of control. Never blocks and never waits for the audio thread: when the
    /// mailbox is full (the device stalled), the frame's continuous state is dropped — the next
    /// frame carries it again — and its events are kept for the next frame that fits.
    pub fn send(&self, mut frame: ControlFrame) {
        {
            let mut carried = self.carried_events.borrow_mut();
            if !carried.is_empty() {
                carried.append(&mut frame.events);
                std::mem::swap(&mut *carried, &mut frame.events);
            }
        }
        match self.frames.try_send(frame) {
            Ok(()) => {}
            Err(TrySendError::Full(frame)) | Err(TrySendError::Disconnected(frame)) => {
                let mut carried = self.carried_events.borrow_mut();
                carried.extend(frame.events);
                // A device that never drains must not turn into an unbounded event list.
                let excess = carried.len().saturating_sub(256);
                if excess > 0 {
                    carried.drain(..excess);
                }
            }
        }
    }

    /// A settings change on its own frame: the gain rides the mailbox like everything else.
    pub fn send_master_gain(&self, gain: f32) {
        self.send(ControlFrame { master_gain: Some(gain), ..ControlFrame::default() });
    }
}

/// The callback's half: take every frame the main thread posted since the last callback and
/// apply them in order (the newest continuous state wins, every event fires). `try_recv` does
/// not block; an empty mailbox costs one atomic load.
fn drain_control(inbox: &Receiver<ControlFrame>, engine: &mut audio::AudioEngine) {
    while let Ok(frame) = inbox.try_recv() {
        frame.apply(engine);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_applies_its_state_and_fires_its_events_in_order() {
        let (tx, rx) = mpsc::sync_channel::<ControlFrame>(MAILBOX_FRAMES);
        let mut engine = audio::AudioEngine::new(48_000.0);
        tx.send(ControlFrame { master_gain: Some(0.25), ..ControlFrame::default() }).expect("room");
        tx.send(ControlFrame { master_gain: Some(0.5), ..ControlFrame::default() }).expect("room");
        tx.send(ControlFrame { wind_level: 0.9, ..ControlFrame::default() }).expect("room");
        drain_control(&rx, &mut engine);
        assert!(
            (engine.master_gain() - 0.5).abs() < 1e-6,
            "the newest frame's state wins; a frame without a gain leaves it"
        );
        // Nothing left: the next callback pays one failed try_recv and renders.
        assert!(rx.try_recv().is_err());
    }
}
