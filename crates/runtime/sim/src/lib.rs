mod aim_dispersion;
mod aiming;
mod breach_space;
mod clock;
mod combat;
mod command;
mod cover_damage;
mod crater_ledger;
mod drive_modules;
mod drowning;
pub mod event_stamp;
mod landing;
mod module_hit;
mod ramming;
mod repair;
mod replay;
mod shell;
mod shell_continuation;
mod shell_splash;
mod shell_step;
mod shell_trace;
mod spotting;
mod state;
mod tank_drive;
mod tank_factory;
mod tank_state;
mod timestep;
mod wreck;

pub use aim_dispersion::recover_dispersion;
pub use aiming::{
    AimingState, GUN_ELEVATION_RATE_RAD_S, MAX_GUN_PITCH_RAD, MIN_GUN_PITCH_RAD, step_aiming,
};
pub use clock::{
    DEFAULT_SERVER_TICK_HZ, DEFAULT_SIMULATION_TICK_HZ, DEFAULT_SNAPSHOT_HZ, SimulationClock,
};
pub use combat::FIRE_BUFFER_S;
pub use command::TankCommand;
pub use cover_damage::{
    CoverPhase, CoverState, LiveCover, cover_states_for, damage_cover, initial_cover_states,
    live_cover_for_movement, live_cover_for_sight_and_shells, movement_cover_for_phase_bytes,
    record_cover_scar, rubble_mounds, rubble_mounds_for_phase_bytes, sight_cover_for_phase_bytes,
};
pub use crater_ledger::{MAX_CRATERS, record_high_explosive_burst};
pub use drive_modules::{DriveModuleStatus, TrackDriveStatus, TrackSideDrive};
pub use drowning::{DROWN_DEPTH_M, DROWN_PULSE_INTERVAL_S, ENGINE_FLOOD_S};
pub use landing::SAFE_LANDING_MPS;
pub use repair::{
    CrewRepair, MODULE_PATCH_FRACTION, MODULE_PATCH_S, TRACK_REGEN_CHUNK, TRACK_REGEN_INTERVAL_S,
    TRACK_REPAIR_S,
};
pub use replay::{Replay, ReplayExpected, ReplayFrame, ReplayReport, ReplaySpawn, run_replay};
pub use shell::ShellState;
pub use shell_trace::{
    SHELL_MAX_AGE_SECONDS, SegmentImpact, ShellTraceWorld, TraceOutcome, TraceTank, segment_impact,
    trace_shell,
};
pub use spotting::{
    SPOTTED_HOLD_TICKS, SPOTTING_INTERVAL_TICKS, VIEW_RANGE_M, compute_observer_masks,
    compute_spotted_masks, line_of_sight, tank_line_of_sight,
};
pub use state::SimulationState;
pub use tank_drive::{TankDriveState, TankDriveWorld, step_tank_drive};
pub use tank_state::TankState;
pub use timestep::FixedTimestep;
