//! One test binary for `battle_host`: every integration test file is a module here, so a change
//! links ONE program instead of one per file (2026-09-06: 314 test binaries across the
//! workspace were ~180 CPU-seconds of linking per heavy crate on every gate). Goldens and
//! env-gated tests (`gate_completeness.rs`) keep their own binaries: they re-record under an
//! env var and must not share a process with a value test of the same file.

mod battle_formats;
mod battle_statistics;
mod bot_water;
mod bystra_battle;
mod format_lobby;
mod local_authoritative_server;
mod mazurski_battle;
mod orliny_battle;
mod ostrogorsk_battle;
mod prokhorovka_battle;
mod remote_battle;
mod remote_reconnect;
mod server_tick_policy;
