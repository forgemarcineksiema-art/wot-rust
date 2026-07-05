//! Procedural battle audio: the whole soundscape synthesized at runtime, no recorded assets.
//!
//! The crate is renderer- and device-free on purpose: [`AudioEngine`] is a pure function from
//! (events, control state) to interleaved stereo samples, deterministic under a fixed event
//! sequence, so every sound in the game is unit-testable the same way the sim is. The client
//! owns the platform stream and calls [`AudioEngine::render`] from its audio callback.
//!
//! Layering, from the ground up:
//! - [`dsp`] — noise, filters, envelopes, the soft clipper: the primitives.
//! - [`voice`] — the mono voice contract plus waveform analysis helpers for tests.
//! - [`voices`] — the designs: cannon report, armor clang, ground thud, diesel bed, wind, UI.
//! - [`spatial`] — how a mono voice at a world position reaches two ears: 1/r gain,
//!   constant-power pan, air absorption, speed-of-sound delay.
//! - [`mixer`] — events to voices, the voice budget, and the master bus.

pub mod dsp;
pub mod events;
pub mod mixer;
pub mod remote;
pub mod spatial;
pub mod voice;
pub mod voices;

pub use events::AudioEvent;
pub use mixer::AudioEngine;
pub use remote::RemoteEngineState;
pub use spatial::Listener;
pub use voices::impact::GroundKind;
