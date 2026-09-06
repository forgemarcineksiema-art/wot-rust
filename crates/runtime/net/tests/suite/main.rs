//! One test binary for `net`: every integration test file is a module here, so a change
//! links ONE program instead of one per file (2026-09-06: 314 test binaries across the
//! workspace were ~180 CPU-seconds of linking per heavy crate on every gate). Goldens and
//! env-gated tests (`gate_completeness.rs`) keep their own binaries: they re-record under an
//! env var and must not share a process with a value test of the same file.

mod observer_bit;
mod protocol_frame;
mod protocol_roundtrip;
mod snapshot_budget;
mod snapshot_combat;
mod snapshot_filter;
mod snapshot_schedule;
